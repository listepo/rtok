//! The shell (T15.2): a header line, the tab bar, the page body, a footer. The tabs
//! are the model's page list, never a second one (D23). Body pages arrive with
//! T15.3+ — the two the model serves today render their data minimally, and a page
//! the model adds ahead of its tab falls through to a placeholder that says so.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Paragraph, Row, Table, Tabs};

use super::app::App;

/// One screen: header · tabs · body · footer.
pub(super) fn draw(frame: &mut Frame, app: &App) {
    let [header, tabs, body, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(frame.area());
    frame.render_widget(Paragraph::new(header_line(frame.area())), header);
    frame.render_widget(tab_bar(app), tabs);
    render_page(frame, app, body);
    frame.render_widget(Paragraph::new(footer_line(app)), footer);
}

/// `rtok · <project> · <cols>x<rows>` — what, where, and the window it is in.
fn header_line(area: Rect) -> String {
    format!("rtok · {} · {}x{}", project(), area.width, area.height)
}

/// The project is the current directory's name: the store the model reads is scoped
/// to where the operator sits, so that is what the header names.
fn project() -> String {
    std::env::current_dir()
        .ok()
        .and_then(|dir| dir.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "-".into())
}

/// The model's pages in order, the selected one highlighted.
fn tab_bar(app: &App) -> Tabs<'_> {
    Tabs::new(app.tab_names())
        .select(app.selected())
        .highlight_style(Style::new().bold())
}

/// The selected page's body.
fn render_page(frame: &mut Frame, app: &App, area: Rect) {
    match app.page() {
        "overview" => frame.render_widget(overview(app), area),
        "plugins" => frame.render_widget(plugins_table(app), area),
        page => frame.render_widget(placeholder(page), area),
    }
}

/// The model's Overview page, minimally: the usage totals it serves. Bars, CTT and
/// the sparkline are T15.3.
fn overview(app: &App) -> Paragraph<'static> {
    let usage = &app.snapshot().usage;
    let (input, output) = (usage.input, usage.output);
    let (created, read) = (usage.cache_create, usage.cache_read);
    Paragraph::new(vec![
        Line::from("usage totals (usage rows, all apis)"),
        Line::from(format!("input        {input}")),
        Line::from(format!("output       {output}")),
        Line::from(format!("cache create {created}")),
        Line::from(format!("cache read   {read}")),
    ])
}

/// The model's Plugins page, minimally: the catalogue row per plugin. Toggling and
/// detail are T15.4.
fn plugins_table(app: &App) -> Table<'static> {
    let rows = app.snapshot().plugins.iter().map(|plugin| {
        Row::new([
            plugin.id.to_string(),
            plugin.title.clone(),
            if plugin.enabled { "on" } else { "off" }.to_string(),
            plugin.stats.as_ref().map_or_else(
                || "-".into(),
                |stats| {
                    format!(
                        "{before}->{after} tok ({rows} rows)",
                        before = stats.est_before,
                        after = stats.est_after,
                        rows = stats.rows
                    )
                },
            ),
        ])
    });
    Table::new(
        rows,
        [
            Constraint::Length(9),
            Constraint::Min(24),
            Constraint::Length(4),
            Constraint::Min(24),
        ],
    )
    .header(Row::new(["id", "title", "on", "saved"]))
}

fn placeholder(page: &str) -> Paragraph<'static> {
    Paragraph::new(format!(
        "{page}: this page arrives with T15.3+ (roadmap P15)"
    ))
}

/// Key hints and when the data last came off the model.
fn footer_line(app: &App) -> String {
    let tabs = app.tab_names().len();
    let stamp = crate::log::stamp(app.updated());
    let time = stamp.rsplit_once(' ').map_or("-", |(_, t)| t);
    format!("q quit · Left/Right or 1..{tabs} switch tabs · updated {time} UTC")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::tests::config;
    use crossterm::event::{KeyCode, KeyModifiers};
    use ratatui::backend::TestBackend;

    /// What the loop would put on a real terminal, rendered into a buffer instead.
    fn screen(app: &App) -> String {
        let mut terminal = ratatui::Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| draw(frame, app)).unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect()
    }

    /// The shell paints every tab the model offers, plus the hints and the tick —
    /// the T15.2 chrome — without a terminal.
    #[test]
    fn shell_paints_the_model_tabs_hints_and_tick() {
        let app = App::new(&config());
        let screen = screen(&app);
        for name in app.tab_names() {
            assert!(screen.contains(name), "tab {name} is on screen");
        }
        assert!(screen.contains("q quit"));
        assert!(screen.contains("updated"));
        assert!(screen.contains("UTC"));
    }

    /// Switching tabs changes the body; both pages the model serves have one.
    #[test]
    fn body_follows_the_selected_tab() {
        let mut app = App::new(&config());
        assert!(screen(&app).contains("usage totals"));
        app.key(KeyCode::Right, KeyModifiers::NONE);
        let screen = screen(&app);
        assert!(screen.contains("id"));
        assert!(screen.contains("title"));
    }

    #[test]
    fn header_names_the_project_and_the_window() {
        let line = header_line(Rect::new(0, 0, 120, 40));
        assert!(line.starts_with("rtok · "), "line: {line}");
        assert!(line.ends_with("120x40"), "line: {line}");
    }

    #[test]
    fn footer_hints_the_keys_and_the_last_tick() {
        let app = App::new(&config());
        let line = footer_line(&app);
        assert!(line.contains("q quit"), "line: {line}");
        assert!(line.contains("UTC"), "line: {line}");
    }
}
