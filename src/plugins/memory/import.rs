//! `rtok memory import <file.jsonl>` (plan T6.3).

use crate::config::Config;
use anyhow::Result;
use serde::Deserialize;
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
    crate::store::hex_sha256(body.as_bytes())
}

/// Import one JSON object per line. Dedupe by sha256 of `body`; a line whose
/// `(project, kind, title)` already names a local note is skipped too, whatever its body —
/// an older export must never replace a newer local body (T209: `notes_topic` is one row
/// per topic key, so a plain insert would otherwise fail outright on the collision).
/// Always exit-success. `dry_run` counts exactly what a real run would insert and skip,
/// and writes no rows.
pub fn run(cfg: &Config, path: &Path, dry_run: bool) -> Result<Report> {
    let cx = crate::plugin::Runtime::open(cfg.clone(), "import")?;
    let raw = std::fs::read_to_string(path).unwrap_or_default();
    let mut seen: HashSet<String> = cx
        .store
        .note_bodies()?
        .into_iter()
        .map(|b| sha(&b))
        .collect();
    // Topic keys already taken locally, kept up to date as lines are processed below so
    // `dry_run` (which never writes) reaches the same skip/insert call a real run would
    // for two imported lines that share a key neither one starts out matching locally.
    let mut keys: HashSet<(Option<String>, String, String)> = cx
        .store
        .list_notes(None)?
        .into_iter()
        .map(|(project, kind, title, _body)| (project, kind, title))
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
        let key = (row.project.clone(), row.kind.clone(), row.title.clone());
        if !keys.insert(key) {
            r.skipped += 1;
            continue;
        }
        if dry_run {
            r.inserted += 1;
            continue;
        }
        match cx.store.insert_note_if_absent(
            row.project.as_deref(),
            &row.kind,
            &row.title,
            &row.body,
        )? {
            Some(_) => r.inserted += 1,
            // Lost a race to a concurrent local write between the pre-load above and
            // this statement — the local body stands, same as the in-memory check.
            None => r.skipped += 1,
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::config as cfg;
    use std::fs;

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
        let a = run(&c, &p, false).unwrap();
        assert_eq!(
            a,
            Report {
                inserted: 50,
                skipped: 0,
                malformed: 0
            }
        );
        let b = run(&c, &p, false).unwrap();
        assert_eq!(
            b,
            Report {
                inserted: 0,
                skipped: 50,
                malformed: 0
            }
        );
        fs::write(&p, fifty() + "not json\n").unwrap();
        let d = run(&c, &p, false).unwrap();
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

    /// T209: `notes_topic` is one row per `(project, kind, title)`, so an import line
    /// naming a key a local note already holds must be skipped, never overwrite it — an
    /// older export must not replace a newer local body. `dry_run` reports the same
    /// count without touching the store.
    #[test]
    fn a_line_whose_key_exists_locally_with_a_different_body_is_skipped() {
        let (c, dir) = cfg("key-collision");
        let cx = crate::plugin::Runtime::open(c.clone(), "seed").unwrap();
        cx.store
            .insert_note(Some("p"), "note", "t", "local body")
            .unwrap();
        drop(cx);
        let p = dir.join("n.jsonl");
        fs::write(
            &p,
            r#"{"kind":"note","title":"t","body":"import body","project":"p"}"#.to_string() + "\n",
        )
        .unwrap();

        let dry = run(&c, &p, true).unwrap();
        assert_eq!(
            dry,
            Report {
                inserted: 0,
                skipped: 1,
                malformed: 0
            }
        );
        let real = run(&c, &p, false).unwrap();
        assert_eq!(real, dry, "dry_run and a real run agree");

        let cx = crate::plugin::Runtime::open(c.clone(), "verify").unwrap();
        let rows = cx.store.list_notes(Some("p")).unwrap();
        assert_eq!(
            rows,
            vec![(
                Some("p".to_string()),
                "note".to_string(),
                "t".to_string(),
                "local body".to_string()
            )],
            "the local body survives the import untouched"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
