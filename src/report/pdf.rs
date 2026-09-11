//! The PDF rendering of [`Document`](super::Document) (T22.3): the same eight
//! sections in the same order, paged on A4 with a contents page and viewer
//! bookmarks. It formats; it computes nothing (D24).
//!
//! The renderer is `printpdf` (T22.0). `svg2pdf` is deliberately not linked: it
//! returns standalone one-page PDFs, and printpdf 0.12 cannot place a PDF page
//! onto another page — merging the two would be byte-level PDF surgery, far
//! outside this task. The charts are the same series drawn natively instead:
//! identical pairs, order and titles as the HTML bars. Streams stay
//! uncompressed, so every figure is greppable in the artifact (the T22.3
//! parity test reads the headings straight back out of the bytes).

use super::Document;
use super::markdown::ms;
use printpdf::{
    BuiltinFont, Color, Mm, Op, PaintMode, PdfDocument, PdfFontHandle, PdfPage, PdfSaveOptions,
    Point, Pt, Rect, Rgb, TextItem,
};

const TOP: f32 = 282.0;
const BOTTOM: f32 = 15.0;
const LEFT: f32 = 15.0;
const LEAD: f32 = 4.6;
const BODY: f32 = 9.5;
const WIDTH: usize = 100;
/// Body lines per page; headings and charts reserve whole lines (never split).
const LINES: usize = ((TOP - BOTTOM) / LEAD) as usize;

/// One P22 section: the fixed title plus its content blocks. Titles are the
/// same strings as the HTML `<h2>`s, in the same order — the parity test pins
/// that, and the recommendations body comes from the document (T22.5 flows
/// through with no change here).
struct Sec {
    title: &'static str,
    blocks: Vec<Block>,
}

enum Block {
    Para(String),
    Chart {
        title: String,
        pairs: Vec<(String, i64)>,
    },
}

/// Render the whole document to PDF bytes (A4, Helvetica only — no font files
/// ship, which is why the T22.0 survey picked this renderer).
pub fn render(doc: &Document) -> Vec<u8> {
    let secs = sections(doc);
    // Paginate first so the contents page knows every section's page number.
    let mut starts = Vec::with_capacity(secs.len());
    let mut pages: Vec<Vec<PageItem>> = vec![Vec::new()];
    for sec in &secs {
        let mut items = vec![PageItem::Heading(sec.title)];
        for b in &sec.blocks {
            match b {
                Block::Para(p) => {
                    for line in wrap(p, WIDTH) {
                        items.push(PageItem::Line(line));
                    }
                }
                Block::Chart { title, pairs } => items.push(PageItem::Chart {
                    title: title.clone(),
                    pairs: pairs.clone(),
                }),
            }
        }
        let mut iter = items.into_iter().peekable();
        let mut recorded = false;
        while iter.peek().is_some() {
            let rest = LINES - pages.last().expect("page").len();
            // A heading never stands alone at the bottom of a page.
            let take = chunk(&mut iter, rest);
            if take.is_empty() {
                pages.push(Vec::new());
            } else {
                if !recorded {
                    starts.push(pages.len() - 1);
                    recorded = true;
                }
                pages.last_mut().expect("page").extend(take);
            }
        }
    }
    let mut doc_pdf = PdfDocument::new("rtok report");
    doc_pdf.add_bookmark("rtok report", 0);
    for (sec, start) in secs.iter().zip(&starts) {
        doc_pdf.add_bookmark(sec.title, start + 2);
    }
    let toc = toc_page(doc, &secs, &starts);
    let mut out = vec![toc];
    for items in pages {
        out.push(content_page(items));
    }
    let mut warnings = Vec::new();
    doc_pdf.with_pages(out).save(
        &PdfSaveOptions {
            optimize: false,
            ..Default::default()
        },
        &mut warnings,
    )
}

/// Take up to `rest` line-costs of blocks; a chart is atomic (never split).
fn chunk<I: Iterator<Item = PageItem>>(
    iter: &mut std::iter::Peekable<I>,
    rest: usize,
) -> Vec<PageItem> {
    let mut take = Vec::new();
    let mut cost = 0;
    while let Some(next) = iter.peek() {
        let c = next.cost();
        if cost + c > rest {
            // An orphan heading moves with the block it introduces.
            if take.len() == 1 && matches!(take[0], PageItem::Heading(_)) {
                return Vec::new();
            }
            break;
        }
        cost += c;
        take.push(iter.next().expect("peeked"));
    }
    take
}

#[derive(Clone)]
enum PageItem {
    Heading(&'static str),
    Line(String),
    Chart {
        title: String,
        pairs: Vec<(String, i64)>,
    },
}

impl PageItem {
    fn cost(&self) -> usize {
        match self {
            PageItem::Heading(_) => 3,
            PageItem::Line(_) => 1,
            PageItem::Chart { pairs, .. } => pairs.len() + 2,
        }
    }
}

/// Page 0: title, window, and the contents list with page numbers.
fn toc_page(doc: &Document, secs: &[Sec], starts: &[usize]) -> PdfPage {
    let w = &doc.ledgers.window;
    let mut ops = Vec::new();
    let mut y = TOP;
    show(
        &mut ops,
        LEFT,
        y,
        BuiltinFont::HelveticaBold,
        18.0,
        "rtok report",
    );
    y -= 9.0;
    show(
        &mut ops,
        LEFT,
        y,
        BuiltinFont::Helvetica,
        BODY,
        &format!("Window {} -> {} ({})", w.from_date, w.to_date, w.since),
    );
    y -= 10.0;
    show(
        &mut ops,
        LEFT,
        y,
        BuiltinFont::HelveticaBold,
        13.0,
        "Contents",
    );
    y -= 8.0;
    for (sec, start) in secs.iter().zip(starts) {
        show(
            &mut ops,
            LEFT,
            y,
            BuiltinFont::Helvetica,
            BODY,
            &format!("{}  {}", sec.title, start + 1),
        );
        y -= LEAD;
    }
    PdfPage::new(Mm(210.0), Mm(297.0), ops)
}

/// One content page: headings, wrapped lines, and native bar charts.
fn content_page(items: Vec<PageItem>) -> PdfPage {
    let mut ops = Vec::new();
    let mut y = TOP;
    for item in items {
        match item {
            PageItem::Heading(t) => {
                y -= 2.0;
                show(&mut ops, LEFT, y, BuiltinFont::HelveticaBold, 13.0, t);
                y -= 7.0;
            }
            PageItem::Line(l) => {
                show(&mut ops, LEFT, y, BuiltinFont::Helvetica, BODY, &l);
                y -= LEAD;
            }
            PageItem::Chart { title, pairs } => {
                show(&mut ops, LEFT, y, BuiltinFont::HelveticaBold, 10.0, &title);
                y -= 6.0;
                y = bars(&mut ops, y, &pairs);
            }
        }
    }
    PdfPage::new(Mm(210.0), Mm(297.0), ops)
}

/// The HTML bars' geometry in millimetres: same pairs, same order, same
/// title — a bar per pair scaled to the maximum, label left, value right.
fn bars(ops: &mut Vec<Op>, mut y: f32, pairs: &[(String, i64)]) -> f32 {
    let shares = super::bar_shares(pairs);
    for ((label, v), share) in pairs.iter().zip(shares.iter()) {
        // Truncate on chars, not bytes: plugin ids come from external plugins
        // and a byte cut can land inside a multi-byte UTF-8 char.
        let label: String = if label.chars().count() > 26 {
            label.chars().take(26).collect()
        } else {
            label.clone()
        };
        show(ops, LEFT, y, BuiltinFont::Helvetica, 8.0, &label);
        let w = *share as f32 * 90.0;
        ops.push(Op::SetFillColor { col: gray() });
        ops.push(Op::DrawRectangle {
            rectangle: Rect {
                x: Pt(mm(63.0)),
                y: Pt(mm(y - 3.0)),
                width: Pt(mm(w.max(0.6))),
                height: Pt(mm(3.2)),
                mode: Some(PaintMode::Fill),
                winding_order: None,
            },
        });
        show(
            ops,
            63.0 + w + 2.0,
            y,
            BuiltinFont::Helvetica,
            8.0,
            &v.to_string(),
        );
        y -= 5.2;
    }
    y -= 2.0;
    y
}

/// The eight sections, in the fixed P22 order, straight from the document.
fn sections(doc: &Document) -> Vec<Sec> {
    let w = &doc.ledgers.window;
    let mut secs = Vec::with_capacity(8);
    let mut window = vec![
        format!(
            "calls: {} of {} in window ({})",
            w.calls_in_window, w.calls_total, w.since
        ),
        format!(
            "measurements: {} (whole ledger, no row times)",
            w.measurements
        ),
        format!("usage: {} (whole ledger, no row times)", w.usage),
    ];
    if w.calls_total == 0 && w.measurements == 0 && w.usage == 0 {
        window.push("No rows in window. The store has no rows to report.".into());
    }
    secs.push(Sec {
        title: "Window",
        blocks: paras(window),
    });

    let sav = &doc.ledgers.savings;
    let mut savings =
        vec!["Estimated tokens from Measurement rows only (est_before - est_after, net).".into()];
    if sav.rows.is_empty() {
        savings.push("No rows in window.".into());
    } else {
        for r in &sav.rows {
            savings.push(format!(
                "{}: {} rows, est {} -> {}, saved {}",
                r.plugin, r.rows, r.est_before, r.est_after, r.saved
            ));
        }
        savings.push(format!(
            "Total: {} est tokens over {} Measurement rows.",
            sav.total_saved, sav.total_rows
        ));
    }
    let mut blocks = paras(savings);
    if !sav.rows.is_empty() {
        blocks.push(chart(
            "saved tokens per plugin",
            sav.rows.iter().map(|r| (r.plugin.clone(), r.saved)),
        ));
    }
    secs.push(Sec {
        title: "Savings",
        blocks,
    });

    let calls = &doc.ledgers.calls;
    let mut lines = vec!["Latency per surface, nearest-rank p50/p95 over timed calls.".into()];
    if calls.total == 0 {
        lines.push("No rows in window.".into());
    } else {
        for r in &calls.rows {
            lines.push(format!(
                "{}: {} calls, {} timed, p50 {} ms, p95 {} ms",
                r.surface,
                r.calls,
                r.timed,
                ms(r.p50_ms),
                ms(r.p95_ms)
            ));
        }
        lines.push(format!(
            "{} of {} calls rows in window ({}).",
            calls.in_window, calls.total, w.since
        ));
    }
    let mut blocks = paras(lines);
    if calls.total > 0 {
        blocks.push(chart(
            "calls per surface",
            calls
                .rows
                .iter()
                .map(|r| (r.surface.clone(), r.calls as i64)),
        ));
    }
    secs.push(Sec {
        title: "Calls",
        blocks,
    });

    let cache = &doc.ledgers.cache;
    let mut lines = vec!["Prompt-cache busts by cause, from the proxy usage rows.".into()];
    if cache.sessions == 0 {
        lines.push("No rows in window.".into());
    } else {
        for (cause, n) in &cache.by_cause {
            lines.push(format!("{cause}: {n}"));
        }
        lines.push(format!(
            "Busts: {} over {} turns in {} session(s).",
            cache.busts, cache.turns, cache.sessions
        ));
    }
    let mut blocks = paras(lines);
    if cache.sessions > 0 {
        blocks.push(chart(
            "busts per cause",
            cache.by_cause.iter().map(|(c, n)| (c.clone(), *n as i64)),
        ));
    }
    secs.push(Sec {
        title: "Cache",
        blocks,
    });

    let exp = &doc.ledgers.expand;
    let expand = if exp.decisions == 0 {
        vec!["No rows in window.".into()]
    } else {
        let what = if exp.expanded_ids.is_empty() {
            "none".into()
        } else {
            exp.expanded_ids.join(", ")
        };
        vec![format!(
            "rtok expand froze {} of {} live-zone pointers ({:.1}%). Expanded: {}.",
            exp.expanded,
            exp.decisions,
            100.0 * exp.rate,
            what
        )]
    };
    secs.push(Sec {
        title: "Expand",
        blocks: paras(expand),
    });

    let mut config: Vec<String> =
        vec!["Every effective key with its origin (config show --sources).".into()];
    for e in &doc.config {
        config.push(format!("{} = {} ({})", e.key, e.value, e.source));
    }
    config.push(format!("{} keys.", doc.config.len()));
    secs.push(Sec {
        title: "Config",
        blocks: paras(config),
    });

    let mut doctor = vec!["Live probes (the rtok doctor page), not store rows.".into()];
    doctor.extend(doc.doctor.to_text().lines().map(str::to_string));
    secs.push(Sec {
        title: "Doctor",
        blocks: paras(doctor),
    });

    let reco = if doc.recommendations.is_empty() {
        vec!["No recommendations.".into()]
    } else {
        doc.recommendations
            .iter()
            .map(|r| format!("{}: {} ({})", r.rule, r.finding, r.evidence))
            .collect()
    };
    secs.push(Sec {
        title: "Recommendations",
        blocks: paras(reco),
    });
    secs
}

fn paras(lines: Vec<String>) -> Vec<Block> {
    lines.into_iter().map(Block::Para).collect()
}

fn chart(title: &str, pairs: impl Iterator<Item = (String, i64)>) -> Block {
    Block::Chart {
        title: title.into(),
        pairs: pairs.collect(),
    }
}

/// One text line at `(x, y)` millimetres from the bottom-left corner.
fn show(ops: &mut Vec<Op>, x: f32, y: f32, font: BuiltinFont, size: f32, text: &str) {
    ops.push(Op::StartTextSection);
    ops.push(Op::SetTextCursor {
        pos: Point::new(Mm(x), Mm(y)),
    });
    ops.push(Op::SetFont {
        font: PdfFontHandle::Builtin(font),
        size: Pt(size),
    });
    ops.push(Op::SetLineHeight { lh: Pt(size) });
    ops.push(Op::ShowText {
        items: vec![TextItem::Text(ansi(text))],
    });
    ops.push(Op::EndTextSection);
}

fn gray() -> Color {
    Color::Rgb(Rgb {
        r: 0.25,
        g: 0.25,
        b: 0.25,
        icc_profile: None,
    })
}

fn mm(x: f32) -> f32 {
    x * 2.834_645_7
}

/// Word wrap to `width` columns; overlong words split. Callers pass one
/// paragraph each; the page flow never splits a chart.
fn wrap(s: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in s.split(' ') {
        for piece in split_word(word, width) {
            if !cur.is_empty() && cur.len() + 1 + piece.len() > width {
                lines.push(std::mem::take(&mut cur));
            }
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(&piece);
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn split_word(word: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    if chars.len() <= width {
        return vec![word.to_string()];
    }
    // Chunk on chars, not bytes: a byte chunk can cut a multi-byte UTF-8 char
    // in half, which `from_utf8_lossy` then replaces with U+FFFD.
    chars.chunks(width).map(|c| c.iter().collect()).collect()
}

/// Builtin Helvetica is WinAnsi: map the punctuation the report emits, pass
/// Latin-1 through, and degrade the rest to `?` rather than emitting bytes no
/// viewer can shape. Tabs and carriage returns would corrupt the text cursor.
fn ansi(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '→' => "->".to_string(),
            '—' | '–' => "-".to_string(),
            '·' => "-".to_string(),
            '…' => "...".to_string(),
            '‘' | '’' => "'".to_string(),
            '“' | '”' => "\"".to_string(),
            '\t' => " ".to_string(),
            '\r' => String::new(),
            c if c.is_ascii() => c.to_string(),
            c if (c as u32) < 256 && !c.is_control() => c.to_string(),
            _ => "?".to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rstest::rstest;

    use super::*;
    use crate::report::Document;
    use crate::web::model::*;

    /// A label longer than 26 chars, all multi-byte, must not panic when
    /// `bars` truncates it (regression: byte slicing used to land mid-char).
    #[rstest]
    fn bars_truncates_long_multibyte_label_without_panicking() {
        let mut ops = Vec::new();
        let label = "é".repeat(30);
        let pairs = vec![(label, 5i64)];
        bars(&mut ops, TOP, &pairs);
    }

    #[rstest]
    fn split_word_splits_on_chars_not_bytes() {
        assert_eq!(split_word("ééééé", 2), vec!["éé", "éé", "é"]);
    }

    #[rstest]
    fn split_word_leaves_short_word_unchanged() {
        assert_eq!(split_word("short", 26), vec!["short"]);
    }

    fn positions(hay: &[u8], needle: &str) -> Vec<usize> {
        let n = needle.as_bytes();
        hay.windows(n.len())
            .enumerate()
            .filter(|(_, w)| *w == n)
            .map(|(i, _)| i)
            .collect()
    }

    fn object_offset(pdf: &[u8], id: u32) -> usize {
        let needle = format!("{id} 0 obj");
        positions(pdf, &needle)
            .into_iter()
            .find(|&pos| pos == 0 || !pdf[pos - 1].is_ascii_digit())
            .unwrap_or_else(|| panic!("object {id}"))
    }

    fn page_content_starts(pdf: &[u8]) -> Vec<usize> {
        page_object_ids(pdf)
            .iter()
            .map(|page_id| {
                let page_off = object_offset(pdf, *page_id);
                let page_end = page_off
                    + pdf[page_off..]
                        .windows(6)
                        .position(|w| w == b"endobj")
                        .expect("page endobj");
                let page_body = String::from_utf8_lossy(&pdf[page_off..page_end]);
                let contents_id = page_body
                    .split("/Contents")
                    .nth(1)
                    .and_then(|tail| {
                        tail.chars()
                            .skip_while(|c| !c.is_ascii_digit())
                            .take_while(|c| c.is_ascii_digit())
                            .collect::<String>()
                            .parse::<u32>()
                            .ok()
                    })
                    .expect("page contents ref");
                object_offset(pdf, contents_id)
            })
            .collect()
    }

    fn pdf_page_index(pdf: &[u8], byte_offset: usize) -> usize {
        let starts = page_content_starts(pdf);
        starts
            .iter()
            .rposition(|&start| start <= byte_offset)
            .unwrap_or(0)
    }

    fn toc_page_num(pdf: &[u8], title: &str) -> usize {
        let needle = format!("({title}  ");
        let pos = positions(pdf, &needle)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("toc entry for {title}"));
        let tail = String::from_utf8_lossy(&pdf[pos..pos + 32]);
        tail.chars()
            .skip_while(|c| !c.is_ascii_digit())
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or_else(|_| panic!("toc page digits for {title}: {tail:?}"))
    }

    fn body_heading_page(pdf: &[u8], title: &str) -> usize {
        let needle = format!("({title})");
        let pos = positions(pdf, &needle)
            .into_iter()
            .last()
            .unwrap_or_else(|| panic!("body heading for {title}"));
        pdf_page_index(pdf, pos)
    }

    fn utf16be_title_hex(title: &str) -> String {
        let mut hex = String::from("FEFF");
        for unit in title.encode_utf16() {
            hex.push_str(&format!("{:04X}", unit));
        }
        hex
    }

    fn page_object_ids(pdf: &[u8]) -> Vec<u32> {
        let text = String::from_utf8_lossy(pdf);
        let kids = text
            .find("/Kids[")
            .or_else(|| text.find("/Kids ["))
            .expect("page tree Kids");
        let bracket = text[kids..]
            .find(']')
            .expect("page tree Kids closing bracket");
        let slice = &text[kids..kids + bracket];
        let mut ids = Vec::new();
        let mut at = 0usize;
        while let Some(rel) = slice[at..].find(" 0 R") {
            let end = at + rel;
            let mut start = end;
            while start > 0 && slice.as_bytes()[start - 1].is_ascii_digit() {
                start -= 1;
            }
            ids.push(slice[start..end].parse::<u32>().expect("page tree kid id"));
            at = end + 4;
        }
        ids
    }

    fn bookmark_page(pdf: &[u8], title: &str) -> usize {
        let page_ids = page_object_ids(pdf);
        let text = String::from_utf8_lossy(pdf);
        let needle = format!("/Title<{}>/Dest[", utf16be_title_hex(title));
        let pos = text
            .find(&needle)
            .unwrap_or_else(|| panic!("bookmark for {title}"));
        let slice = &text[pos + needle.len()..pos + needle.len() + 16];
        let obj_id = slice
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .unwrap_or_else(|_| panic!("bookmark dest id for {title}: {slice:?}"));
        page_ids
            .iter()
            .position(|&id| id == obj_id)
            .unwrap_or_else(|| panic!("bookmark {title} dest {obj_id} not a page"))
    }

    fn fat_doc(savings_rows: usize, config_rows: usize) -> Document {
        let rows: Vec<ReportSavings> = (0..savings_rows)
            .map(|i| ReportSavings {
                plugin: format!("plugin-{i:03}"),
                rows: 1u64,
                est_before: 10,
                est_after: 5,
                saved: 5,
            })
            .collect();
        Document {
            ledgers: ReportLedgers {
                window: ReportWindow {
                    since: "30d".into(),
                    from_unix: 1,
                    to_unix: 2,
                    from_date: "2026-08-10".into(),
                    to_date: "2026-09-09".into(),
                    db_path: "/tmp/rtok.db".into(),
                    calls_in_window: 7,
                    calls_total: 7,
                    measurements: 3,
                    usage: 3,
                },
                savings: ReportSavingsSection {
                    rows,
                    total_rows: savings_rows as u64,
                    total_saved: savings_rows as i64 * 5,
                    kinds: vec!["filter".into()],
                },
                calls: ReportCallsSection {
                    rows: vec![ReportCalls {
                        surface: "hook".into(),
                        calls: 2,
                        timed: 2,
                        p50_ms: Some(2.0),
                        p95_ms: Some(4.0),
                    }],
                    in_window: 2,
                    total: 2,
                    hooks: vec![],
                },
                cache: ReportCache {
                    sessions: 0,
                    turns: 0,
                    busts: 0,
                    by_cause: vec![],
                    detail: vec![],
                },
                expand: ReportExpand {
                    decisions: 0,
                    expanded: 0,
                    rate: 0.0,
                    expanded_ids: vec![],
                    cost: 0,
                    cost_rows: 0,
                },
            },
            config: (0..config_rows)
                .map(|i| ConfigEntry {
                    key: format!("section.test.key-{i:03}"),
                    value: "value".into(),
                    source: "user".into(),
                })
                .collect(),
            doctor: crate::doctor::Report {
                hooks_total: 0,
                hooks_by_event: BTreeMap::new(),
                mcp: vec![],
                proxy: String::new(),
                proxy_openai: String::new(),
                mcp_tool_search_disabled: false,
                bash_max_output_length: None,
                auto_compact_window: None,
                instructions: None,
            },
            recommendations: vec![],
        }
    }

    /// T36.13: contents page numbers and outline destinations land on the page
    /// that actually carries the section heading, including after a page break.
    #[rstest]
    fn section_page_numbers_match_heading_pages() {
        let pdf = render(&fat_doc(40, 120));
        let pages = positions(&pdf, "/Type/Page").len() - positions(&pdf, "/Type/Pages").len();
        assert!(pages >= 3, "need a multi-page report, got {pages}");
        for title in [
            "Window",
            "Savings",
            "Calls",
            "Cache",
            "Expand",
            "Config",
            "Doctor",
            "Recommendations",
        ] {
            let toc = toc_page_num(&pdf, title);
            let body = body_heading_page(&pdf, title);
            let bookmark = bookmark_page(&pdf, title);
            assert_eq!(toc, body, "{title}: toc {toc} != body {body}");
            assert_eq!(bookmark, toc, "{title}: outline {bookmark} != toc {toc}");
        }
    }
}
