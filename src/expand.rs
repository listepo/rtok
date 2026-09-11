//! `rtok expand <id>` (plan T3.5) and the shared fetch used by the MCP `expand` tool (T5.4).

use crate::config::Config;
use crate::plugin::{Measurement, Runtime};
use crate::tokens::Class;
use anyhow::{Result, bail};

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
pub fn slice_lines(lines: Vec<&str>, a: usize, b: usize) -> Vec<&str> {
    lines
        .into_iter()
        .take(b)
        .skip(a.saturating_sub(1))
        .collect()
}

/// Optional `--lines` / `--grep` filtering shared with the MCP `expand` tool.
pub fn filter_lines<'a>(text: &'a str, lines: Option<&str>, grep: Option<&str>) -> Vec<&'a str> {
    let mut out: Vec<&str> = text.lines().collect();
    if let Some(spec) = lines
        && let Ok((a, b)) = parse_range(spec, out.len())
    {
        out = slice_lines(out, a, b);
    }
    if let Some(g) = grep {
        out.retain(|l| l.contains(g));
    }
    out
}

fn cap_lines(out: &mut Vec<&str>, max_lines: u32) -> usize {
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
    max_lines: u32,
) -> String {
    let mut out = filter_lines(text, lines, grep);
    let omitted = cap_lines(&mut out, max_lines);
    let mut rendered = out.join("\n");
    if omitted > 0 {
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        rendered.push_str(&format!("… {omitted} lines omitted (expand {id})"));
    }
    rendered
}

/// Print the archived payload. `--lines a-b` is 1-based inclusive; `--grep` is substring.
pub fn run(cfg: &Config, id: &str, lines: Option<&str>, grep: Option<&str>) -> Result<()> {
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
    let rendered = render_lines(&text, id, lines, grep, max_lines);
    if !rendered.is_empty() {
        println!("{rendered}");
    }
    Ok(())
}

pub(crate) fn parse_range(spec: &str, n: usize) -> Result<(usize, usize)> {
    let mut parts = spec.splitn(2, '-');
    let a: usize = parts.next().unwrap_or("1").parse().unwrap_or(1);
    let b: usize = parts.next().map(|s| s.parse().unwrap_or(n)).unwrap_or(n);
    Ok((a.max(1), b.min(n)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn cfg(name: &str) -> Config {
        crate::testutil::config(name).0
    }

    #[test]
    fn unknown_id_is_err() {
        let c = cfg("unknown");
        let err = run(&c, "no-such", None, None).unwrap_err();
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
        run(&c, &id, None, None).unwrap();
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
        let err = run(&c, &id, None, None).unwrap_err();
        assert!(err.to_string().contains("unknown archive id"), "{err}");
    }

    #[test]
    fn parse_range_bare_start_runs_to_end() {
        assert_eq!(parse_range("10", 20).unwrap(), (10, 20));
        assert_eq!(parse_range("5-5", 20).unwrap(), (5, 5));
    }

    #[rstest]
    fn max_lines_truncates_and_reports() {
        let text: String = (1..=150)
            .map(|n| format!("line{n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = render_lines(&text, "arc123", None, None, 100);
        assert_eq!(
            out.lines().filter(|l| !l.contains("lines omitted")).count(),
            100
        );
        assert!(out.contains("50 lines omitted (expand arc123)"), "{out}");
    }
}
