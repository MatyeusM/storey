//! Browse file list: plain-directory rows with a selection marker.
//!
//! Split from [`crate::ui`] so the draw dispatcher stays small.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;

/// Plain-dir rows; only plain dirs highlight a selection.
pub fn file_list(app: &App) -> List<'_> {
    let highlight = !app.is_repo;
    let items: Vec<ListItem<'_>> = app
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| ListItem::new(row_line(e, highlight && i == app.selected)))
        .collect();
    List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("{} entries", app.entries.len())),
    )
}

/// One browser row: two-space indent; the selected row gets a white
/// `>` marker and cyan text instead of a highlighted background.
fn row_line(e: &crate::app::Entry, selected: bool) -> Line<'static> {
    let text = format!("{}{}", e.name, if e.is_dir { "/" } else { "" });
    if selected {
        Line::from(vec![
            Span::styled("> ", Style::default()),
            Span::styled(text, Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![Span::raw("  "), Span::raw(text)])
    }
}
