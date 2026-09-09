//! The shell (T15.2): a header line, the tab bar, the page body, a footer. The tabs
//! are the model's page list, never a second one (D23). The Overview tab renders CTT,
//! per-plugin savings bars and a per-turn sparkline off the snapshot (T15.3); the
//! Plugins page renders minimally until T15.4, and a page the model adds ahead of its
//! tab falls through to a placeholder that says so.

use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Row, Sparkline, Table, Tabs};

use super::app::App;
use crate::web::model::PluginPage;

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
        "overview" => render_overview(frame, app, area),
        "plugins" => frame.render_widget(plugins_table(app), area),
        page => frame.render_widget(placeholder(page), area),
    }
}

/// The model's Overview page (T15.3): usage totals, CTT, one savings bar per measured
/// plugin, and the per-turn sparkline — every number off the snapshot the model served
/// (D23), never a second query.
fn render_overview(frame: &mut Frame, app: &App, area: Rect) {
    let [top, spark] = Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(area);
    frame.render_widget(overview_text(app), top);
    let data: Vec<u64> = app
        .snapshot()
        .usage
        .turns
        .iter()
        .map(|t| (*t).max(0) as u64)
        .collect();
    let title = if data.is_empty() {
        "ctx tokens per turn (no usage rows yet)".to_string()
    } else {
        format!("ctx tokens per turn (last {} turns)", data.len())
    };
    frame.render_widget(
        Sparkline::default()
            .block(Block::default().title(title))
            .data(&data),
        spark,
    );
}

/// Totals, CTT and the bars as text; the sparkline is the widget below.
fn overview_text(app: &App) -> Paragraph<'static> {
    let usage = &app.snapshot().usage;
    let totals = &usage.totals;
    let mut lines = vec![
        Line::from("usage totals (usage rows, all apis)"),
        Line::from(format!("input        {}", totals.input)),
        Line::from(format!("output       {}", totals.output)),
        Line::from(format!("cache create {}", totals.cache_create)),
        Line::from(format!("cache read   {}", totals.cache_read)),
        Line::from(format!(
            "ctt          {} (input-side tok × turns after)",
            usage.ctt
        )),
        Line::from("saved by plugin (Measurement rows)"),
    ];
    lines.extend(savings_lines(&app.snapshot().plugins));
    Paragraph::new(lines)
}

/// One bar per plugin that measured a saving, busiest saver first, each scaled to the
/// largest — the longest bar always fills [`BAR_WIDTH`]. A plugin that saved nothing
/// is T22.5's finding, not a bar; with no measured savings the tab says so instead of
/// drawing zeros.
fn savings_lines(plugins: &[PluginPage]) -> Vec<Line<'static>> {
    let mut saved: Vec<(&str, i64)> = plugins
        .iter()
        .filter_map(|p| p.stats.as_ref().map(|s| (p.id, s.est_before - s.est_after)))
        .filter(|(_, saved)| *saved > 0)
        .collect();
    if saved.is_empty() {
        return vec![Line::from("no measured savings yet")];
    }
    saved.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let max = saved.iter().map(|(_, s)| *s).max().unwrap_or(0);
    saved
        .into_iter()
        .map(|(id, s)| Line::from(format!("{id:9} {} {s} tok", bar(s, max))))
        .collect()
}

/// `█` scaled to the largest saving, so any positive saving draws at least one block.
/// Zero, negative and empty-maximum inputs draw nothing.
fn bar(saved: i64, max: i64) -> String {
    if saved <= 0 || max <= 0 {
        return String::new();
    }
    let full = (saved as u64)
        .saturating_mul(BAR_WIDTH as u64)
        .div_ceil(max as u64) as usize;
    "█".repeat(full)
}

/// The longest savings bar, in blocks. Fits beside the id and the count on an
/// 80-column terminal with room to spare.
const BAR_WIDTH: usize = 16;

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
    use crate::config::Config;
    use crate::plugin::{Measurement, Runtime};
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

    /// T15.3: the Overview tab shows the snapshot's numbers — totals, CTT, one bar per
    /// measured plugin, the sparkline — so the tab, `rtok stats --json` and the web
    /// Overview agree on one store (Gate P15). The values are five digits, so the match
    /// is the number, not a digit the chrome happens to contain.
    #[test]
    fn overview_tab_renders_the_snapshot_numbers() {
        let app = App::new(&seeded());
        let snap = app.snapshot();
        assert_eq!(snap.usage.totals.input, 11_540);
        assert_eq!(snap.usage.ctt, 1_500);
        let screen = screen(&app);
        for n in [
            snap.usage.totals.input,
            snap.usage.totals.output,
            snap.usage.totals.cache_create,
            snap.usage.totals.cache_read,
            snap.usage.ctt,
        ] {
            assert!(screen.contains(&n.to_string()), "{n} is on screen");
        }
        assert!(
            screen.contains("ctx tokens per turn"),
            "the sparkline title"
        );
        assert!(
            "▁▂▃▄▅▆▇█".chars().any(|c| screen.contains(c)),
            "the sparkline draws blocks"
        );
        // Two measured plugins: `cmd` saved 30 (the full bar), `read` 15 (half).
        for (id, count, blocks) in [
            ("cmd", "30 tok", BAR_WIDTH),
            ("read", "15 tok", BAR_WIDTH / 2),
        ] {
            assert!(screen.contains(id), "{id} has a bar");
            assert!(screen.contains(count), "{id} saved {count}");
            assert!(
                screen.contains(&"█".repeat(blocks)),
                "{id} draws {blocks} blocks"
            );
        }
    }

    /// The bar's guards: nothing saved is no bar.
    #[test]
    fn bar_draws_nothing_for_nothing_saved() {
        assert_eq!(bar(0, 10), "");
        assert_eq!(bar(-3, 10), "");
        assert_eq!(bar(5, 0), "");
        assert_eq!(bar(1, 1_000_000), "█", "any positive saving draws");
    }

    /// A config whose store holds two sessions, three turns and two measured plugins,
    /// so the Overview tab has numbers worth rendering. Its own temp dir, not the
    /// shared `config()` one: seeding writes while other tests hold the shared store
    /// open.
    fn seeded() -> Config {
        let dir = std::env::temp_dir().join(format!("rtok-tui-overview-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cfg = Config::load_from(&dir).expect("config");
        {
            let store = crate::store::Store::open(&cfg.core.db_path).expect("seed store");
            store.insert_proxy_turn("a", 1_500, 0, 0, 1_111).unwrap();
            store.insert_proxy_turn("a", 40, 0, 1_200, 2_222).unwrap();
            store
                .insert_proxy_turn("b", 10_000, 2_000, 0, 3_333)
                .unwrap();
        }
        let rt = Runtime::open(cfg.clone(), "seed").expect("seed runtime");
        for (plugin, before, after) in [("cmd", 30, 0), ("read", 30, 15)] {
            rt.record(&Measurement {
                plugin,
                kind: "filter",
                before_bytes: 100,
                after_bytes: 40,
                est_before: before,
                est_after: after,
                ref_id: None,
                call_id: None,
            })
            .unwrap();
        }
        cfg
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
