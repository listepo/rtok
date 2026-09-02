//! `rtok stats --save-baseline` / `--compare` (plan T1.3).

use super::stats::Report;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub fn dir(home: &Path) -> PathBuf {
    home.join("measurements")
}

pub fn path(home: &Path, name: &str) -> PathBuf {
    dir(home).join(format!("{name}.json"))
}

pub fn save(home: &Path, name: &str, report: &Report) -> Result<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        bail!("bad baseline name");
    }
    let dir = dir(home);
    std::fs::create_dir_all(&dir)?;
    let path = path(home, name);
    std::fs::write(&path, report.to_json()?)?;
    Ok(path)
}

pub fn load(home: &Path, name: &str) -> Result<Report> {
    let path = path(home, name);
    let raw = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
    Ok(serde_json::from_str(&raw)?)
}

/// Print per-field deltas. All zeros after a save of the same report.
pub fn compare(home: &Path, name: &str, now: &Report) -> Result<String> {
    let old = load(home, name)?;
    let mut out = String::new();
    line(&mut out, "sessions", old.sessions, now.sessions);
    line(&mut out, "lines", old.lines, now.lines);
    line(&mut out, "usage_input", old.usage_input, now.usage_input);
    line(&mut out, "usage_output", old.usage_output, now.usage_output);
    for (label, a, b) in [
        ("tools", &old.tools, &now.tools),
        ("bash", &old.bash_families, &now.bash_families),
        ("mcp", &old.mcp_groups, &now.mcp_groups),
    ] {
        let names: std::collections::BTreeSet<_> = a.keys().chain(b.keys()).cloned().collect();
        for n in names {
            let oa = a.get(&n);
            let ob = b.get(&n);
            line(
                &mut out,
                &format!("{label}.{n}.est_tokens"),
                oa.map(|r| r.est_tokens).unwrap_or(0),
                ob.map(|r| r.est_tokens).unwrap_or(0),
            );
            line(
                &mut out,
                &format!("{label}.{n}.ctt"),
                oa.map(|r| r.ctt).unwrap_or(0),
                ob.map(|r| r.ctt).unwrap_or(0),
            );
        }
    }
    Ok(out)
}

fn line(out: &mut String, key: &str, old: u64, now: u64) {
    let d = now as i64 - old as i64;
    out.push_str(&format!("{key} {now} Δ{d}\n"));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::stats;
    use std::time::Duration;

    #[test]
    fn save_then_compare_all_deltas_zero() {
        let home = std::env::temp_dir().join(format!("rtok-base-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        std::fs::create_dir_all(&home).unwrap();
        let transcripts = home.join("tx");
        std::fs::create_dir_all(&transcripts).unwrap();
        std::fs::write(transcripts.join("s.jsonl"), "{}\n").unwrap();
        let report = stats::collect(
            &transcripts,
            Duration::from_secs(86400),
            "",
            stats::Replay::from_cfg(&crate::config::Config::default()),
        )
        .unwrap();
        save(&home, "before-rtok", &report).unwrap();
        let text = compare(&home, "before-rtok", &report).unwrap();
        assert!(text.contains("Δ0"), "{text}");
        assert!(!text.contains("Δ1"), "{text}");
        assert!(
            !text.lines().any(|l| l.contains('Δ') && !l.ends_with("Δ0")),
            "{text}"
        );
        std::fs::remove_dir_all(&home).ok();
    }
}
