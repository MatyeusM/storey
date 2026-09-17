//! Detail rendering: record panel plus suggestion popup.
//!
//! Kept separate from [`crate::detail`] so both files stay under the
//! size limit; pure drawing over [`App`] state.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use crate::{app::App, detail::Focus, dialog, records};

/// Draw the detail panel plus the suggestion popup when open.
pub fn render_detail(f: &mut ratatui::Frame<'_>, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("tagging".to_string());
    let mut lines = detail_lines(app);
    if lines.is_empty() {
        lines.push(Line::raw("record not found — Esc back to files"));
    }
    f.render_widget(Paragraph::new(lines).block(block), area);
    if app.tagging.suggest_open {
        render_suggest_popup(f, app);
    }
}

/// Body lines for the record, buffers, and known categories.
fn detail_lines(app: &App) -> Vec<Line<'static>> {
    let Some(record) = app.detail_record() else {
        return Vec::new();
    };
    let mut lines = vec![
        Line::raw(format!("name: {}", record.display_name)),
        Line::raw(format!("id: {}  size: {}", record.short_id(), record.size)),
        Line::raw(format!("location: {}", record.location())),
        Line::raw(format!("tags: {}", record.display_tags())),
        Line::raw(mode_line(app)),
        category_line(app),
        Line::raw(format!(
            "values: {}  (v edits, comma-separated)",
            app.tagging.vals
        )),
    ];
    lines.push(categories_line(app));
    lines
}

/// `mode: set|add (m toggles)`.
fn mode_line(app: &App) -> String {
    let mode = if app.tagging.set_mode { "set" } else { "add" };
    format!("mode: {mode}  (m toggles)")
}

/// `categories: actor(2) date(1)` or a none-yet placeholder.
fn categories_line(app: &App) -> Line<'static> {
    let cats = records::categories_with_counts(&app.files.records);
    if cats.is_empty() {
        Line::raw("categories: none yet")
    } else {
        let summary = cats
            .iter()
            .map(|(cat, n)| format!("{cat}({n})"))
            .collect::<Vec<_>>()
            .join(" ");
        Line::raw(format!("categories: {summary}"))
    }
}

/// Category row with ghost completion while editing.
fn category_line(app: &App) -> Line<'static> {
    let editing = app.tagging.focus == Some(Focus::Cat);
    let mut spans = vec![Span::raw(format!("category: {}  ", app.tagging.cat))];
    if editing {
        push_ghost(app, &mut spans);
        spans.push(Span::raw(
            "  (editing: Tab completes, Enter applies)".to_string(),
        ));
    } else {
        spans.push(Span::raw(" (c edits)".to_string()));
    }
    Line::from(spans)
}

/// Dim remainder of the first category match, if any.
fn push_ghost(app: &App, spans: &mut Vec<Span<'static>>) {
    let cats = records::categories_with_counts(&app.files.records);
    let Some(full) = records::complete_category(&cats, &app.tagging.cat) else {
        return;
    };
    let rest = full[app.tagging.cat.len().min(full.len())..].to_string();
    if !rest.is_empty() {
        spans.push(Span::styled(
            rest,
            Style::default().add_modifier(Modifier::DIM),
        ));
    }
}

/// Centered accept popup listing ranked suggestions.
fn render_suggest_popup(f: &mut ratatui::Frame<'_>, app: &App) {
    let area = f.area();
    let height = (app.tagging.suggestions.len() as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(5);
    let rect = dialog::centered(area, 56.min(area.width.saturating_sub(4)).max(30), height);
    f.render_widget(Clear, rect);
    let items: Vec<ListItem<'_>> = app
        .tagging
        .suggestions
        .iter()
        .map(|s| {
            ListItem::new(format!(
                "{}={}  ({:.2} {})",
                s.category, s.value, s.score, s.source
            ))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("suggestions — y applies all, n closes"),
    );
    f.render_widget(list, rect);
}
