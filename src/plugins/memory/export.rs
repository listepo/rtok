//! `rtok memory export` (plan T67.2): the JSONL that `memory import` reads.

use crate::config::Config;
use anyhow::Result;
use std::io::Write;

/// One `{kind,title,body,project}` per line in id order; `checkpoint:*` rows are
/// session-local and stay behind. Returns the row count.
pub fn run(cfg: &Config, project: Option<&str>, out: &mut impl Write) -> Result<u32> {
    let cx = crate::plugin::Runtime::open(cfg.clone(), "export")?;
    let rows = cx.store.list_notes(project)?;
    for (project, kind, title, body) in &rows {
        serde_json::to_writer(
            &mut *out,
            &serde_json::json!({"kind": kind, "title": title, "body": body, "project": project}),
        )?;
        out.write_all(b"\n")?;
    }
    Ok(rows.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::memory::import;
    use crate::testutil::config as cfg;

    #[test]
    fn export_round_trips_through_import_without_checkpoints() {
        let (a, dir_a) = cfg("export-a");
        {
            let cx = crate::plugin::Runtime::open(a.clone(), "seed").unwrap();
            for (kind, title, body) in [
                ("decision", "auth", "jwt"),
                ("bug", "cache", "stale key"),
                ("note", "walrus", "journal"),
            ] {
                cx.store.insert_note(Some("p"), kind, title, body).unwrap();
            }
            cx.store
                .insert_note(Some("rtok"), "checkpoint:s1", "compact", "checkpoint\n")
                .unwrap();
        }
        let mut buf = Vec::new();
        assert_eq!(run(&a, None, &mut buf).unwrap(), 3);
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.lines().count(), 3, "{text}");
        assert!(
            text.starts_with(r#"{"body":"jwt","kind":"decision","project":"p","title":"auth"}"#),
            "{text}"
        );
        assert!(!text.contains("checkpoint"), "{text}");
        let mut only_q = Vec::new();
        assert_eq!(run(&a, Some("q"), &mut only_q).unwrap(), 0);

        let (b, dir_b) = cfg("export-b");
        let file = dir_b.join("n.jsonl");
        std::fs::write(&file, &text).unwrap();
        let first = import::run(&b, &file, false).unwrap();
        assert_eq!((first.inserted, first.skipped), (3, 0));
        let second = import::run(&b, &file, false).unwrap();
        assert_eq!((second.inserted, second.skipped), (0, 3));
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }
}
