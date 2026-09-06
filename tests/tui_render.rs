use crossterm::event::KeyCode;
use eventlog::model::config::Config;
use eventlog::model::event::Event;
use eventlog::query::Phase;
use eventlog::tui::{App, BottomView};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

fn load_fixture() -> Vec<Event> {
    let content = std::fs::read_to_string("tests/fixtures/self-log-2026-09-06.jsonl").unwrap();
    content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| Event::parse_line(l).unwrap())
        .collect()
}

fn buffer_text(buf: &Buffer) -> String {
    let area = buf.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buf.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "));
        }
        out.push('\n');
    }
    out
}

fn draw(app: &App) -> String {
    let backend = TestBackend::new(100, 30);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| eventlog::tui::views::render(f, app))
        .unwrap();
    buffer_text(terminal.backend().buffer())
}

#[test]
fn follow_view_shows_last_n_lines_of_the_fixture() {
    let events = load_fixture();
    let app = App::new(events, Config::default());

    let content = draw(&app);

    // The fixture's tip (seq 125) must be visible; the very first line
    // (seq 1) must have scrolled out of a 30-row terminal.
    assert!(
        content.contains("125"),
        "expected the tip to be visible:\n{content}"
    );
    assert!(
        !content.contains("log-writers"),
        "expected seq 1 to have scrolled out:\n{content}"
    );
}

#[test]
fn filter_key_then_typing_ack_and_enter_leaves_only_ack_rows() {
    let events = load_fixture();
    let mut app = App::new(events, Config::default());
    let keys = app.cfg.keys.clone();

    app.handle_key(KeyCode::Char('/'), &keys);
    assert!(
        app.filter_input.is_some(),
        "filter key should open the input line"
    );

    for c in "ack".chars() {
        app.handle_key(KeyCode::Char(c), &keys);
    }
    app.handle_key(KeyCode::Enter, &keys);

    assert_eq!(app.filter, "ack");
    assert!(app.filter_input.is_none());

    let filtered = app.filtered_events();
    assert!(!filtered.is_empty());
    assert!(filtered.iter().all(|e| e.r#type == "ack"));

    let content = draw(&app);
    assert!(
        content.contains("[filter: ack]"),
        "expected the filter to show in the title:\n{content}"
    );
}

#[test]
fn agents_pane_shows_a_retired_agent_with_phase_retired() {
    let events = load_fixture();
    let mut app = App::new(events, Config::default());
    app.bottom = BottomView::Agents;

    let retired = app
        .state
        .agents
        .values()
        .find(|a| a.phase == Phase::Retired)
        .expect("fixture has at least one retired agent");
    let retired_name = retired.name.clone();

    let content = draw(&app);
    assert!(
        content.contains(&retired_name),
        "expected {retired_name} in the agents pane:\n{content}"
    );
    assert!(
        content.contains("retired"),
        "expected the retired phase label:\n{content}"
    );
}
