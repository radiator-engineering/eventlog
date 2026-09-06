//! Pure rendering of [`App`] into a [`ratatui::Frame`]. Kept separate from
//! `tui::mod`'s event loop so render tests can drive it against a
//! `ratatui::backend::TestBackend` with no real terminal.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table};

use crate::model::event::Event;
use crate::query::Phase;

use super::{App, BottomView, Pane};

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let has_input = app.filter_input.is_some();

    let chunks = if has_input {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3),
                Constraint::Percentage(40),
                Constraint::Length(1),
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Percentage(40)])
            .split(area)
    };

    render_follow(frame, app, chunks[0]);
    render_bottom(frame, app, chunks[1]);
    if has_input {
        render_filter_input(frame, app, chunks[2]);
    }
}

fn render_follow(frame: &mut Frame, app: &App, area: Rect) {
    let filtered = app.filtered_events();

    let mut title = String::from("follow");
    if !app.filter.is_empty() {
        title.push_str(&format!(" [filter: {}]", app.filter));
    }
    if app.follow {
        title.push_str(" [following]");
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if app.pane == Pane::Follow {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        });

    let visible = area.height.saturating_sub(2).max(1) as usize;
    let start = filtered.len().saturating_sub(visible);

    let items: Vec<ListItem> = filtered[start..]
        .iter()
        .enumerate()
        .map(|(offset, e)| {
            let idx = start + offset;
            let style = if idx == app.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(format_row(e))).style(style)
        })
        .collect();

    frame.render_widget(List::new(items).block(block), area);
}

fn format_row(e: &Event) -> String {
    let summary = e.fields.get("summary").map(|s| s.as_str()).unwrap_or("");
    format!(
        "{:>5} {:<10} {:<24} {}",
        e.seq,
        e.r#type,
        e.agent.as_deref().or(e.by.as_deref()).unwrap_or("-"),
        summary
    )
}

fn render_bottom(frame: &mut Frame, app: &App, area: Rect) {
    match app.bottom {
        BottomView::Agents => render_agents(frame, app, area),
        BottomView::State => render_state(frame, app, area),
        BottomView::Why => render_why(frame, app, area),
    }
}

fn render_agents(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("agents")
        .border_style(if app.pane == Pane::Bottom {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        });

    let rows: Vec<Row> = app
        .state
        .agents
        .values()
        .map(|a| {
            Row::new(vec![
                a.name.clone(),
                a.model.clone().unwrap_or_default(),
                phase_label(a.phase).to_string(),
            ])
        })
        .collect();

    let widths = [
        Constraint::Percentage(40),
        Constraint::Percentage(30),
        Constraint::Percentage(30),
    ];
    let table = Table::new(rows, widths)
        .header(Row::new(vec!["agent", "model", "phase"]))
        .block(block);

    frame.render_widget(table, area);
}

fn phase_label(phase: Phase) -> &'static str {
    match phase {
        Phase::Spawned => "spawned",
        Phase::Prompted => "prompted",
        Phase::Claimed => "claimed",
        Phase::Progressing => "progressing",
        Phase::Resulted => "resulted",
        Phase::Retired => "retired",
    }
}

fn render_state(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("state")
        .border_style(if app.pane == Pane::Bottom {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        });

    let mut lines = vec![
        format!("at seq {}", app.state.at),
        format!("agents: {}", app.state.agents.len()),
        format!("open claims: {}", app.state.claims.len()),
        format!("open escalations: {}", app.state.escalations.len()),
        format!("open intents: {}", app.state.intents.len()),
        format!("open lifecycles: {}", app.state.open_lifecycles.len()),
    ];
    for (key, (value, seq)) in &app.state.decisions {
        lines.push(format!("decision {key}={value} (seq {seq})"));
    }

    let text: Vec<Line> = lines.into_iter().map(Line::from).collect();
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_why(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("why")
        .border_style(if app.pane == Pane::Bottom {
            Style::default().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        });

    let text: Vec<Line> = app.why_lines().into_iter().map(Line::from).collect();
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn render_filter_input(frame: &mut Frame, app: &App, area: Rect) {
    let buf = app.filter_input.as_deref().unwrap_or("");
    frame.render_widget(Paragraph::new(format!("filter: {buf}")), area);
}
