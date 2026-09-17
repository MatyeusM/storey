//! Terminal setup and event loop for the TUI.

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, Event, MouseButton, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{app::App, ui};

/// Run the UI until quit, restoring the terminal afterwards.
pub fn run(mut app: App) -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    execute!(stdout, crossterm::event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, &mut app);
    restore()?;
    result
}

fn restore() -> anyhow::Result<()> {
    let _ = execute!(io::stdout(), crossterm::event::DisableMouseCapture);
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    Ok(())
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    while !app.quit_requested() {
        app.poll_pending();
        terminal.draw(|f| ui::draw(f, app))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Key(key) => {
                if app.on_key(key.code, key.modifiers) {
                    app.quit();
                }
            }
            Event::Mouse(m) if m.kind == MouseEventKind::Down(MouseButton::Left) => {
                ui::click(app, m.column, m.row);
            }
            _ => {}
        }
    }
    Ok(())
}
