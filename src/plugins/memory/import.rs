//! `rtok memory import <file.jsonl>` (plan T6.3).

use crate::config::Config;
use crate::plugin::Ctx;
use anyhow::Result;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub inserted: u32,
    pub skipped: u32,
    pub malformed: u32,
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "inserted {}  skipped {}  malformed {}",
            self.inserted, self.skipped, self.malformed
        )
    }
}

#[derive(Deserialize)]
struct Line {
    kind: String,
    title: String,
    body: String,
    #[serde(default)]
    project: Option<String>,
}

fn sha(body: &str) -> String {
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    format!("{:x}", h.finalize())
}

/// Import one JSON object per line. Dedupe by sha256 of `body`. Always exit-success.
pub fn run(cfg: &Config, path: &Path) -> Result<Report> {
    let cx = Ctx::open(cfg.clone(), "import")?;
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    let mut seen: HashSet<String> = cx
        .store
        .note_bodies()?
        .into_iter()
        .map(|b| sha(&b))
        .collect();
    let mut r = Report::default();
    for line in raw.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let Ok(row) = serde_json::from_str::<Line>(t) else {
            r.malformed += 1;
            continue;
        };
        let h = sha(&row.body);
        if !seen.insert(h) {
            r.skipped += 1;
            continue;
        }
        cx.store
            .insert_note(row.project.as_deref(), &row.kind, &row.title, &row.body)?;
        r.inserted += 1;
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(name: &str) -> (Config, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("rtok-import-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut c = Config::default();
        c.core.db_path = dir.join("rtok.db");
        c.core.archive_dir = dir.join("archive");
        c.core.log_file = dir.join("rtok.log");
        (c, dir)
    }

    fn fifty() -> String {
        (0..50)
            .map(|i| format!(r#"{{"kind":"note","title":"t{i}","body":"body-{i}"}}"#))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    }

    #[test]
    fn fifty_then_reimport_then_malformed_exits_ok() {
        let (c, dir) = cfg("fifty");
        let p = dir.join("n.jsonl");
        fs::write(&p, fifty()).unwrap();
        let a = run(&c, &p).unwrap();
        assert_eq!(
            a,
            Report {
                inserted: 50,
                skipped: 0,
                malformed: 0
            }
        );
        let b = run(&c, &p).unwrap();
        assert_eq!(
            b,
            Report {
                inserted: 0,
                skipped: 50,
                malformed: 0
            }
        );
        fs::write(&p, fifty() + "not json\n").unwrap();
        let d = run(&c, &p).unwrap();
        assert_eq!(
            d,
            Report {
                inserted: 0,
                skipped: 50,
                malformed: 1
            }
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
