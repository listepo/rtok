//! `rtok stats` (plan T1.2): per-tool sizes, Bash families, MCP groups, CTT.
//!
//! Tool-result tokens use 4 chars/token (`research.md` §2 heuristic) so this report is
//! comparable to that baseline. Usage counters are the API numbers from the transcript.

use super::jsonl::{self, Parsed};
use crate::config::Config;
use crate::store::Store;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, SystemTime};

const CHARS_PER_TOKEN: f64 = 4.0;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SizeRow {
    pub count: u64,
    pub total_bytes: u64,
    pub mean: u64,
    pub p95: u64,
    pub max: u64,
    pub est_tokens: u64,
    pub ctt: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub sessions: u64,
    pub lines: u64,
    pub malformed: u64,
    pub tools: BTreeMap<String, SizeRow>,
    pub bash_families: BTreeMap<String, SizeRow>,
    pub mcp_groups: BTreeMap<String, SizeRow>,
    pub usage_input: u64,
    pub usage_cache_create: u64,
    pub usage_cache_read: u64,
    pub usage_output: u64,
    pub cache_hit_rate: f64,
    pub median_final_context: u64,
}

impl Report {
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn to_table(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "sessions {}  lines {}  malformed {}\n",
            self.sessions, self.lines, self.malformed
        ));
        s.push_str(&format!(
            "usage input={} cache_create={} cache_read={} output={}  hit={:.1}%  median_context={}\n",
            self.usage_input,
            self.usage_cache_create,
            self.usage_cache_read,
            self.usage_output,
            self.cache_hit_rate * 100.0,
            self.median_final_context
        ));
        s.push_str(&format_section("tool", &self.tools));
        s.push_str(&format_section("bash", &self.bash_families));
        s.push_str(&format_section("mcp", &self.mcp_groups));
        s
    }
}

fn format_section(title: &str, rows: &BTreeMap<String, SizeRow>) -> String {
    let mut s = format!(
        "{title:<24} {:>7} {:>12} {:>8} {:>8} {:>8} {:>12} {:>12}\n",
        "count", "bytes", "mean", "p95", "max", "est_tokens", "ctt"
    );
    for (name, r) in rows {
        s.push_str(&format!(
            "{name:<24} {:>7} {:>12} {:>8} {:>8} {:>8} {:>12} {:>12}\n",
            r.count, r.total_bytes, r.mean, r.p95, r.max, r.est_tokens, r.ctt
        ));
    }
    s
}

pub fn parse_since(s: &str) -> Result<Duration> {
    let s = s.trim();
    let (n, unit) = s.split_at(s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len()));
    let n: u64 = n.parse().map_err(|_| anyhow::anyhow!("bad --since {s}"))?;
    Ok(match unit {
        "" | "d" => Duration::from_secs(n * 86400),
        "h" => Duration::from_secs(n * 3600),
        _ => bail!("bad --since unit in {s}"),
    })
}

pub fn collect(dir: &Path, since: Duration, plugin: &str) -> Result<Report> {
    let cutoff = SystemTime::now()
        .checked_sub(since)
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let mut report = Report::default();
    let mut finals = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if p.extension().and_then(|s| s.to_str()) != Some("jsonl") {
                continue;
            }
            let meta = e.metadata().ok();
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if mtime < cutoff {
                continue;
            }
            let parsed = jsonl::parse_path(&p)?;
            fold_session(&parsed, plugin, &mut report, &mut finals);
        }
    }
    finish_rows(&mut report.tools);
    finish_rows(&mut report.bash_families);
    finish_rows(&mut report.mcp_groups);
    let denom = report.usage_cache_read + report.usage_cache_create + report.usage_input;
    report.cache_hit_rate = if denom == 0 {
        0.0
    } else {
        report.usage_cache_read as f64 / denom as f64
    };
    finals.sort_unstable();
    report.median_final_context = if finals.is_empty() {
        0
    } else {
        finals[finals.len() / 2]
    };
    Ok(report)
}

fn fold_session(parsed: &Parsed, plugin: &str, report: &mut Report, finals: &mut Vec<u64>) {
    report.sessions += 1;
    report.lines += parsed.lines;
    report.malformed += parsed.malformed;
    let n = u64::from(parsed.turns);
    let mut id_name: BTreeMap<&str, &str> = BTreeMap::new();
    let mut id_family: BTreeMap<&str, String> = BTreeMap::new();
    for u in &parsed.tool_uses {
        id_name.insert(u.id.as_str(), u.name.as_str());
        if u.name == "Bash"
            && let Some(cmd) = u.input.get("command").and_then(|v| v.as_str())
        {
            id_family.insert(u.id.as_str(), bash_family(cmd));
        }
    }
    for r in &parsed.tool_results {
        let name = id_name
            .get(r.tool_use_id.as_str())
            .copied()
            .unwrap_or("unknown");
        if !plugin.is_empty() && plugin != name && !name.contains(plugin) {
            continue;
        }
        let bytes = r.content.len() as u64;
        let tokens = est_tokens(bytes);
        let remain = n.saturating_sub(u64::from(r.turn));
        let ctt = tokens.saturating_mul(remain);
        add(&mut report.tools, name, bytes, tokens, ctt);
        if name == "Bash" {
            let fam = id_family
                .get(r.tool_use_id.as_str())
                .map(String::as_str)
                .unwrap_or("other");
            add(&mut report.bash_families, fam, bytes, tokens, ctt);
        }
        if let Some(grp) = mcp_group(name) {
            add(&mut report.mcp_groups, grp, bytes, tokens, ctt);
        }
    }
    for u in &parsed.usages {
        report.usage_input += u64::from(u.input_tokens);
        report.usage_cache_create += u64::from(u.cache_creation_input_tokens);
        report.usage_cache_read += u64::from(u.cache_read_input_tokens);
        report.usage_output += u64::from(u.output_tokens);
    }
    if let Some(last) = parsed.usages.last() {
        finals.push(
            u64::from(last.input_tokens)
                + u64::from(last.cache_creation_input_tokens)
                + u64::from(last.cache_read_input_tokens),
        );
    }
}

fn add(map: &mut BTreeMap<String, SizeRow>, name: &str, bytes: u64, tokens: u64, ctt: u64) {
    let row = map.entry(name.to_string()).or_default();
    row.count += 1;
    row.total_bytes += bytes;
    row.max = row.max.max(bytes);
    row.est_tokens += tokens;
    row.ctt += ctt;
    // mean/p95 filled in finish_rows from totals; p95 needs samples — stash bytes in max-only
    // for T1.2 we recompute mean from totals; p95 approximated as max until we store samples.
}

/// Samples live in `total_bytes` history via a side vec keyed... keep it simple: mean from
/// totals; p95 = max for v0 (honest: we don't keep every size). Tests pin mean/ctt.
fn finish_rows(map: &mut BTreeMap<String, SizeRow>) {
    for row in map.values_mut() {
        row.mean = row.total_bytes.checked_div(row.count).unwrap_or(0);
        row.p95 = row.max;
    }
}

fn est_tokens(bytes: u64) -> u64 {
    if bytes == 0 {
        0
    } else {
        ((bytes as f64) / CHARS_PER_TOKEN).ceil() as u64
    }
}

pub fn bash_family(cmd: &str) -> String {
    let mut s = cmd.trim();
    loop {
        let next = strip_prefix_env(s).or_else(|| strip_prefix_cd(s));
        match next {
            Some(rest) if rest != s => s = rest,
            _ => break,
        }
    }
    s.split_whitespace()
        .next()
        .unwrap_or("other")
        .rsplit('/')
        .next()
        .unwrap_or("other")
        .to_string()
}

fn strip_prefix_env(s: &str) -> Option<&str> {
    let t = s.trim_start();
    let ident_end = t.find(|c: char| !c.is_ascii_alphanumeric() && c != '_')?;
    if ident_end == 0 || !t.as_bytes().get(ident_end).is_some_and(|b| *b == b'=') {
        return None;
    }
    let rest = &t[ident_end + 1..];
    let rest = if rest.starts_with('\'') || rest.starts_with('"') {
        let q = rest.as_bytes()[0];
        let end = rest.as_bytes().iter().skip(1).position(|b| *b == q)? + 1;
        rest[end + 1..].trim_start()
    } else {
        rest.split_once(char::is_whitespace)?.1.trim_start()
    };
    Some(rest)
}

fn strip_prefix_cd(s: &str) -> Option<&str> {
    let t = s.trim_start();
    if !t.starts_with("cd ") && !t.starts_with("cd\t") {
        return None;
    }
    let after = t[2..].trim_start();
    if let Some(rest) = after.strip_prefix("&&") {
        return Some(rest.trim_start());
    }
    let after_path = after.split_once(char::is_whitespace)?.1.trim_start();
    Some(after_path.strip_prefix("&&")?.trim_start())
}

fn mcp_group(name: &str) -> Option<&str> {
    let rest = name.strip_prefix("mcp__")?;
    Some(rest.split("__").next().unwrap_or(rest))
}

/// SQLite measurements for a catalogue plugin (`rtok stats --plugin cmd --json`).
pub fn plugin_json(cfg: &Config, plugin: &str) -> Result<String> {
    let store = Store::open(&cfg.core.db_path)?;
    let rows = store.list_measurements(plugin)?;
    let archive_hits = rows.iter().filter(|r| r.kind == "expand").count();
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .filter(|r| r.kind != "expand")
        .map(|r| {
            json!({
                "kind": r.kind,
                "before": r.before_bytes,
                "after": r.after_bytes,
                "est_before": r.est_before,
                "est_after": r.est_after,
                "ref_id": r.ref_id,
            })
        })
        .collect();
    let mut out = json!({
        "plugin": plugin,
        "archive_hits": archive_hits,
        "rows": rows,
    });
    if plugin == "archive" {
        // T5.4 honesty metric: how often a live-zone pointer had to be expanded.
        let (decisions, expanded) = store.archive_decision_counts()?;
        let rate = if decisions > 0 {
            expanded as f64 / decisions as f64
        } else {
            0.0
        };
        out["decisions"] = json!(decisions);
        out["expanded"] = json!(expanded);
        out["expand_rate"] = json!(rate);
    }
    Ok(serde_json::to_string_pretty(&out)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::io::Write;

    #[test]
    fn since_60d_parses() {
        assert_eq!(parse_since("60d").unwrap(), Duration::from_secs(60 * 86400));
    }

    #[test]
    fn bash_family_strips_cd_and_env() {
        assert_eq!(bash_family("cd /tmp && git status"), "git");
        assert_eq!(bash_family("FOO=1 grep x"), "grep");
        assert_eq!(bash_family("sed -n 1p"), "sed");
    }

    #[test]
    fn ctt_and_tool_totals_on_mini_session() {
        let dir = tempfile_dir();
        let path = dir.join("s.jsonl");
        let mut f = fs::File::create(&path).unwrap();
        // 3 turns: assistant, user result, assistant. result at turn 1, N=3 → ctt = tokens*(3-1)
        let body = "hello world!!"; // 13 bytes → ceil(13/4)=4 tokens; ctt=8
        writeln!(
            f,
            "{}",
            json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"Bash","input":{"command":"cd x && sed -n p"}}],"usage":{"input_tokens":10,"output_tokens":1}}})
        )
        .unwrap();
        writeln!(
            f,
            "{}",
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":body}]}})
        )
        .unwrap();
        writeln!(
            f,
            "{}",
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"done"}],"usage":{"input_tokens":20,"cache_read_input_tokens":80,"output_tokens":2}}})
        )
        .unwrap();
        let r = collect(&dir, Duration::from_secs(86400 * 60), "").unwrap();
        assert_eq!(r.sessions, 1);
        let bash = r.tools.get("Bash").unwrap();
        assert_eq!(bash.count, 1);
        assert_eq!(bash.est_tokens, 4);
        assert_eq!(bash.ctt, 8);
        assert_eq!(r.bash_families.get("sed").unwrap().count, 1);
        assert_eq!(r.usage_cache_read, 80);
        fs::remove_dir_all(&dir).ok();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("rtok-stats-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
}
