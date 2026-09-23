//! T226: the one palette and the styles every page draws with. Colours are the
//! terminal's own ANSI names, never RGB, so the TUI follows the operator's theme —
//! light or dark — instead of fighting it.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Row};

/// Selection, badges, table headers and the savings bars.
pub(super) const ACCENT: Color = Color::Cyan;
/// Frames, labels, secondary text.
pub(super) const MUTED: Color = Color::DarkGray;
/// A live session, a plugin that is on.
pub(super) const OK: Color = Color::Green;
/// Alerts, filters that hide rows, a never-invoked skill.
pub(super) const WARN: Color = Color::Yellow;
/// The store error banner, a failed call.
pub(super) const ERR: Color = Color::Red;

pub(super) fn muted() -> Style {
    Style::new().fg(MUTED)
}

pub(super) fn title() -> Style {
    Style::new().fg(ACCENT).bold()
}

/// The ` rtok ` badge in the header and the selected tab.
pub(super) fn badge() -> Style {
    Style::new().fg(Color::Black).bg(ACCENT).bold()
}

/// The cursor row of every table: the same badge, full width.
pub(super) fn selected() -> Style {
    badge()
}

/// A rounded, muted frame with one column of padding — the body and the `?` overlay.
pub(super) fn frame() -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(muted())
        .padding(Padding::horizontal(1))
}

/// [`frame`] with a title.
pub(super) fn pane(title: impl Into<String>) -> Block<'static> {
    frame().title(Line::styled(format!(" {} ", title.into()), self::title()))
}

/// A one-row divider with a title: a top border only, so a detail pane costs the same
/// height a bare title did.
pub(super) fn section(title: impl Into<String>) -> Block<'static> {
    Block::new()
        .borders(Borders::TOP)
        .border_style(muted())
        .title(Line::styled(format!(" {} ", title.into()), self::title()))
}

/// A table header: accent, bold, no extra row.
pub(super) fn header<'a>(cells: impl IntoIterator<Item = &'a str>) -> Row<'static> {
    Row::new(cells.into_iter().map(str::to_owned)).style(title())
}

/// Key hints off the KEYS table (T60.8): `key description` pairs, the key in accent.
pub(super) fn hints(keys: &[(&'static str, &'static str)]) -> Line<'static> {
    let mut spans = Vec::with_capacity(keys.len() * 3);
    for (i, (key, desc)) in keys.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(*key, title()));
        spans.push(Span::styled(format!(" {desc}"), muted()));
    }
    Line::from(spans)
}

/// One banner line: a coloured glyph, then the message in bold.
pub(super) fn banner(glyph: char, msg: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{glyph} "), Style::new().fg(color).bold()),
        Span::styled(msg.to_owned(), Style::new().bold()),
    ])
}
