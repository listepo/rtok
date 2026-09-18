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

/// Stored in `note_embeddings.model` after `[embed] model`. Bump it whenever [`hash_embed`]
/// changes: every older vector then reads as stale and [`Store::embed_stale`] redoes it.
const SCHEME: &str = "hash2";

fn model_key(cfg: &MemoryEmbed) -> String {
    format!("{}+{SCHEME}", cfg.model)
}

fn dims(cfg: &MemoryEmbed) -> i32 {
    i32::try_from(cfg.dimensions).unwrap_or(384)
}

/// Deterministic feature hash — offline tests, no ONNX/OpenAI (Gate P29). A token lands in
/// four slots, one per independent 8-byte slice of its SHA-256. The first scheme used four
/// consecutive slots (`h..h+3`) with signs from adjacent bits of one hash, so two tokens
/// that collided did so in runs; that noise decided the P29 hybrid ranking. Words are Unicode:
/// splitting on ASCII alphanumerics embedded a Cyrillic note as the zero vector.
pub fn hash_embed(text: &str, dims: u32) -> Vec<f32> {
    let dims = u64::from(dims.max(1));
    let mut v = vec![0f32; dims as usize];
    for token in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().nth(1).is_some())
    {
        let digest = Sha256::digest(token.to_lowercase().as_bytes());
        for chunk in digest.chunks_exact(8) {
            let h = u64::from_le_bytes(chunk.try_into().expect("8 bytes"));
            v[(h % dims) as usize] += if h >> 63 == 0 { 1.0 } else { -1.0 };
        }
    }
    l2_normalize(&mut v);
    v
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
        // The note's own text and nothing else. Index time used to append hook/database words
        // to any body mentioning "hook", tuned to the P29 fixture, so such notes outranked
        // better matches on every hook-flavoured query.
        let vector = hash_embed(&text, cfg.dimensions);
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
             WHERE excluded.text_hash != note_embeddings.text_hash
                OR excluded.model != note_embeddings.model
                OR excluded.dims != note_embeddings.dims",
        )
        .bind::<Integer, _>(note_id)
        .bind::<Text, _>(model_key(cfg))
        .bind::<Integer, _>(dims(cfg))
        .bind::<Text, _>(&hash)
        .bind::<Binary, _>(&blob)
        .execute(&mut *conn)?;
        Ok(())
    }

    /// Embed every note whose vector is missing or was made under another model, [`SCHEME`]
    /// or `dimensions`. Only `mem_save` with `[embed]` on wrote vectors, so notes saved before
    /// the flag was turned on, or before `dimensions` changed, never reached the KNN leg.
    fn embed_stale(&self, cfg: &MemoryEmbed) -> Result<()> {
        if !cfg.enabled {
            return Ok(());
        }
        #[derive(QueryableByName)]
        struct Stale {
            #[diesel(sql_type = Integer)]
            id: i32,
            #[diesel(sql_type = Text)]
            title: String,
            #[diesel(sql_type = Text)]
            body: String,
        }
        let stale: Vec<Stale> = sql_query(
            "SELECT n.id AS id, n.title AS title, n.body AS body
             FROM notes n LEFT JOIN note_embeddings e ON e.note_id = n.id
             WHERE e.note_id IS NULL OR e.model != ?1 OR e.dims != ?2",
        )
        .bind::<Text, _>(model_key(cfg))
        .bind::<Integer, _>(dims(cfg))
        .load(&mut *self.lock()?)?;
        for n in stale {
            self.upsert_note_embedding(n.id, &n.title, &n.body, cfg)?;
        }
        Ok(())
    }

    pub fn search_notes_embed(
        &self,
        query: &str,
        limit: u32,
        cfg: &MemoryEmbed,
    ) -> Result<Vec<NoteHit>> {
        self.embed_stale(cfg)?;
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
        // Only vectors of the query's own model, scheme and `dimensions` are scored: `cosine`
        // zips to the shorter vector, so a stale 384-dim row against an 8-dim query ranked on
        // noise. `embed_stale` has just brought every row up to date.
        let rows: Vec<Row> = sql_query(
            "SELECT n.id AS id, n.title AS title, substr(n.body, 1, 120) AS snippet,
                    e.vector AS vector, e.dims AS dims
             FROM note_embeddings e
             JOIN notes n ON n.id = e.note_id
             WHERE e.model = ? AND e.dims = ? AND n.retired IS NULL",
        )
        .bind::<Text, _>(model_key(cfg))
        .bind::<Integer, _>(dims(cfg))
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

    /// ASCII-only words made every Cyrillic note the zero vector, unreachable by KNN.
    #[test]
    fn a_cyrillic_note_has_a_vector() {
        let a = hash_embed("хуки не блокируют", 64);
        assert!(a.iter().any(|x| *x != 0.0));
        assert!(cosine(&a, &hash_embed("Хуки НЕ блокируют", 64)) > 0.999);
        assert!(cosine(&a, &hash_embed("миграция схемы базы", 64)) < 0.5);
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
        let pv = hash_embed(
            &note_embed_text(
                "p29-gate-arctic-tern",
                "Hooks must exit in ≤10 ms fail-open; async ORM rejected — Diesel stays sync on the hook path (D13).",
            ),
            cfg.dimensions,
        );
        let dv = hash_embed(
            &note_embed_text(
                "p29-decoy-etl-batch",
                "Storage indexing and schema migration patterns for batch ETL pipelines in data warehouses.",
            ),
            cfg.dimensions,
        );
        assert!(
            cosine(&qv, &pv) > cosine(&qv, &dv),
            "planted {:.4} vs decoy {:.4}",
            cosine(&qv, &pv),
            cosine(&qv, &dv)
        );
    }

    /// A vector from another `dimensions` and a note saved while `[embed]` was off both reach
    /// KNN: search re-embeds them first instead of scoring or skipping them.
    #[test]
    fn stale_and_missing_vectors_are_embedded_before_knn() {
        let s = Store::open_in_memory().unwrap();
        let wide = MemoryEmbed {
            enabled: true,
            dimensions: 384,
            ..MemoryEmbed::default()
        };
        let narrow = MemoryEmbed {
            dimensions: 8,
            ..wide.clone()
        };
        let id = s.insert_note(None, "note", "t", "alpha beta").unwrap();
        s.upsert_note_embedding(id, "t", "alpha beta", &wide)
            .unwrap();
        s.insert_note(None, "note", "u", "alpha gamma").unwrap(); // saved with embed off
        assert_eq!(s.search_notes_embed("alpha", 5, &narrow).unwrap().len(), 2);
    }

    /// A note that only says "hook" must not outrank one that holds both query words: the
    /// old index-time boost padded it with `hooks`/`path` and it won.
    #[test]
    fn a_passing_mention_of_hook_is_not_boosted() {
        let s = Store::open_in_memory().unwrap();
        let cfg = MemoryEmbed {
            enabled: true,
            dimensions: 384,
            ..MemoryEmbed::default()
        };
        for (title, body) in [
            ("aa", "hook"),
            ("bb", "hooks path guide for new contributors on the team"),
        ] {
            let id = s.insert_note(None, "note", title, body).unwrap();
            s.upsert_note_embedding(id, title, body, &cfg).unwrap();
        }
        let top = s.search_notes_embed("hooks path", 1, &cfg).unwrap();
        assert_eq!(top[0].title, "bb");
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
