// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! API proxy request/response parsing for each wire (Anthropic, OpenAI chat/responses,
//! Gemini): session/model/usage extraction, SSE usage, request preparation, the tool-result,
//! skill and live-blob views plugins rewrite, context edits, the semantic-cache key and the
//! skills-listing measure. Pure JSON in, no network.
#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use rtok::proxy::{anthropic, semantic_cache, wire};
use serde_json::Value;

const PATHS: &[&str] = &[
    "/v1/messages",
    "/v1/chat/completions",
    "/v1/responses",
    "/v1beta/models/gemini-2.0-flash:generateContent",
    "/v1/models/gemini-2.0-flash:streamGenerateContent",
];

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    path: u8,
    body: &'a [u8],
    sse: &'a [u8],
    include_usage: bool,
    context_edits: bool,
    upstream: &'a str,
    query: Option<&'a str>,
}

fuzz_target!(|i: Input<'_>| {
    let path = PATHS[usize::from(i.path) % PATHS.len()];
    let _ = rtok::measure::skills_listing::measure(i.body);
    let _ = wire::join_upstream(i.upstream, path, i.query);
    let Some(w) = wire::for_path(path) else {
        return;
    };
    if let Ok(event) = serde_json::from_slice::<Value>(i.sse) {
        let _ = w.usage_from_sse(&event);
    }
    let Ok(mut body) = serde_json::from_slice::<Value>(i.body) else {
        return;
    };
    let _ = (
        w.session_id(&body),
        w.model(path, Some(&body)),
        w.usage_from_body(&body),
    );
    let cfg = rtok::config::SemanticCache::default();
    let _ = semantic_cache::eligible(&body, &cfg);
    let _ = semantic_cache::build_prompt(w, &body, &cfg);
    let _ = w.tool_results(&mut body).len();
    let _ = w.skills(&mut body).len();
    let _ = w.live_blobs(&mut body).len();
    let _ = w.skill_refs(&mut body).len();
    let _ = w.prepare_request(&mut body, i.include_usage);
    let _ = anthropic::apply_context_edits(&mut body, i.context_edits);
});
