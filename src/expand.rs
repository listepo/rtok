//! `rtok expand <id>` (plan T3.5) and the shared fetch used by the MCP `expand` tool (T5.4).

use crate::config::Config;
use crate::plugin::{Measurement, Runtime};
use crate::tokens::Class;
use anyhow::{Result, bail};
use regex::Regex;

/// Read an archived payload. When the id is a live-zone pointer (T5.3) this freezes it:
/// the owning plugin sends the original from the next request on, and one `expand`
/// measurement records the cost — `rtok stats --plugin <id>` derives the expand rate.
pub fn fetch(cx: &Runtime, id: &str) -> Result<Option<Vec<u8>>> {
    let Some(bytes) = cx
        .store
        .get_archive(id, Some(&cx.config.core.archive_dir))?
    else {
        return Ok(None);
    };
    if cx.store.mark_expanded(id)? > 0 {
        let n = bytes.len() as u64;
        let plugin = match cx.store.live_zone_pointer(id)? {
            Some(p) if p.starts_with("[toon ") => "toon",
            Some(_) => "archive",
            None => "archive",
        };
        cx.record(&Measurement {
            plugin,
            kind: "expand",
            before_bytes: 0,
            after_bytes: n,
            est_before: 0,
            est_after: cx.estimate(&String::from_utf8_lossy(&bytes), Class::Code),
            ref_id: Some(id.to_string()),
            call_id: cx.call_id,
        })?;
    }
    Ok(Some(bytes))
}

/// 1-based inclusive line range over already-split lines.
pub fn slice_lines<T>(lines: Vec<T>, a: usize, b: usize) -> Vec<T> {
    lines
        .into_iter()
        .take(b)
        .skip(a.saturating_sub(1))
        .collect()
}

/// Optional `--lines` / `--grep` filtering shared with the MCP `expand` tool.
///
/// `grep` is a regex (a pattern that does not compile is matched literally) and every hit
/// prints as `N:line`, numbered by its position in the archived payload, so a following
/// `--lines a-b` can pull the context around a hit instead of the whole body — the
/// search-then-slice loop of recursive-llm (`research.md` §12, T66.1).
///
/// `context` (T67.2, `--context N`, per call like `--grep`): with `grep`, print each hit
/// with N lines on either side, windows that overlap or touch merged into one block and
/// blocks separated by `--`. Numbers stay absolute; without `grep` it is ignored, and 0
/// keeps the hits-only output.
pub fn filter_lines(
    text: &str,
    lines: Option<&str>,
    grep: Option<&str>,
    context: usize,
) -> Result<Vec<String>> {
    let mut out: Vec<(usize, &str)> = text.lines().enumerate().map(|(i, l)| (i + 1, l)).collect();
    if let Some(spec) = lines {
        let (a, b) = parse_range(spec, out.len())?;
        out = slice_lines(out, a, b);
    }
    let Some(g) = grep else {
        return Ok(out.into_iter().map(|(_, l)| l.to_string()).collect());
    };
    let re = Regex::new(g).or_else(|_| Regex::new(&regex::escape(g)))?;
    if context == 0 {
        return Ok(out
            .into_iter()
            .filter(|(_, l)| re.is_match(l))
            .map(|(n, l)| format!("{n}:{l}"))
            .collect());
    }
    // Inclusive `out`-index windows `[hit-N, hit+N]`, union of overlapping or adjacent.
    let mut windows: Vec<(usize, usize)> = Vec::new();
    for (i, (_, l)) in out.iter().enumerate() {
        if !re.is_match(l) {
            continue;
        }
        let lo = i.saturating_sub(context);
        let hi = (i + context).min(out.len() - 1);
        match windows.last_mut() {
            Some(last) if lo <= last.1 + 1 => last.1 = last.1.max(hi),
            _ => windows.push((lo, hi)),
        }
    }
    let mut rendered: Vec<String> = Vec::new();
    for (w, (lo, hi)) in windows.iter().enumerate() {
        if w > 0 {
            rendered.push("--".to_string());
        }
        rendered.extend(out[*lo..=*hi].iter().map(|(n, l)| format!("{n}:{l}")));
    }
    Ok(rendered)
}

/// Head and tail of `text` around `marker`, at most `max` chars in total. Shared by the
/// `read` cap and the MCP `expand` cap; the caller decides what the marker names.
///
/// The marker is the floor: below its length only the marker comes back, because the id
/// in it is what makes the cut lossless (`config validate` rejects `max_chars` < 100).
pub(crate) fn cut(text: &str, marker: &str, max: usize) -> String {
    let body_budget = max.saturating_sub(marker.chars().count());
    let keep = body_budget / 2;
    // Byte offsets of the first and last `keep` chars; no `Vec<char>` copy of the whole text.
    let head_end = text.char_indices().nth(keep).map_or(text.len(), |(i, _)| i);
    let tail_start = match keep {
        0 => text.len(),
        k => text.char_indices().rev().nth(k - 1).map_or(0, |(i, _)| i),
    };
    format!("{}{marker}{}", &text[..head_end], &text[tail_start..])
}

fn cap_lines<T>(out: &mut Vec<T>, max_lines: u32) -> usize {
    if max_lines == 0 {
        return 0;
    }
    let max = max_lines as usize;
    if out.len() <= max {
        return 0;
    }
    let omitted = out.len() - max;
    out.truncate(max);
    omitted
}

/// Render filtered lines plus an optional `[expand] max_lines` trailer.
pub(crate) fn render_lines(
    text: &str,
    id: &str,
    lines: Option<&str>,
    grep: Option<&str>,
    context: usize,
    max_lines: u32,
) -> Result<String> {
    let mut out = filter_lines(text, lines, grep, context)?;
    let omitted = cap_lines(&mut out, max_lines);
    let mut rendered = out.join("\n");
    if omitted > 0 {
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&format!("… {omitted} lines omitted (expand {id})"));
    }
    Ok(rendered)
}

/// Print the archived payload. `--lines a-b` is 1-based inclusive; `--grep` is a regex whose
/// hits come back `N:`-numbered (see [`filter_lines`]); `--context N` widens those hits.
pub fn run(
    cfg: &Config,
    id: &str,
    lines: Option<&str>,
    grep: Option<&str>,
    context: usize,
) -> Result<()> {
    // Validate before fetch: fetching a live-zone pointer freezes it. A malformed
    // range must not mutate archive state even though no payload can be printed.
    if let Some(spec) = lines {
        parse_range(spec, usize::MAX)?;
    }
    let cx = Runtime::open(cfg.clone(), "expand")?;
    let Some(bytes) = fetch(&cx, id)? else {
        bail!("unknown archive id: {id}");
    };
    let max_lines = cfg.expand.max_lines;
    if lines.is_none() && grep.is_none() && max_lines == 0 {
        std::io::Write::write_all(&mut std::io::stdout(), &bytes)?;
        return Ok(());
    }
    let text = String::from_utf8_lossy(&bytes);
    let rendered = render_lines(&text, id, lines, grep, context, max_lines)?;
    if !rendered.is_empty() {
        println!("{rendered}");
    }
    Ok(())
}

pub(crate) fn parse_range(spec: &str, n: usize) -> Result<(usize, usize)> {
    let spec = spec.trim();
    if spec.is_empty() {
        bail!("invalid line range `{spec}`: expected a positive line or a-b");
    }
    let mut parts = spec.splitn(2, '-');
    let start = parts.next().unwrap_or_default();
    let end = parts.next();
    let a = if start.is_empty() {
        1
    } else {
        start.parse::<usize>().map_err(|_| {
            anyhow::anyhow!("invalid line range `{spec}`: expected a positive line or a-b")
        })?
    };
    let requested_b = match end {
        Some("") | None => None,
        Some(s) => Some(s.parse::<usize>().map_err(|_| {
            anyhow::anyhow!("invalid line range `{spec}`: expected a positive line or a-b")
        })?),
    };
    if a == 0 || requested_b == Some(0) {
        bail!("invalid line range `{spec}`: lines are 1-based");
    }
    if requested_b.is_some_and(|b| a > b) {
        bail!("invalid line range `{spec}`: start exceeds end");
    }
    // A start past the last line would slice to nothing and print empty output
    // with exit 0; fail loudly instead so the caller knows the range is wrong.
    if a > n {
        bail!("invalid line range `{spec}`: start exceeds line count {n}");
    }
    Ok((a, requested_b.unwrap_or(n).min(n)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn cfg(name: &str) -> Config {
        crate::testutil::config(name).0
    }

    #[test]
    fn malformed_range_is_rejected_before_archive_lookup() {
        let c = cfg("bad-range");
        let err = run(&c, "no-such", Some("3-2"), None, 0).unwrap_err();
        assert!(err.to_string().contains("invalid line range"), "{err}");
    }

    #[test]
    fn unknown_id_is_err() {
        let c = cfg("unknown");
        let err = run(&c, "no-such", None, None, 0).unwrap_err();
        assert!(err.to_string().contains("unknown archive id"), "{err}");
    }

    #[test]
    fn round_trip_from_put_archive() {
        let c = cfg("round");
        let cx = crate::plugin::Runtime::open(c.clone(), "expand").unwrap();
        let id = cx
            .store
            .put_archive("expand", b"hello\nworld\n", &c.core.archive_dir)
            .unwrap();
        let got = cx
            .store
            .get_archive(&id, Some(&c.core.archive_dir))
            .unwrap()
            .unwrap();
        assert_eq!(got, b"hello\nworld\n");
        drop(cx);
        run(&c, &id, None, None, 0).unwrap();
    }

    #[test]
    fn moved_archive_dir_still_reads() {
        let c = cfg("moved");
        let cx = crate::plugin::Runtime::open(c.clone(), "expand").unwrap();
        let id = cx
            .store
            .put_archive("expand", b"relocated\n", &c.core.archive_dir)
            .unwrap();
        drop(cx);
        let dest = c.core.archive_dir.parent().unwrap().join("archive-moved");
        std::fs::rename(&c.core.archive_dir, &dest).unwrap();
        let mut c2 = c.clone();
        c2.core.archive_dir = dest;
        let cx = crate::plugin::Runtime::open(c2.clone(), "expand").unwrap();
        assert_eq!(fetch(&cx, &id).unwrap().unwrap(), b"relocated\n");
    }

    #[test]
    fn missing_file_is_unknown_id() {
        let c = cfg("gone");
        let cx = crate::plugin::Runtime::open(c.clone(), "expand").unwrap();
        let id = cx
            .store
            .put_archive("expand", b"bye\n", &c.core.archive_dir)
            .unwrap();
        std::fs::remove_file(c.core.archive_dir.join(&id)).unwrap();
        drop(cx);
        let err = run(&c, &id, None, None, 0).unwrap_err();
        assert!(err.to_string().contains("unknown archive id"), "{err}");
    }

    /// T55.11: `rtok expand` runs under session "expand", not the proxy session that
    /// owns the decision — the freeze must still reach the owning session's pointer.
    #[test]
    fn cli_expand_freezes_the_owning_sessions_pointer() {
        let c = cfg("freeze-owner");
        let cx_proxy = crate::plugin::Runtime::open(c.clone(), "proxy-sess").unwrap();
        let id = cx_proxy
            .store
            .put_archive("proxy-sess", b"payload\n", &c.core.archive_dir)
            .unwrap();
        cx_proxy
            .store
            .put_archive_decision("tu-1", &id, "proxy-sess", &format!("[archived {id}]"))
            .unwrap();
        drop(cx_proxy);
        let cx_cli = crate::plugin::Runtime::open(c, "expand").unwrap();
        assert_eq!(fetch(&cx_cli, &id).unwrap().unwrap(), b"payload\n");
        assert!(
            cx_cli
                .store
                .archive_decision("proxy-sess", "tu-1")
                .unwrap()
                .unwrap()
                .expanded,
            "the writer's session must see the freeze"
        );
    }

    /// T55.11: an expand of a toon pointer attributes its Measurement to `toon`;
    /// `live_zone_pointer` no longer needs the writer's session.
    #[test]
    fn expand_measurement_attributes_toon_pointers() {
        let c = cfg("toon-attr");
        let cx_proxy = crate::plugin::Runtime::open(c.clone(), "proxy-sess").unwrap();
        let id = cx_proxy
            .store
            .put_archive("proxy-sess", b"table\n", &c.core.archive_dir)
            .unwrap();
        cx_proxy
            .store
            .put_archive_decision("tu-1", &id, "proxy-sess", &format!("[toon {id}]\na,b"))
            .unwrap();
        drop(cx_proxy);
        let cx_cli = crate::plugin::Runtime::open(c, "expand").unwrap();
        assert!(fetch(&cx_cli, &id).unwrap().is_some());
        let rows = cx_cli.store.list_measurements("toon").unwrap();
        assert!(rows.iter().any(|r| r.kind == "expand"), "{rows:?}");
    }

    /// T66.1: a hit carries its line number so `--lines` can follow; the pattern is a
    /// regex, or the literal text when it does not compile.
    #[test]
    fn grep_is_regex_numbered_by_archive_line_and_falls_back_to_literal() {
        let text = "alpha\nerror[E0308]: mismatched\nbeta\nerror[E0599]: no method\n";
        assert_eq!(
            filter_lines(text, None, Some(r"error\[E0\d+\]"), 0).unwrap(),
            ["2:error[E0308]: mismatched", "4:error[E0599]: no method"]
        );
        // Numbers stay absolute inside a range.
        assert_eq!(
            filter_lines(text, Some("3-4"), Some("error"), 0).unwrap(),
            ["4:error[E0599]: no method"]
        );
        // An unclosed bracket is not a regex: match it literally, never error.
        assert_eq!(
            filter_lines(text, None, Some("[E0308"), 0).unwrap(),
            ["2:error[E0308]: mismatched"]
        );
        // Without grep the output is the bare lines, as before (context is ignored too).
        assert_eq!(
            filter_lines(text, Some("1-2"), None, 7).unwrap(),
            ["alpha", "error[E0308]: mismatched"]
        );
    }

    /// T67.2: `context N` widens each grep hit to `[hit-N, hit+N]` with absolute numbers;
    /// windows that overlap or touch merge into one block and blocks separate on `--`.
    #[test]
    fn context_windows_merge_and_separate_on_dash_dash() {
        let text = "a1\na2\nHIT\na4\na5\nb1\nHIT\nb3\nb4\nb5\n";
        // Hits at lines 3 and 7, context 2: windows [1,5] and [5,9] overlap → one block.
        assert_eq!(
            filter_lines(text, None, Some("HIT"), 2).unwrap(),
            [
                "1:a1", "2:a2", "3:HIT", "4:a4", "5:a5", "6:b1", "7:HIT", "8:b3", "9:b4",
            ]
        );
        // Hits at lines 2 and 9, context 1: a gap between the windows keeps both and the `--`.
        let spread = "h1\nHIT\nx3\nx4\nx5\nx6\nx7\nx8\nHIT\n";
        assert_eq!(
            filter_lines(spread, None, Some("HIT"), 1).unwrap(),
            ["1:h1", "2:HIT", "3:x3", "--", "8:x8", "9:HIT"]
        );
    }

    /// A window at either edge clamps to the file instead of under- or overflowing.
    #[test]
    fn context_windows_clamp_at_the_file_edges() {
        assert_eq!(
            filter_lines("HIT\nb\n", None, Some("HIT"), 3).unwrap(),
            ["1:HIT", "2:b"]
        );
        assert_eq!(
            filter_lines("a\nHIT", None, Some("HIT"), 3).unwrap(),
            ["1:a", "2:HIT"]
        );
    }

    /// With `--lines` the windows are computed inside the slice but the numbers stay
    /// absolute, matching how bare hits number today.
    #[test]
    fn context_inside_a_lines_range_keeps_absolute_numbers() {
        let text = "l1\nl2\nhit\nl4\nl5\nl6\nhit\nl8\n";
        assert_eq!(
            filter_lines(text, Some("2-8"), Some("hit"), 1).unwrap(),
            ["2:l2", "3:hit", "4:l4", "--", "6:l6", "7:hit", "8:l8"]
        );
    }

    /// `context 0` (the default) is byte-identical to the old hits-only output, and the
    /// joined output still answers to `[expand] max_lines` with its trailer.
    #[test]
    fn context_zero_is_the_old_output_and_max_lines_still_caps() {
        let text: String = (1..=20)
            .map(|n| {
                if n == 10 || n == 16 {
                    "hit".into()
                } else {
                    format!("l{n}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            filter_lines(&text, None, Some("hit"), 0).unwrap(),
            ["10:hit", "16:hit"]
        );
        // Hits at 10 and 16, context 3: windows [7,13] and [13,19] merge to 13 lines.
        let wide = filter_lines(&text, None, Some("hit"), 3).unwrap();
        assert_eq!(wide.len(), 13);
        assert!(wide.iter().all(|l| l != "--"));
        let out = render_lines(&text, "arc123", None, Some("hit"), 3, 5).unwrap();
        assert_eq!(
            out.lines().filter(|l| !l.contains("lines omitted")).count(),
            5
        );
        assert!(out.contains("8 lines omitted (expand arc123)"), "{out}");
    }

    #[test]
    fn parse_range_bare_start_runs_to_end() {
        assert_eq!(parse_range("10", 20).unwrap(), (10, 20));
        assert_eq!(parse_range("5-5", 20).unwrap(), (5, 5));
        assert_eq!(parse_range("-5", 20).unwrap(), (1, 5));
        assert_eq!(parse_range("5-", 20).unwrap(), (5, 20));
    }

    #[test]
    fn parse_range_rejects_malformed_zero_and_reverse_ranges() {
        for spec in ["", "abc", "1-two", "1-2-3", "0", "0-2", "3-2"] {
            let err = parse_range(spec, 20).unwrap_err();
            assert!(
                err.to_string().contains("invalid line range"),
                "{spec}: {err}"
            );
        }
    }

    #[test]
    fn parse_range_start_past_end_is_err_not_empty() {
        for spec in ["100-200", "11", "11-"] {
            let err = parse_range(spec, 10).unwrap_err();
            assert!(
                err.to_string().contains("start exceeds line count 10"),
                "{spec}: {err}"
            );
        }
        assert_eq!(parse_range("8-100", 10).unwrap(), (8, 10));
    }

    #[rstest]
    fn max_lines_truncates_and_reports() {
        let text: String = (1..=150)
            .map(|n| format!("line{n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = render_lines(&text, "arc123", None, None, 0, 100).unwrap();
        assert_eq!(
            out.lines().filter(|l| !l.contains("lines omitted")).count(),
            100
        );
        assert!(out.contains("50 lines omitted (expand arc123)"), "{out}");
    }
}
