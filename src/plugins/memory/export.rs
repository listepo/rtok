// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! `rtok memory export` (plan T66.2): the JSONL that `memory import` reads.

use crate::config::Config;
use crate::store::ExportNote;
use anyhow::Result;
use serde::Serialize;
use std::io::Write;

#[derive(Serialize)]
struct Line<'a> {
    id: i32,
    ts: i64,
    kind: &'a str,
    title: &'a str,
    body: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    project: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retired: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    superseded_by: Option<i32>,
    pinned: i32,
}

fn write_line(out: &mut impl Write, row: &ExportNote) -> Result<()> {
    let line = Line {
        id: row.id,
        ts: row.ts,
        kind: &row.kind,
        title: &row.title,
        body: &row.body,
        project: row.project.as_deref(),
        retired: row.retired,
        superseded_by: row.superseded_by,
        pinned: row.pinned,
    };
    serde_json::to_writer(&mut *out, &line)?;
    out.write_all(b"\n")?;
    Ok(())
}

/// One JSON object per line in id order; `checkpoint:*` and `session:*` rows stay behind.
/// Returns the row count.
pub fn run(cfg: &Config, project: Option<&str>, out: &mut impl Write) -> Result<u32> {
    let cx = crate::plugin::Runtime::open(cfg.clone(), "export")?;
    // Retired notes travel as tombstones (T294). Import writes `retired` back, so a
    // round-trip does not resurrect them as live (the failure T304 closed by omitting them).
    let rows = cx.store.list_export_notes(project)?;
    for row in &rows {
        write_line(out, row)?;
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
        let line = text.lines().next().expect("export has a line");
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(v.get("id").is_some());
        assert!(v.get("ts").is_some());
        assert_eq!(v["kind"], "decision");
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

    #[test]
    fn export_round_trips_tombstone_and_pin() {
        let (a, dir_a) = cfg("export-tombstone");
        let (retired_id, pinned_id) = {
            let cx = crate::plugin::Runtime::open(a.clone(), "seed").unwrap();
            cx.store
                .insert_note(Some("p"), "note", "live", "still here")
                .unwrap();
            let retired = cx
                .store
                .insert_note(Some("p"), "note", "gone", "body kept")
                .unwrap();
            cx.store.retire_note(retired, None).unwrap();
            let pinned = cx
                .store
                .insert_note(Some("p"), "note", "pinned", "first in recall")
                .unwrap();
            cx.store.set_note_pinned(pinned, true).unwrap();
            (retired, pinned)
        };
        let mut buf = Vec::new();
        assert_eq!(run(&a, None, &mut buf).unwrap(), 3);
        let text = String::from_utf8(buf).unwrap();
        assert!(!text.contains("checkpoint"), "{text}");

        let (b, dir_b) = cfg("export-tombstone-b");
        let file = dir_b.join("n.jsonl");
        std::fs::write(&file, &text).unwrap();
        let report = import::run(&b, &file, false).unwrap();
        assert_eq!((report.inserted, report.skipped), (3, 0));

        let cx = crate::plugin::Runtime::open(b.clone(), "verify").unwrap();
        let retired_row = cx.store.note_row(retired_id).unwrap().unwrap();
        assert!(retired_row.retired.is_some());
        assert_eq!(retired_row.body, "body kept");
        let pinned_row = cx.store.note_row(pinned_id).unwrap().unwrap();
        assert!(pinned_row.is_pinned());
        let recall = cx.store.list_note_titles(Some("p"), 5).unwrap();
        assert_eq!(recall.first().map(|(id, _)| *id), Some(pinned_id));
        assert!(
            !recall.iter().any(|(id, _)| *id == retired_id),
            "retired note must not recall"
        );
        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn checkpoint_row_is_absent_from_export_file() {
        let (c, dir) = cfg("export-checkpoint");
        {
            let cx = crate::plugin::Runtime::open(c.clone(), "seed").unwrap();
            cx.store
                .insert_note(Some("p"), "note", "keep", "yes")
                .unwrap();
            cx.store
                .insert_note(Some("p"), "checkpoint:s1", "compact", "no")
                .unwrap();
            cx.store
                .insert_note(Some("p"), "session:s1", "handoff", "no")
                .unwrap();
        }
        let mut buf = Vec::new();
        assert_eq!(run(&c, None, &mut buf).unwrap(), 1);
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(!text.contains("checkpoint"));
        assert!(!text.contains("session:"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
