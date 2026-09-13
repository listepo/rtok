//! Re-read dedup (plan T4.4): same session/path/mode/range + sha256 → short hit.

use rtok_plugin_sdk::{Class, Ctx, Measurement, PostToolUse};

pub fn key(path: &str, mode: &str, range: Option<&str>) -> String {
    format!("{path}\t{mode}\t{}", range.unwrap_or(""))
}

pub fn hit(cx: &Ctx, key: &str, body: &[u8], lines: usize) -> Option<String> {
    let (Some(id), _) = cx.get_read_cache(key).ok().flatten()? else {
        return None;
    };
    let prev = cx.get_archive(&id).ok().flatten()?;
    if prev.as_slice() != body {
        return None;
    }
    let msg = format!("unchanged since {id} ({lines} lines)");
    let msg = if msg.len() < 80 {
        msg
    } else {
        format!("unchanged since {:.8} ({lines} lines)", id)
    };
    let _ = cx.record(&Measurement {
        plugin: "read",
        kind: "dedup",
        before_bytes: body.len() as u64,
        after_bytes: msg.len() as u64,
        est_before: cx.estimate(std::str::from_utf8(body).unwrap_or(""), Class::Code),
        est_after: cx.estimate(&msg, Class::Code),
        ref_id: Some(id),
        call_id: None,
    });
    Some(msg)
}

pub fn remember(cx: &Ctx, key: &str, body: &[u8]) -> anyhow::Result<String> {
    let id = cx.put_archive(body)?;
    cx.put_read_cache(key, &hex_sha256(body), Some(&id))?;
    Ok(id)
}

pub fn invalidate(ev: &PostToolUse<'_>, cx: &Ctx) {
    if ev.tool_name != "Edit" && ev.tool_name != "Write" {
        return;
    }
    let Some(path) = ev
        .tool_input
        .get("file_path")
        .or_else(|| ev.tool_input.get("path"))
        .and_then(|v| v.as_str())
    else {
        return;
    };
    let _ = cx.clear_read_cache(path);
    let _ = cx.clear_read_cache(&format!("read:{path}"));
}

fn hex_sha256(bytes: &[u8]) -> String {
    crate::store::hex_sha256(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::read::read;
    use crate::plugins::read::tests::cx;
    use serde_json::json;
    use std::fs;

    #[test]
    fn two_identical_reads_second_is_short() {
        let (cx, dir) = cx("same");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\nbeta\n").unwrap();
        let path = p.to_str().unwrap();
        let first = read(&Ctx::new(&cx), path, "full", None).unwrap();
        assert!(first.contains("1:alpha"), "{first}");
        let second = read(&Ctx::new(&cx), path, "full", None).unwrap();
        assert!(second.len() < 80, "{second}");
        assert!(second.contains("unchanged"), "{second}");
        assert!(cx.store.measurement_count("read").unwrap() >= 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn edit_fixture_between_reads_is_full() {
        let (cx, dir) = cx("edit");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\n").unwrap();
        let path = p.to_str().unwrap();
        let _ = read(&Ctx::new(&cx), path, "full", None).unwrap();
        fs::write(&p, "omega\n").unwrap();
        let second = read(&Ctx::new(&cx), path, "full", None).unwrap();
        assert!(second.contains("1:omega"), "{second}");
        assert!(!second.contains("unchanged"), "{second}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn post_tool_edit_clears_hit() {
        let (cx, dir) = cx("hook");
        let p = dir.join("a.txt");
        fs::write(&p, "alpha\n").unwrap();
        let path = p.to_str().unwrap();
        let _ = read(&Ctx::new(&cx), path, "full", None).unwrap();
        let input = json!({"file_path": path});
        invalidate(
            &PostToolUse {
                tool_name: "Edit",
                tool_input: &input,
                tool_response: &json!({}),
            },
            &Ctx::new(&cx),
        );
        let second = read(&Ctx::new(&cx), path, "full", None).unwrap();
        assert!(second.contains("1:alpha"), "{second}");
        let _ = fs::remove_dir_all(dir);
    }
}
