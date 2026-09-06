//! Spec section 8: the live TUI over `query::State`.
//!
//! `App` holds the raw events and the fold; every mutation re-folds because
//! folding is cheap at this scale (spec section 8). Rendering
//! ([`views::render`]) is a pure function of `App`, so it is testable with
//! `ratatui::backend::TestBackend` without a real terminal.

pub mod views;

use std::io;
use std::time::Duration;

use anyhow::Context;
use crossterm::event::{self, Event as CEvent, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::{Backend, CrosstermBackend};

use crate::log::Log;
use crate::model::config::{Config, KeysConfig};
use crate::model::event::Event;
use crate::query::{self, State};

/// Which of the two panes has focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Follow,
    Bottom,
}

/// What the bottom pane is currently showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomView {
    Agents,
    State,
    Why,
}

/// A key press outside the filter input can ask the driving loop to do
/// something it cannot do itself: quit, or suspend the terminal to run
/// `open`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Quit,
    OpenRef(String),
}

/// Reference fields `why` walks, in both directions (spec section 8).
const REF_FIELDS: [&str; 4] = ["for", "for_ack", "seq_done", "intent"];

pub struct App {
    pub events: Vec<Event>,
    pub cfg: Config,
    pub state: State,
    pub filter: String,
    /// `Some(buffer)` while the filter input line is open for editing.
    pub filter_input: Option<String>,
    pub follow: bool,
    pub pane: Pane,
    pub bottom: BottomView,
    /// Index into [`App::filtered_events`], not into `events`.
    pub selected: usize,
    pub quit: bool,
}

impl App {
    pub fn new(events: Vec<Event>, cfg: Config) -> App {
        let state = query::fold(&events, &cfg);
        let mut app = App {
            events,
            cfg,
            state,
            filter: String::new(),
            filter_input: None,
            follow: true,
            pane: Pane::Follow,
            bottom: BottomView::Agents,
            selected: 0,
            quit: false,
        };
        app.selected = app.filtered_events().len().saturating_sub(1);
        app
    }

    fn refold(&mut self) {
        self.state = query::fold(&self.events, &self.cfg);
    }

    /// Replace the events wholesale and re-fold. Called on every new line
    /// while following (spec section 8: "re-folds on every new line").
    pub fn update_events(&mut self, events: Vec<Event>) {
        self.events = events;
        self.refold();
        self.clamp_selection();
        if self.follow {
            self.selected = self.filtered_events().len().saturating_sub(1);
        }
    }

    /// The TUI's own filter, over `type`, `by`, `agent` and `ref`
    /// (case-insensitive substring). Built fresh over `Vec<Event>`, not
    /// imported from `cmd::view`.
    pub fn filtered_events(&self) -> Vec<&Event> {
        if self.filter.is_empty() {
            self.events.iter().collect()
        } else {
            let needle = self.filter.to_lowercase();
            self.events
                .iter()
                .filter(|e| event_matches(e, &needle))
                .collect()
        }
    }

    fn clamp_selection(&mut self) {
        let len = self.filtered_events().len();
        self.selected = if len == 0 {
            0
        } else {
            self.selected.min(len - 1)
        };
    }

    fn move_selection(&mut self, delta: isize) {
        let len = self.filtered_events().len();
        if len == 0 {
            return;
        }
        let cur = (self.selected as isize + delta).clamp(0, len as isize - 1);
        self.selected = cur as usize;
        self.follow = false;
    }

    pub fn selected_ref(&self) -> Option<String> {
        let events = self.filtered_events();
        events
            .get(self.selected)
            .and_then(|e| e.fields.get("ref").cloned())
    }

    /// The `why` report for the selected row: itself, what it references,
    /// and what references it, over the reference fields the spec names.
    pub fn why_lines(&self) -> Vec<String> {
        let events = self.filtered_events();
        match events.get(self.selected) {
            Some(e) => why_report(&self.events, e.seq),
            None => vec!["no selection".to_string()],
        }
    }

    /// Handle one key press. While the filter input is open, every key
    /// edits the buffer instead of triggering a binding.
    pub fn handle_key(&mut self, code: KeyCode, keys: &KeysConfig) -> Action {
        if let Some(buf) = self.filter_input.as_mut() {
            match code {
                KeyCode::Enter => {
                    self.filter = buf.clone();
                    self.filter_input = None;
                    self.clamp_selection();
                }
                KeyCode::Esc => {
                    self.filter_input = None;
                }
                KeyCode::Backspace => {
                    buf.pop();
                }
                KeyCode::Char(c) => {
                    buf.push(c);
                }
                _ => {}
            }
            return Action::None;
        }

        if key_matches(&keys.filter, code) {
            self.filter_input = Some(String::new());
            return Action::None;
        }
        if key_matches(&keys.follow, code) {
            self.follow = !self.follow;
            return Action::None;
        }
        if key_matches(&keys.panes, code) {
            self.pane = match self.pane {
                Pane::Follow => Pane::Bottom,
                Pane::Bottom => Pane::Follow,
            };
            return Action::None;
        }
        if key_matches(&keys.open, code) {
            return match self.selected_ref() {
                Some(target) => Action::OpenRef(target),
                None => Action::None,
            };
        }

        match code {
            KeyCode::Char('w') => {
                self.bottom = BottomView::Why;
            }
            KeyCode::Char('q') => {
                self.quit = true;
                return Action::Quit;
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            _ => {}
        }
        Action::None
    }
}

fn event_matches(e: &Event, needle_lower: &str) -> bool {
    let ref_field = e.fields.get("ref").map(|s| s.as_str());
    [
        Some(e.r#type.as_str()),
        e.by.as_deref(),
        e.agent.as_deref(),
        ref_field,
    ]
    .into_iter()
    .flatten()
    .any(|s| s.to_lowercase().contains(needle_lower))
}

fn key_matches(spec: &str, code: KeyCode) -> bool {
    match code {
        KeyCode::Tab => spec.eq_ignore_ascii_case("tab"),
        KeyCode::Char(c) => spec.chars().count() == 1 && spec.starts_with(c),
        _ => false,
    }
}

fn why_report(events: &[Event], seq: u64) -> Vec<String> {
    let Some(subject) = events.iter().find(|e| e.seq == seq) else {
        return vec![format!("no event at seq {seq}")];
    };

    let mut lines = vec![format!(
        "seq {} {} by={}",
        subject.seq,
        subject.r#type,
        subject.writer()
    )];

    for field in REF_FIELDS {
        let Some(target) = subject.seq_ref(field) else {
            continue;
        };
        match events.iter().find(|e| e.seq == target) {
            Some(t) => lines.push(format!(
                "  {field} -> seq {} {} by={}",
                t.seq,
                t.r#type,
                t.writer()
            )),
            None => lines.push(format!("  {field} -> seq {target} (not found)")),
        }
    }

    for e in events {
        for field in REF_FIELDS {
            if e.seq_ref(field) == Some(seq) {
                lines.push(format!(
                    "  referenced by seq {} {} ({field})",
                    e.seq, e.r#type
                ));
            }
        }
    }

    lines
}

/// Run the live app loop against a real terminal. Not exercised by render
/// tests; those call [`views::render`] directly against a `TestBackend`.
pub fn run_terminal(mut app: App, log: Log) -> anyhow::Result<()> {
    enable_raw_mode().context("enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).context("enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("create terminal")?;

    let keys = app.cfg.keys.clone();
    let outcome = drive(&mut terminal, &mut app, &log, &keys);

    disable_raw_mode().ok();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    outcome
}

fn drive<B: Backend + io::Write>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    log: &Log,
    keys: &KeysConfig,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| views::render(f, app))?;

        if event::poll(Duration::from_millis(250))? {
            if let CEvent::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                match app.handle_key(key.code, keys) {
                    Action::Quit => break,
                    Action::OpenRef(target) => open_ref(terminal, &target)?,
                    Action::None => {}
                }
            }
        } else if let Ok(report) = log.read()
            && report.events.len() != app.events.len()
        {
            app.update_events(report.events);
        }

        if app.quit {
            break;
        }
    }
    Ok(())
}

/// Suspend the terminal, run `open` on the selected row's `ref`, and
/// restore the terminal on return.
fn open_ref<B: Backend + io::Write>(
    terminal: &mut Terminal<B>,
    target: &str,
) -> anyhow::Result<()> {
    disable_raw_mode().ok();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = std::process::Command::new("open").arg(target).status();
    let _ = execute!(terminal.backend_mut(), EnterAlternateScreen);
    enable_raw_mode().ok();
    terminal.clear().ok();
    Ok(())
}
