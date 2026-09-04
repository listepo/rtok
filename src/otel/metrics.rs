//! T16.7 (D19): cumulative, monotonic sums from whole-table aggregates. No watermark: the
//! same numbers `rtok stats` prints, re-posted each flush with a later timestamp.

use anyhow::Result;

use super::otlp::{Sum, b, s, seconds_to_ns};
use crate::store::Store;

/// `gen_ai.provider.name` from the wire's `api` slug (`openai_chat` → `openai`).
fn provider(api: &str) -> &str {
    if api.starts_with("openai") {
        "openai"
    } else {
        api
    }
}

pub fn sums(store: &Store, now_ns: u64) -> Result<Vec<Sum>> {
    let start_ns = seconds_to_ns(store.otel_first_ts()?);
    let sum = |name: &str, unit: &str, description: &str, points| Sum {
        name: name.into(),
        unit: unit.into(),
        description: description.into(),
        start_ns,
        time_ns: now_ns,
        points,
    };

    let mut tokens = Vec::new();
    for t in store.otel_token_totals()? {
        let model = t.model.clone().unwrap_or_else(|| "unknown".into());
        for (kind, n) in [
            ("input", t.input),
            ("output", t.output),
            ("cache_read", t.cache_read),
            ("cache_creation", t.cache_create),
        ] {
            tokens.push((
                vec![
                    s("gen_ai.token.type", kind),
                    s("gen_ai.request.model", &model),
                    s("gen_ai.provider.name", provider(&t.api)),
                    s("rtok.api", &t.api),
                ],
                n,
            ));
        }
    }
    let saved = store
        .otel_saved_totals()?
        .into_iter()
        .map(|r| {
            (
                vec![s("rtok.plugin", &r.plugin), s("rtok.kind", &r.kind)],
                r.saved,
            )
        })
        .collect();
    let calls = store
        .otel_call_totals()?
        .into_iter()
        .map(|r| {
            (
                vec![
                    s("rtok.surface", &r.surface),
                    s("rtok.kind", &r.kind),
                    b("rtok.ok", r.ok != 0),
                ],
                r.n,
            )
        })
        .collect();
    Ok(vec![
        sum(
            "rtok.tokens",
            "{token}",
            "Tokens billed by the provider, from the proxy's usage ledger",
            tokens,
        ),
        sum(
            "rtok.tokens.saved",
            "{token}",
            "Estimated tokens removed by rtok plugins (est_before − est_after)",
            saved,
        ),
        sum(
            "rtok.calls",
            "{call}",
            "Hook, MCP and proxied calls recorded",
            calls,
        ),
    ])
}
