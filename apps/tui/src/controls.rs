//! Bottom control bar: one unframed line of key chips + descriptions.
//!
//! Screens register via [`ControlBar::add_control`]; `priority`
//! orders left to right. A chip is ` key ` on cyan with black
//! uppercase text — a keyboard-button hint — followed by its
//! description in normal style.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::progress::Progress;

/// One `key → description` entry; lower `priority` sorts first.
pub struct Control {
    key: String,
    desc: String,
    priority: u8,
}

/// Ordered control set for one frame; see [`for_app`].
#[derive(Default)]
pub struct ControlBar {
    controls: Vec<Control>,
}

impl ControlBar {
    /// Empty bar; chain [`add_control`] to fill it.
    pub fn new() -> Self {
        Self {
            controls: Vec::new(),
        }
    }

    /// Register `key` (shown as an uppercase chip) with `desc`.
    pub fn add_control(&mut self, key: &str, desc: &str, priority: u8) -> &mut Self {
        self.controls.push(Control {
            key: key.to_string(),
            desc: desc.to_string(),
            priority,
        });
        self
    }

    /// One line: chips + descriptions, `notice`, and a right-aligned
    /// progress label while a background op runs.
    pub fn render(&self, notice: &str, progress: Option<&Progress>, width: u16) -> Line<'static> {
        let mut order: Vec<&Control> = self.controls.iter().collect();
        order.sort_by_key(|c| c.priority);
        let mut spans = Vec::new();
        for c in order {
            spans.push(Span::styled(format!(" {} ", c.key.to_uppercase()), chip()));
            spans.push(Span::raw(format!(" {}  ", c.desc)));
        }
        if !notice.is_empty() {
            spans.push(Span::styled(
                format!("· {notice}"),
                Style::default().fg(Color::Yellow),
            ));
        }
        if let Some(p) = progress {
            push_progress(&mut spans, &p.label(), width);
        }
        Line::from(spans)
    }
}

/// Append a progress label padded toward the bar's right end.
fn push_progress(spans: &mut Vec<Span<'static>>, label: &str, width: u16) {
    let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = (width as usize).saturating_sub(used + label.chars().count() + 1);
    spans.push(Span::raw(" ".repeat(pad + 1)));
    spans.push(Span::styled(
        label.to_string(),
        Style::default().fg(Color::Green),
    ));
}

fn chip() -> Style {
    Style::default()
        .bg(Color::Cyan)
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

/// Controls for the current state: dialog, view, or plain dir.
pub fn for_app(app: &crate::app::App) -> ControlBar {
    use crate::app::Mode;
    let mut bar = ControlBar::new();
    if app.add_open {
        dialog_controls(&mut bar);
    } else {
        match app.mode {
            Mode::Files if app.files.rm_open => rm_controls(&mut bar),
            Mode::Files if app.files.rename.is_some() => rename_controls(&mut bar),
            Mode::Files if app.files.filter.open => filter_controls(&mut bar),
            Mode::Files => files_controls(&mut bar),
            Mode::Detail if app.tagging.suggest_open => suggest_controls(&mut bar),
            Mode::Detail if app.tagging.focus.is_some() => edit_controls(&mut bar),
            Mode::Detail => detail_controls(&mut bar),
            Mode::Plugins => plugins_controls(&mut bar),
            Mode::Browse if app.is_repo => repo_controls(&mut bar),
            Mode::Browse => plain_controls(&mut bar),
        }
    }
    bar
}

fn dialog_controls(bar: &mut ControlBar) {
    bar.add_control("↑↓/jk", "pick", 1)
        .add_control("enter", "add", 2)
        .add_control("esc", "cancel", 3);
}

fn repo_controls(bar: &mut ControlBar) {
    bar.add_control("←→/hjkl", "tags", 1)
        .add_control("enter", "remove/add", 2)
        .add_control("a", "add", 3)
        .add_control("x", "delete", 4)
        .add_control("u", "up", 5)
        .add_control("t", "files", 6)
        .add_control("r", "restore", 7)
        .add_control("q", "quit", 8);
}

fn files_controls(bar: &mut ControlBar) {
    bar.add_control("↑↓/jk", "move", 1)
        .add_control("enter", "tag", 2)
        .add_control("u/U", "update/verify", 3)
        .add_control("F", "filter", 4)
        .add_control("X", "clear filter", 5)
        .add_control("d", "remove", 6)
        .add_control("m", "rename", 7)
        .add_control("esc", "back", 8)
        .add_control("q", "quit", 9);
}

fn rm_controls(bar: &mut ControlBar) {
    bar.add_control("y", "delete file", 1)
        .add_control("n", "cancel", 2);
}

fn rename_controls(bar: &mut ControlBar) {
    bar.add_control("type", "new name", 1)
        .add_control("enter", "apply", 2)
        .add_control("esc", "cancel", 3);
}

fn filter_controls(bar: &mut ControlBar) {
    bar.add_control("tab", "switch mode", 1)
        .add_control("type", "query", 2)
        .add_control("enter", "close", 3)
        .add_control("esc", "close", 4);
}

fn detail_controls(bar: &mut ControlBar) {
    bar.add_control("c/v", "edit", 1)
        .add_control("m", "set/add", 2)
        .add_control("enter", "apply", 3)
        .add_control("s", "suggest", 4)
        .add_control("P", "plugins", 5)
        .add_control("X", "remove category", 6)
        .add_control("esc", "back", 7)
        .add_control("q", "quit", 8);
}

fn edit_controls(bar: &mut ControlBar) {
    bar.add_control("type", "input", 1)
        .add_control("tab", "complete", 2)
        .add_control("enter", "apply", 3)
        .add_control("esc", "unfocus", 4);
}

fn suggest_controls(bar: &mut ControlBar) {
    bar.add_control("y", "apply all", 1)
        .add_control("n", "close", 2);
}

fn plugins_controls(bar: &mut ControlBar) {
    bar.add_control("↑↓/jk", "move", 1)
        .add_control("enter", "open folder", 2)
        .add_control("esc", "back", 3)
        .add_control("q", "quit", 4);
}

fn plain_controls(bar: &mut ControlBar) {
    bar.add_control("↑↓/jk", "move", 1)
        .add_control("enter", "open", 2)
        .add_control("u", "up", 3)
        .add_control("i", "init", 4)
        .add_control("q", "quit", 5);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_orders_chips() {
        let mut bar = ControlBar::new();
        bar.add_control("b", "second", 2)
            .add_control("a", "first", 1);
        let line = bar.render("", None, 0);
        let text: String = line.spans.iter().map(|s| s.content.clone()).collect();
        assert!(text.find('A').unwrap() < text.find('B').unwrap());
    }

    #[test]
    fn chip_is_padded_uppercase() {
        let mut bar = ControlBar::new();
        bar.add_control("q", "quit", 1);
        let line = bar.render("", None, 0);
        assert_eq!(line.spans[0].content, " Q ");
        assert!(line.spans[1].content.starts_with(" quit"));
    }

    #[test]
    fn progress_pads_toward_right_end() {
        use crate::progress::Progress;
        let bar = ControlBar::new();
        let done = Progress {
            op: "hash".into(),
            done: 1,
            total: Some(2),
        };
        let line = bar.render("", Some(&done), 40);
        let text: String = line.spans.iter().map(|s| s.content.clone()).collect();
        assert!(text.ends_with("hash 50% [#####-----]"));
        assert_eq!(text.chars().count(), 40);
    }

    #[test]
    fn notice_appends_yellow_tail() {
        let bar = ControlBar::new();
        let line = bar.render("boom", None, 0);
        assert!(line.spans.iter().any(|s| s.content.contains("boom")));
    }
}
