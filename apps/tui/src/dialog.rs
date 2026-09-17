//! Centered add-tag dialog: pick an unused category to append.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, List, ListItem},
};

use crate::app::App;

/// Area of the dialog for clicks; `None` when closed.
pub fn dialog_rect(area: Rect, choices: usize) -> Option<Rect> {
    if choices == 0 {
        return None;
    }
    let width = 32.min(area.width.saturating_sub(4)).max(20);
    let height = (choices as u16 + 4)
        .min(area.height.saturating_sub(4))
        .max(5);
    Some(centered(area, width, height))
}

/// Row index inside the dialog for a terminal click, if any.
pub fn row_at(rect: Rect, col: u16, row: u16, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    let inner_top = rect.y + 2;
    let inner_left = rect.x + 1;
    let inner_right = rect.x + rect.width.saturating_sub(1);
    if col < inner_left || col >= inner_right {
        return None;
    }
    if row < inner_top {
        return None;
    }
    let idx = (row - inner_top) as usize;
    if idx < len { Some(idx) } else { None }
}

/// Render the add dialog over the current frame.
pub fn render(f: &mut ratatui::Frame<'_>, app: &App, choices: &[String]) {
    let area = f.area();
    let Some(rect) = dialog_rect(area, choices.len()) else {
        return;
    };
    f.render_widget(Clear, rect);
    let items: Vec<ListItem<'_>> = choices
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let item = ListItem::new(c.as_str());
            if i == app.add_sel {
                item.style(Style::default().add_modifier(Modifier::REVERSED))
            } else {
                item
            }
        })
        .collect();
    let list = List::new(items).block(Block::default().borders(Borders::ALL).title("add tag"));
    f.render_widget(list, rect);
}

pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 24,
        }
    }

    #[test]
    fn rect_centers_and_hits_rows() {
        let rect = dialog_rect(area(), 3).unwrap();
        assert!(rect.x > 0 && rect.y > 0);
        let first = row_at(rect, rect.x + 2, rect.y + 2, 3);
        assert_eq!(first, Some(0));
        assert_eq!(row_at(rect, 0, 0, 3), None);
    }

    #[test]
    fn empty_has_no_rect() {
        assert!(dialog_rect(area(), 0).is_none());
    }
}
