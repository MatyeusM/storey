//! Repo-aware top bar: basename + tag breadcrumbs in libraries.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::{
    app::App,
    tagbar::{self, SEPARATOR},
};

/// Width of the `[UP]` hit zone (columns 1..=4, both modes).
pub const UP_MAX_COL: u16 = 4;
/// Right button width (`[Init]` is 6, `[Files]` is 7).
pub const RIGHT_WIDTH: u16 = 7;

fn rev(text: &str) -> Span<'static> {
    Span::styled(
        text.to_string(),
        Style::default().add_modifier(Modifier::REVERSED),
    )
}

/// Keep the tail with a leading ellipsis when over budget.
pub fn fit_path(display: &str, budget: usize) -> String {
    let chars: Vec<char> = display.chars().collect();
    if chars.len() <= budget {
        return display.to_string();
    }
    if budget <= 1 {
        return "…".to_string();
    }
    let tail: String = chars[chars.len() - (budget - 1)..].iter().collect();
    format!("…{tail}")
}

/// Column ranges of the breadcrumb chips for mouse clicks.
pub fn chip_ranges(app: &App) -> Vec<(u16, u16)> {
    tagbar::chip_ranges(UP_MAX_COL + 2, &tagbar::basename_of(&app.cwd), &app.view)
}

/// Top bar: breadcrumbs in libraries, path + `[UP]` elsewhere.
pub fn top_bar(app: &App, width: u16) -> Paragraph<'static> {
    let inner = width.saturating_sub(2) as usize;
    let right = if app.is_repo { "[Files]" } else { "[Init]" };
    if app.is_repo {
        repo_bar(app, inner, right)
    } else {
        plain_bar(app, inner, right)
    }
}

fn repo_bar(app: &App, inner: usize, right: &str) -> Paragraph<'static> {
    let name = tagbar::basename_of(&app.cwd);
    let mut spans: Vec<Span<'static>> =
        vec![rev("[UP]"), Span::raw(" ".to_string()), Span::raw(name)];
    spans.push(Span::raw(" ".to_string()));
    let labels = tagbar::chip_labels(&app.view);
    for (i, label) in labels.iter().enumerate() {
        spans.push(chip_span(label, i, app.tag_sel, &app.view));
        if i + 1 < labels.len() {
            spans.push(Span::raw(SEPARATOR.to_string()));
        }
    }
    spans.push(Span::raw(" ".to_string()));
    let used = bar_len(&spans);
    let pad = inner.saturating_sub(used + right.len());
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(rev(right));
    Paragraph::new(Line::from(spans)).block(repo_title(app))
}

fn chip_span(label: &str, idx: usize, sel: usize, view: &[String]) -> Span<'static> {
    let text = label.to_string();
    if idx == sel || tagbar::is_add_chip(view.len(), idx) || tagbar::is_restore_chip(view, idx) {
        return rev_owned(text);
    }
    Span::raw(text)
}

fn rev_owned(text: String) -> Span<'static> {
    Span::styled(text, Style::default().add_modifier(Modifier::REVERSED))
}

fn plain_bar(app: &App, inner: usize, right: &str) -> Paragraph<'static> {
    let left_len = "[UP] ".len() + 1;
    let budget = inner.saturating_sub(left_len + right.len() + 1).max(1);
    let path = fit_path(&app.cwd.display().to_string(), budget);
    let pad = inner.saturating_sub(left_len + path.chars().count() + right.len());
    let spans: Vec<Span<'static>> = vec![
        rev("[UP]"),
        Span::raw(" ".to_string()),
        Span::raw(path),
        Span::raw(" ".to_string()),
        Span::raw(" ".repeat(pad)),
        rev(right),
    ];
    Paragraph::new(Line::from(spans)).block(repo_title(app))
}

fn repo_title(app: &App) -> Block<'static> {
    use crate::app::Mode;
    let title = match app.mode {
        Mode::Browse => {
            if app.is_repo {
                "storey — browse"
            } else {
                "storey — not a library"
            }
        }
        Mode::Files => "storey — files",
        Mode::Detail => "storey — tagging",
        Mode::Plugins => "storey — plugins",
    };
    Block::default()
        .borders(Borders::ALL)
        .title(title.to_string())
}

fn bar_len(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_truncates_tail() {
        assert_eq!(fit_path("/a/b/c", 100), "/a/b/c");
        assert!(fit_path("/a/very/long/path", 5).starts_with('…'));
    }
}
