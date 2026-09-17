//! Dedicated `storey` error handler: a modal popup, not a bottom line.
//!
//! Any CLI failure surfaced through [`App::fail`] keeps its short
//! notice but also pops this dialog (unless already acknowledged),
//! with a pointer to the issue tracker.

use ratatui::{
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use storey_shared::cli::CliError;

use crate::{app::App, bin, dialog, fs};

/// Where to report suspected `storey` bugs.
pub const ISSUES_URL: &str = "https://github.com/matyeusm/storey/issues";

/// One surfaced CLI failure: what was attempted and the short detail.
#[derive(Debug, Clone)]
pub struct ErrorPopup {
    pub op: String,
    pub detail: String,
}

impl App {
    /// Record a CLI failure: short notice always, modal unless the
    /// user already acknowledged errors or one is already open.
    /// Callers reset ack via [`unack_error`] for explicit retries.
    pub fn fail(&mut self, op: &str, e: &CliError) {
        self.notice = if e.code.is_none() {
            bin::missing_notice(self.client.bin())
        } else {
            fs::short_err(e.to_string())
        };
        if self.error_ack || self.error.is_some() {
            return;
        }
        self.error = Some(ErrorPopup {
            op: op.to_string(),
            detail: self.notice.clone(),
        });
    }

    /// Dismiss the modal; background refreshes stay quiet afterwards.
    pub fn close_error(&mut self) {
        self.error = None;
        self.error_ack = true;
    }

    /// Allow the next failure to pop again (explicit user actions).
    pub(crate) fn unack_error(&mut self) {
        self.error_ack = false;
    }
}

/// Render the modal centered over the current frame.
pub fn render(f: &mut ratatui::Frame<'_>, err: &ErrorPopup) {
    let area = f.area();
    let width = 64.min(area.width.saturating_sub(4)).max(30);
    let inner = (width as usize).saturating_sub(4);
    let mut body = vec![Line::styled(
        format!("storey {} failed:", err.op),
        Style::default().add_modifier(Modifier::BOLD),
    )];
    for line in wrap(&err.detail, inner) {
        body.push(Line::raw(line));
    }
    body.push(Line::raw(""));
    body.push(Line::raw("Suspect a bug? Please file an issue:"));
    body.push(Line::styled(ISSUES_URL, Style::default().fg(Color::Cyan)));
    body.push(Line::raw(""));
    body.push(Line::styled(
        "Esc/Enter: dismiss",
        Style::default().add_modifier(Modifier::DIM),
    ));
    let height = (body.len() as u16 + 2)
        .min(area.height.saturating_sub(2))
        .max(9);
    let rect = dialog::centered(area, width, height);
    f.render_widget(Clear, rect);
    f.render_widget(
        Paragraph::new(body).block(Block::default().borders(Borders::ALL).title("storey error")),
        rect,
    );
}

/// Greedy word wrap; a single overlong word takes its own line.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && line.len() + 1 + word.len() > width {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    fn cli_err() -> CliError {
        CliError {
            code: Some(1),
            message: "boom".into(),
            envelope: None,
        }
    }

    fn bad_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-err-{}", std::process::id()));
        App::new(std::path::PathBuf::from("no-such-storey-binary"), dir)
    }

    #[test]
    fn spawn_failure_pops_at_startup() {
        let app = bad_app();
        assert!(app.error.is_some());
        assert!(app.notice.contains("no storey binary"));
    }

    #[test]
    fn dismiss_suppresses_until_retry() {
        let mut app = bad_app();
        app.close_error();
        assert!(app.error.is_none());
        app.fail("view", &cli_err());
        assert!(app.error.is_none());
        app.unack_error();
        app.fail("view", &cli_err());
        assert_eq!(app.error.as_ref().unwrap().op, "view");
    }

    #[test]
    fn modal_shows_tracker_link() {
        let err = ErrorPopup {
            op: "view".into(),
            detail: "boom".into(),
        };
        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| render(f, &err)).unwrap();
        let buf = term.backend().buffer().clone();
        let text: String = buf
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<Vec<_>>()
            .join("");
        assert!(text.contains("storey error"));
        assert!(text.contains("github.com/matyeusm/storey/issues"));
    }

    #[test]
    fn wrap_breaks_long_lines() {
        let lines = wrap("aa bb cc dd", 5);
        assert_eq!(lines, vec!["aa bb", "cc dd"]);
    }
}
