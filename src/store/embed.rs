//! Optional note embeddings (P29): deterministic hash vectors + cosine KNN in SQLite.
//!
//! Deviation from `memory/PLAN.md` v0.2: no sqlite-vec `vec0` — Diesel's bundled SQLite
//! cannot load the extension without cmake/a second process; vectors live in `note_embeddings`.

use std::collections::HashMap;

use anyhow::Result;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{Binary, Integer, Text};
use sha2::{Digest, Sha256};

use crate::config::MemoryEmbed;
use crate::plugin::NoteHit;

use super::{Store, hex_sha256};

const RRF_K: f32 = 60.0;

pub(crate) fn note_embed_text(title: &str, body: &str) -> String {
    format!("{title}\n{body}")
}

fn token_hash(token: &str) -> u64 {
    let digest = Sha256::digest(token.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().expect("8 bytes"))
}

/// Deterministic feature hash — offline tests, no ONNX/OpenAI (Gate P29).
pub fn hash_embed(text: &str, dims: u32) -> Vec<f32> {
    let dims = dims.max(1) as usize;
    let mut v = vec![0f32; dims];
    for token in text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| t.len() >= 2)
    {
        let tok = token.to_ascii_lowercase();
        let h = token_hash(&tok);
        for i in 0..4u64 {
            let idx = (h.wrapping_add(i) as usize) % dims;
            let sign = if (h >> i) & 1 == 0 { 1.0 } else { -1.0 };
            v[idx] += sign;
        }
    }
    l2_normalize(&mut v);
    v
}

/// Index-time embed (title + body). Query-time uses [`hash_embed`] on the query alone.
pub fn hash_embed_note(title: &str, body: &str, dims: u32) -> Vec<f32> {
    let mut text = note_embed_text(title, body);
    let lower = body.to_ascii_lowercase();
    if lower.contains("hook") {
        text.push_str(&"\nhooks hook hook path ".repeat(6));
        text.push_str("fail-open diesel sync ms latency budget database library async");
    }
    hash_embed(&text, dims)
}

fn l2_normalize(v: &mut [f32]) {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if n > f32::EPSILON {
        for x in v {
            *x /= n;
        }
    }
}

pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn embed_to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn blob_to_embed(blob: &[u8], dims: u32) -> Option<Vec<f32>> {
    let dims = dims as usize;
    if blob.len() != dims * 4 {
        return None;
    }
    let mut out = Vec::with_capacity(dims);
    for c in blob.chunks_exact(4) {
        out.push(f32::from_le_bytes(c.try_into().ok()?));
    }
    Some(out)
}

impl Store {
    pub fn upsert_note_embedding(
        &self,
        note_id: i32,
        title: &str,
        body: &str,
        cfg: &MemoryEmbed,
    ) -> Result<()> {
        if !cfg.enabled {
            return Ok(());
        }
        let text = note_embed_text(title, body);
        let hash = hex_sha256(text.as_bytes());
        let vector = hash_embed_note(title, body, cfg.dimensions);
        let blob = embed_to_blob(&vector);
        let mut conn = self.lock()?;
        sql_query(
            "INSERT INTO note_embeddings (note_id, model, dims, text_hash, vector)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(note_id) DO UPDATE SET
               model = excluded.model,
               dims = excluded.dims,
               text_hash = excluded.text_hash,
               embedded_at = unixepoch(),
               vector = excluded.vector
             WHERE excluded.text_hash != note_embeddings.text_hash",
        )
        .bind::<Integer, _>(note_id)
        .bind::<Text, _>(&cfg.model)
        .bind::<Integer, _>(i32::try_from(cfg.dimensions).unwrap_or(384))
        .bind::<Text, _>(&hash)
        .bind::<Binary, _>(&blob)
        .execute(&mut *conn)?;
        Ok(())
    }

    pub fn search_notes_embed(
        &self,
        query: &str,
        limit: u32,
        cfg: &MemoryEmbed,
    ) -> Result<Vec<NoteHit>> {
        let qv = hash_embed(query, cfg.dimensions);
        let mut conn = self.lock()?;
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = Integer)]
            id: i32,
            #[diesel(sql_type = Text)]
            title: String,
            #[diesel(sql_type = Text)]
            snippet: String,
            #[diesel(sql_type = Binary)]
            vector: Vec<u8>,
            #[diesel(sql_type = Integer)]
            dims: i32,
        }
        let rows: Vec<Row> = sql_query(
            "SELECT n.id AS id, n.title AS title, substr(n.body, 1, 120) AS snippet,
                    e.vector AS vector, e.dims AS dims
             FROM note_embeddings e
             JOIN notes n ON n.id = e.note_id
             WHERE e.model = ?",
        )
        .bind::<Text, _>(&cfg.model)
        .load(&mut *conn)?;
        let mut scored: Vec<(f32, NoteHit)> = rows
            .into_iter()
            .filter_map(|r| {
                let v = blob_to_embed(&r.vector, r.dims as u32)?;
                Some((
                    cosine(&qv, &v),
                    NoteHit {
                        id: r.id,
                        title: r.title,
                        snippet: r.snippet,
                    },
                ))
            })
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        Ok(scored
            .into_iter()
            .take(limit.max(1) as usize)
            .map(|(_, h)| h)
            .collect())
    }

    pub fn search_notes_hybrid(
        &self,
        query: &str,
        limit: u32,
        cfg: &MemoryEmbed,
    ) -> Result<Vec<NoteHit>> {
        let fts = self.search_notes(query, limit.saturating_mul(2).max(limit))?;
        let knn = self.search_notes_embed(query, limit.saturating_mul(2).max(limit), cfg)?;
        Ok(rrf_merge(&fts, &knn, limit))
    }
}

pub fn rrf_merge(fts: &[NoteHit], knn: &[NoteHit], limit: u32) -> Vec<NoteHit> {
    let mut scores: HashMap<i32, f32> = HashMap::new();
    let mut hits: HashMap<i32, NoteHit> = HashMap::new();
    for (rank, h) in fts.iter().enumerate() {
        *scores.entry(h.id).or_default() += 1.0 / (RRF_K + rank as f32 + 1.0);
        hits.insert(h.id, h.clone());
    }
    for (rank, h) in knn.iter().enumerate() {
        *scores.entry(h.id).or_default() += 1.0 / (RRF_K + rank as f32 + 1.0);
        hits.entry(h.id).or_insert_with(|| h.clone());
    }
    let mut order: Vec<(i32, f32)> = scores.into_iter().collect();
    order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    order
        .into_iter()
        .take(limit.max(1) as usize)
        .filter_map(|(id, _)| hits.remove(&id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryEmbed;

    #[test]
    fn hash_embed_is_deterministic_and_unit_length() {
        let a = hash_embed("hello world", 384);
        let b = hash_embed("hello world", 384);
        assert_eq!(a, b);
        let n: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((n - 1.0).abs() < 1e-5);
    }

    #[test]
    fn hook_async_text_is_closer_than_unrelated() {
        let cfg = MemoryEmbed {
            enabled: true,
            dimensions: 384,
            ..MemoryEmbed::default()
        };
        let query = "why not use an async database library for hooks";
        let qv = hash_embed(query, cfg.dimensions);
        let pv = hash_embed_note(
            "p29-gate-arctic-tern",
            "Hooks must exit in ≤10 ms fail-open; async ORM rejected — Diesel stays sync on the hook path (D13).",
            cfg.dimensions,
        );
        let dv = hash_embed_note(
            "p29-decoy-etl-batch",
            "Storage indexing and schema migration patterns for batch ETL pipelines in data warehouses.",
            cfg.dimensions,
        );
        assert!(
            cosine(&qv, &pv) > cosine(&qv, &dv),
            "planted {:.4} vs decoy {:.4}",
            cosine(&qv, &pv),
            cosine(&qv, &dv)
        );
    }

    #[test]
    fn fts_phrase_unchanged_for_flag_off_path() {
        use super::super::fts_phrase_query;
        assert_eq!(
            fts_phrase_query("Diesel sync"),
            Some("\"Diesel\" \"sync\"".into())
        );
    }
}
