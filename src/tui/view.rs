//! The screen. T15.1 renders one placeholder line that exercises the loop's data —
//! tabs, current page, tick; the header · tabs · footer shell that gives the TUI its
//! shape lands with T15.2, the pages with T15.3+.

use ratatui::Frame;
use ratatui::widgets::Paragraph;

use super::app::App;

pub(super) fn draw(frame: &mut Frame, app: &App) {
    let tabs = app.tab_names();
    let tick = crate::log::stamp(app.updated());
    frame.render_widget(
        Paragraph::new(format!(
            "rtok tui — {} tab(s): {} · on {} · tick {} UTC · {} plugin rows",
            tabs.len(),
            tabs.join(", "),
            app.page(),
            tick.rsplit_once(' ').map_or("-", |(_, t)| t),
            app.snapshot().plugins.len(),
        )),
        frame.area(),
    );
}
