//! Off-by-default sub-agent handoff stub (plan T59.6).

use rtok_plugin_sdk::{Class, Ctx, Measurement};

const SHARE: &str = "23 K of 2.83 M (0.8%)";

pub fn handoff(cx: &Ctx, budget_tokens: u32) -> String {
    let cfg = cx.plugin_config::<crate::config::Memory>("memory");
    let text = if !cfg.handoff {
        format!(
            "handoff disabled: Agent/Task results were {SHARE} on the measured workload; \
             enable [plugins.memory] handoff after sub-agents exceed 5 %."
        )
    } else {
        format!(
            "handoff(budget_tokens={budget_tokens}) is enabled but not yet measured on this workload."
        )
    };
    let before = 0u64;
    let after = text.len() as u64;
    let _ = cx.record(&Measurement {
        plugin: "memory",
        kind: "handoff",
        before_bytes: before,
        after_bytes: after,
        est_before: 0,
        est_after: cx.estimate(&text, Class::Prose),
        ref_id: Some(SHARE.into()),
        call_id: None,
    });
    text
}

pub fn handoff_tool() -> rtok_plugin_sdk::ToolDef {
    rtok_plugin_sdk::ToolDef {
        name: "handoff",
        description: "Budgeted session digest for sub-agents (off by default).",
        input_schema: serde_json::json!({
            "type":"object",
            "properties":{"budget_tokens":{"type":"integer"}},
            "required":["budget_tokens"]
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rtok_plugin_sdk::Ctx;

    #[test]
    fn stub_records_measurement_when_off() {
        let cx = crate::plugin::Runtime::in_memory("t596").unwrap();
        let ctx = Ctx::new(&cx);
        let out = handoff(&ctx, 800);
        assert!(out.contains("disabled"));
        assert!(cx.store.measurement_count("memory").unwrap() >= 1);
    }
}
