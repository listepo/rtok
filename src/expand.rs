//! `rtok expand <id>` (plan T3.5).

use crate::config::Config;
use anyhow::{Result, bail};

/// Print the archived payload. `--lines a-b` is 1-based inclusive; `--grep` is substring.
pub fn run(cfg: &Config, id: &str, lines: Option<&str>, grep: Option<&str>) -> Result<()> {
    let cx = crate::plugin::Ctx::open(cfg.clone(), "expand")?;
    let Some(bytes) = cx.store.get_archive(id)? else {
        bail!("unknown archive id: {id}");
    };
    if lines.is_none() && grep.is_none() {
        std::io::Write::write_all(&mut std::io::stdout(), &bytes)?;
        return Ok(());
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut out: Vec<&str> = text.lines().collect();
    if let Some(spec) = lines {
        let (a, b) = parse_range(spec, out.len())?;
        out = out.into_iter().take(b).skip(a.saturating_sub(1)).collect();
    }
    if let Some(g) = grep {
        out.retain(|l| l.contains(g));
    }
    for line in &out {
        println!("{line}");
    }
    Ok(())
}

fn parse_range(spec: &str, n: usize) -> Result<(usize, usize)> {
    let mut parts = spec.splitn(2, '-');
    let a: usize = parts.next().unwrap_or("1").parse().unwrap_or(1);
    let b: usize = parts.next().map(|s| s.parse().unwrap_or(n)).unwrap_or(n);
    Ok((a.max(1), b.min(n)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(name: &str) -> Config {
        let dir = std::env::temp_dir().join(format!("rtok-expand-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        c.core.log_file = dir.join("rtok.log");
        c
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
        let cx = crate::plugin::Ctx::open(c.clone(), "expand").unwrap();
        let id = cx
            .store
            .put_archive("expand", b"hello\nworld\n", &c.core.archive_dir)
            .unwrap();
        let got = cx.store.get_archive(&id).unwrap().unwrap();
        assert_eq!(got, b"hello\nworld\n");
        drop(cx);
        run(&c, &id, None, None).unwrap();
    }
}
