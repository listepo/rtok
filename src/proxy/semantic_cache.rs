//! Opt-in two-tier response cache (P31). Off when `enabled = false` or `embed_backend = "hash"`
//! (direct tier only). See `src/plugins/proxy/PLAN.md` v0.2.

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use axum::body::Bytes;
use rtok_plugin_sdk::Measurement;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::config::SemanticCache;
use crate::proxy::wire::Wire;

#[derive(Debug, Clone, Serialize)]
pub struct CachePrompt {
    pub provider: String,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    pub messages: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools_fingerprint: Option<String>,
}

pub struct CacheHit {
    pub response: Bytes,
    pub content_type: Option<String>,
    pub status: u16,
    pub similarity: f32,
    pub direct: bool,
    pub hash_hex: String,
}

struct Entry {
    hash: [u8; 32],
    response: Bytes,
    content_type: Option<String>,
    status: u16,
    at: Instant,
}

pub struct Cache {
    direct: HashMap<[u8; 32], Entry>,
    semantic: Vec<(Vec<f32>, Entry)>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            direct: HashMap::new(),
            semantic: Vec::new(),
        }
    }

    pub fn lookup(&self, prompt: &CachePrompt, cfg: &SemanticCache) -> Option<CacheHit> {
        let hash = canonical_hash(prompt);
        let ttl = Duration::from_secs(cfg.ttl_s.max(1));
        if let Some(e) = self.direct.get(&hash).filter(|e| e.at.elapsed() < ttl) {
            return Some(hit_from(e, 1.0, true, &hash));
        }
        if cfg.embed_backend == "hash" {
            return None;
        }
        let query = embed_vector(prompt);
        let mut best: Option<(f32, &Entry)> = None;
        for (emb, e) in &self.semantic {
            if e.at.elapsed() >= ttl {
                continue;
            }
            let sim = cosine(emb, &query);
            if sim >= cfg.threshold {
                if best.map_or(true, |(s, _)| sim > s) {
                    best = Some((sim, e));
                }
            }
        }
        best.map(|(sim, e)| hit_from(e, sim, false, &e.hash))
    }

    pub fn store(
        &mut self,
        prompt: &CachePrompt,
        cfg: &SemanticCache,
        response: &[u8],
        content_type: Option<&str>,
        status: u16,
    ) {
        let hash = canonical_hash(prompt);
        let at = Instant::now();
        let response = Bytes::copy_from_slice(response);
        let content_type = content_type.map(str::to_string);
        let embedding = (cfg.embed_backend != "hash").then(|| embed_vector(prompt));
        if let Some(emb) = embedding {
            self.semantic.push((
                emb,
                Entry {
                    hash,
                    response: response.clone(),
                    content_type: content_type.clone(),
                    status,
                    at,
                },
            ));
        }
        self.direct.insert(
            hash,
            Entry {
                hash,
                response,
                content_type,
                status,
                at,
            },
        );
    }
}

fn hit_from(e: &Entry, similarity: f32, direct: bool, hash: &[u8; 32]) -> CacheHit {
    CacheHit {
        response: e.response.clone(),
        content_type: e.content_type.clone(),
        status: e.status,
        similarity,
        direct,
        hash_hex: hex8(hash),
    }
}

pub fn build_prompt(wire: &dyn Wire, body: &Value, cfg: &SemanticCache) -> Option<CachePrompt> {
    let model = body.get("model")?.as_str()?.to_string();
    Some(CachePrompt {
        provider: cfg
            .cache_by_provider
            .then_some(wire.provider().to_string())
            .unwrap_or_default(),
        model: cfg.cache_by_model.then_some(model).unwrap_or_default(),
        system: system_text(body),
        messages: messages_text(body),
        tools_fingerprint: tools_fingerprint(body),
    })
}

pub fn eligible(body: &Value, cfg: &SemanticCache) -> bool {
    if body.get("stream") == Some(&Value::Bool(true)) {
        return false;
    }
    let messages = body.get("messages").and_then(Value::as_array);
    let users = messages
        .map(|messages| {
            messages
                .iter()
                .filter(|m| m.get("role") == Some(&Value::String("user".into())))
                .count()
        })
        .unwrap_or(0);
    if users == 0 || users > cfg.max_messages as usize {
        return false;
    }
    if cfg.require_empty_tools {
        if let Some(tools) = body.get("tools").and_then(Value::as_array) {
            if !tools.is_empty() {
                return false;
            }
        }
    }
    true
}

pub fn measurement(hit: &CacheHit, call_id: Option<i32>, nbytes: usize) -> Measurement {
    let est = (nbytes / 4).max(1) as u32;
    Measurement {
        plugin: "proxy",
        kind: "semantic_cache_hit",
        before_bytes: nbytes as u64,
        after_bytes: nbytes as u64,
        est_before: est,
        est_after: est,
        ref_id: Some(format!("{:.4}:{}", hit.similarity, hit.hash_hex)),
        call_id,
    }
}

#[derive(Debug)]
pub struct AuditReport {
    pub false_hit_pairs: u32,
    pub semantic_pairs: u32,
    pub hit_rate: f32,
}

pub fn audit_corpus(dir: &Path, cfg: &SemanticCache) -> Result<AuditReport, String> {
    let entries = load_corpus(dir)?;
    let n = entries.len();
    let mut false_hits = 0u32;
    let mut semantic = 0u32;
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            if entries[i].hash == entries[j].hash {
                continue;
            }
            if cfg.embed_backend == "hash" {
                continue;
            }
            let sim = cosine(&entries[i].embedding, &entries[j].embedding);
            if sim >= cfg.threshold {
                semantic += 1;
                if entries[i].response != entries[j].response {
                    false_hits += 1;
                }
            }
        }
    }
    let pairs = n * (n - 1);
    Ok(AuditReport {
        false_hit_pairs: false_hits,
        semantic_pairs: semantic,
        hit_rate: if pairs == 0 {
            0.0
        } else {
            semantic as f32 / pairs as f32
        },
    })
}

struct CorpusEntry {
    hash: [u8; 32],
    embedding: Vec<f32>,
    response: Vec<u8>,
}

fn load_corpus(dir: &Path) -> Result<Vec<CorpusEntry>, String> {
    let manifest = dir.join("corpus.json");
    let raw = std::fs::read_to_string(&manifest)
        .map_err(|e| format!("read {}: {e}", manifest.display()))?;
    let doc: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    let entries = doc
        .get("entries")
        .and_then(|v| v.as_array())
        .ok_or("corpus.json missing entries")?;
    entries
        .iter()
        .map(|row| {
            let id = row.get("id").and_then(|v| v.as_str()).ok_or("entry id")?;
            let req = dir.join(
                row.get("request")
                    .and_then(|v| v.as_str())
                    .ok_or("entry request")?,
            );
            let resp = dir.join(
                row.get("response")
                    .and_then(|v| v.as_str())
                    .ok_or("entry response")?,
            );
            let body: Value = serde_json::from_slice(
                &std::fs::read(&req).map_err(|e| format!("{id} request: {e}"))?,
            )
            .map_err(|e| format!("{id} request json: {e}"))?;
            let wire = crate::proxy::wire::for_path("/v1/messages").expect("wire");
            let cfg = SemanticCache::default();
            let prompt = build_prompt(wire, &body, &cfg).ok_or_else(|| format!("{id} prompt"))?;
            let hash = canonical_hash(&prompt);
            let response = std::fs::read(&resp).map_err(|e| format!("{id} response: {e}"))?;
            let embedding = row
                .get("embedding")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
                .unwrap_or_else(|| embed_vector(&prompt));
            Ok(CorpusEntry {
                hash,
                embedding,
                response,
            })
        })
        .collect()
}

pub fn canonical_hash(prompt: &CachePrompt) -> [u8; 32] {
    let json = serde_json::to_vec(prompt).unwrap_or_default();
    let mut h = Sha256::new();
    h.update(&json);
    h.finalize().into()
}

fn messages_text(body: &Value) -> Vec<(String, String)> {
    body.get("messages")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|m| {
                    let role = m.get("role")?.as_str()?;
                    let content = m.get("content")?;
                    Some((role.to_string(), content_text(content)))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn system_text(body: &Value) -> Option<String> {
    body.get("system").map(content_text)
}

fn content_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| b.get("text").and_then(Value::as_str).or_else(|| b.as_str()))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn tools_fingerprint(body: &Value) -> Option<String> {
    let tools = body.get("tools")?.as_array()?;
    if tools.is_empty() {
        return None;
    }
    let mut h = Sha256::new();
    for t in tools {
        if let Some(name) = t.get("name").and_then(Value::as_str) {
            h.update(name.as_bytes());
            h.update([0]);
        }
    }
    Some(hex8(&h.finalize().into()))
}

fn embed_vector(prompt: &CachePrompt) -> Vec<f32> {
    let last_user = prompt
        .messages
        .iter()
        .rev()
        .find(|(r, _)| r == "user")
        .map(|(_, t)| t.as_str())
        .unwrap_or("");
    let text = format!("{}:{}:{}", prompt.provider, prompt.model, last_user);
    let mut out = vec![0.0f32; 8];
    for (i, chunk) in text.as_bytes().chunks(4).enumerate() {
        let mut acc = 0u32;
        for b in chunk {
            acc = (acc << 8) | u32::from(*b);
        }
        out[i % 8] += (acc as f32) / 1_000_000_000.0;
    }
    let norm = out.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut out {
            *x /= norm;
        }
    }
    out
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n == 0 {
        return 0.0;
    }
    let dot = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum::<f32>();
    let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn hex8(hash: &[u8; 32]) -> String {
    hash.iter().take(4).map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn direct_hit_on_exact_replay() {
        let body = serde_json::json!({
            "model": "claude-test",
            "messages": [{"role": "user", "content": "hello"}]
        });
        let cfg = SemanticCache::default();
        let wire = crate::proxy::wire::for_path("/v1/messages").unwrap();
        let prompt = build_prompt(wire, &body, &cfg).unwrap();
        let mut cache = Cache::new();
        cache.store(&prompt, &cfg, b"resp", Some("application/json"), 200);
        let hit = cache.lookup(&prompt, &cfg).unwrap();
        assert!(hit.direct);
        assert_eq!(hit.response.as_ref(), b"resp");
    }

    #[test]
    fn p9_fixture_audit_zero_false_hits() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/p9_semantic_cache_corpus");
        let cfg = SemanticCache {
            enabled: true,
            threshold: 0.99,
            ttl_s: 300,
            max_messages: 1,
            require_empty_tools: true,
            embed_backend: "fixture".into(),
            cache_by_model: true,
            cache_by_provider: true,
        };
        let report = audit_corpus(&dir, &cfg).expect("audit");
        assert_eq!(
            report.false_hit_pairs, 0,
            "false hits at {:.2} hit_rate",
            report.hit_rate
        );
    }

    #[test]
    fn hash_backend_skips_semantic_tier() {
        let a = serde_json::json!({
            "model": "m",
            "messages": [{"role": "user", "content": "a"}]
        });
        let b = serde_json::json!({
            "model": "m",
            "messages": [{"role": "user", "content": "b"}]
        });
        let cfg = SemanticCache::default();
        let wire = crate::proxy::wire::for_path("/v1/messages").unwrap();
        let pa = build_prompt(wire, &a, &cfg).unwrap();
        let pb = build_prompt(wire, &b, &cfg).unwrap();
        let mut cache = Cache::new();
        cache.store(&pa, &cfg, b"ra", None, 200);
        assert!(cache.lookup(&pb, &cfg).is_none());
    }
}
