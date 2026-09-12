//! `compress` — optional semantic shrink after the lossless `archive` lane (P28).
//!
//! When `[plugins.compress] enabled = true`, blocks already archived by `archive` are
//! replaced in-context with a deterministic extractive summary. The archive row is
//! unchanged; `expand <id>` always returns the original bytes.

use serde_json::Value;

use rtok_plugin_sdk::{
    Class, Ctx, DashboardPage, Manifest, Measurement, Plugin, Surface, ToolResultRef, WireRequest,
};

pub struct Compress;

impl Plugin for Compress {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "compress",
            surfaces: &[Surface::Proxy],
            default_on: false,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Compress",
            "Extractive summaries for archived tool output. Off until Gate P28.",
            true,
        )
    }

    fn proxy_filter(&self, req: &mut WireRequest<'_>, cx: &Ctx) -> Vec<Measurement> {
        if !cx
            .plugin_config::<crate::config::Compress>("compress")
            .enabled
        {
            return Vec::new();
        }
        if cx.config::<crate::config::Proxy>("proxy").mode != "compress" {
            return Vec::new();
        }
        rewrite(req.tool_results(), cx)
    }
}

fn rewrite(results: Vec<ToolResultRef<'_>>, cx: &Ctx) -> Vec<Measurement> {
    crate::plugins::archive::outside_live_zone(results, cx)
        .filter_map(|r| summarize_block(&r.id, r.content, cx))
        .collect()
}

fn summarize_block(tool_use_id: &str, content: &mut Value, cx: &Ctx) -> Option<Measurement> {
    let pointer = content.as_str()?;
    let d = match cx.archive_decision(tool_use_id) {
        Ok(Some(d)) if !d.expanded && d.pointer.starts_with("[archived ") => d,
        Ok(_) => return None,
        Err(e) => {
            cx.log("error", "plugin", "compress", &format!("decision: {e}"));
            return None;
        }
    };
    if pointer != d.pointer {
        return None;
    }
    let bytes = cx
        .get_archive(&d.archive_id)
        .map_err(|e| cx.log("error", "plugin", "compress", &format!("get: {e}")))
        .ok()?
        .ok_or_else(|| {
            cx.log("error", "plugin", "compress", "missing archive payload");
        })
        .ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let summary = extractive_summary(&d.archive_id, &text);
    if summary.len() >= pointer.len() {
        return None;
    }
    let m = Measurement {
        plugin: "compress",
        kind: "summary",
        before_bytes: pointer.len() as u64,
        after_bytes: summary.len() as u64,
        est_before: cx.estimate(pointer, Class::Code),
        est_after: cx.estimate(&summary, Class::Code),
        ref_id: Some(d.archive_id.clone()),
        call_id: None,
    };
    *content = Value::String(summary);
    Some(m)
}

/// Deterministic extractive ranker — no network, no LLM (D12).
fn extractive_summary(id: &str, text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let title = lines
        .first()
        .map(|l| l.chars().take(80).collect::<String>());
    let facts: Vec<&str> = lines
        .iter()
        .filter(|l| l.contains(':') || l.contains("error") || l.contains("Error"))
        .take(3)
        .copied()
        .collect();
    let files: Vec<&str> = lines
        .iter()
        .filter(|l| l.contains('/') || l.contains('\\'))
        .take(3)
        .copied()
        .collect();
    let narrative = lines
        .iter()
        .take(4)
        .map(|l| l.chars().take(60).collect::<String>())
        .collect::<Vec<_>>()
        .join("; ");
    let short = &id[..id.len().min(12)];
    let mut s = format!(
        "[compress {short}]\ntype: tool_output\ntitle: {}\nnarrative: {}",
        title.unwrap_or_else(|| "tool result".into()),
        narrative
    );
    if !facts.is_empty() {
        s.push_str("\nfacts:");
        for f in facts {
            s.push_str("\n- ");
            s.push_str(f);
        }
    }
    if !files.is_empty() {
        s.push_str("\nfiles:");
        for f in files {
            s.push_str("\n- ");
            s.push_str(f);
        }
    }
    s.push_str(&format!("\nexpand({id})"));
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::archive::rewrite as archive_rewrite;
    use rtok_plugin_sdk::Archive;

    fn big(tag: &str) -> String {
        (1..=400)
            .map(|i| format!("{tag} line {i}: src/foo.rs: some shell output with words"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn cx(name: &str, compress_on: bool) -> crate::plugin::Runtime {
        let dir = std::env::temp_dir().join(format!("rtok-compress-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = crate::plugin::Runtime::in_memory("s").unwrap();
        cx.config.core.archive_dir = dir;
        cx.config.proxy.mode = "compress".into();
        cx.config.plugins.compress.enabled = compress_on;
        cx
    }

    fn refs<'a>(values: &'a mut [Value]) -> Vec<ToolResultRef<'a>> {
        let total = values.len();
        values
            .iter_mut()
            .enumerate()
            .map(|(index, content)| ToolResultRef {
                id: format!("tu-{}", index + 1),
                content,
                turn: total - index - 1,
            })
            .collect()
    }

    #[test]
    fn default_off_leaves_bytes_identical() {
        use crate::proxy::anthropic::ANTHROPIC;
        use rtok_plugin_sdk::ToolResults;
        let cx = cx("off", false);
        let content = big("t1");
        let mut body = serde_json::json!({
            "messages": (1..=6).map(|i| serde_json::json!({
                "role": "user",
                "content": [{"type":"tool_result","tool_use_id": format!("tu-{i}"), "content": &content}]
            })).collect::<Vec<_>>()
        });
        archive_rewrite(ANTHROPIC.tool_results(&mut body), &Ctx::new(&cx));
        let after_archive = body.clone();
        assert!(
            Compress
                .proxy_filter(&mut WireRequest::new(&ANTHROPIC, &mut body), &Ctx::new(&cx))
                .is_empty()
        );
        assert_eq!(body, after_archive);
    }

    #[test]
    fn fixture_compresses_and_expand_recovers() {
        let cx = cx("on", true);
        let mut values: Vec<Value> = (1..=6)
            .map(|n| Value::String(big(&format!("t{n}"))))
            .collect();
        archive_rewrite(refs(&mut values), &Ctx::new(&cx));
        let ms = rewrite(refs(&mut values), &Ctx::new(&cx));
        assert_eq!(ms.len(), 2);
        assert!(ms[0].after_bytes < ms[0].before_bytes);
        let summary = values[0].as_str().unwrap();
        assert!(summary.starts_with("[compress "));
        let archive_id = ms[0].ref_id.clone().unwrap();
        let recovered = cx.get_archive(&archive_id).unwrap().unwrap();
        assert_eq!(recovered, big("t1").into_bytes());
        assert_eq!(cx.store.count_kind("measurement").unwrap(), 0);
    }

    #[test]
    fn summary_is_deterministic() {
        let cx = cx("det", true);
        let mut first = vec![
            Value::String(big("one")),
            Value::String(big("two")),
            Value::String(big("three")),
            Value::String(big("four")),
            Value::String(big("five")),
            Value::String(big("six")),
        ];
        archive_rewrite(refs(&mut first), &Ctx::new(&cx));
        rewrite(refs(&mut first), &Ctx::new(&cx));
        let body = first.clone();
        let mut second = vec![
            Value::String(big("one")),
            Value::String(big("two")),
            Value::String(big("three")),
            Value::String(big("four")),
            Value::String(big("five")),
            Value::String(big("six")),
        ];
        archive_rewrite(refs(&mut second), &Ctx::new(&cx));
        rewrite(refs(&mut second), &Ctx::new(&cx));
        assert_eq!(body, second);
    }

    #[test]
    fn extractive_summary_shape() {
        let s = extractive_summary("abc123", "title line\nsrc/a.rs: ok\nerr: fail");
        assert!(s.contains("type: tool_output"));
        assert!(s.contains("title: title line"));
        assert!(s.contains("expand(abc123)"));
    }
}
