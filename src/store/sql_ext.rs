//! SQL the typed DSL cannot express. Each item names the construct Diesel 2.3 lacks.

use diesel::prelude::*;
use diesel::query_builder::{AstPass, Query, QueryFragment, QueryId};
use diesel::sql_types::{BigInt, Binary, Integer, Nullable, Text};
use diesel::sqlite::Sqlite;

/// `ROW_NUMBER() OVER (PARTITION BY ended_at ORDER BY id)` — no window functions in
/// Diesel 2.3's typed DSL. Resumes a watermark that covers several sessions in one second.
#[derive(QueryId)]
pub(crate) struct SessionsPending {
    pub mark: i64,
    pub tail: i64,
    pub limit: i64,
}

impl QueryFragment<Sqlite> for SessionsPending {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "SELECT id, host_id, project, cwd, source, started_at, ended_at FROM ( \
                SELECT id, host_id, project, cwd, source, started_at, ended_at, \
                       ROW_NUMBER() OVER (PARTITION BY ended_at ORDER BY id) AS rn \
                FROM sessions \
                WHERE ended_at IS NOT NULL \
             ) WHERE ended_at > ",
        );
        out.push_bind_param::<BigInt, _>(&self.mark)?;
        out.push_sql(" OR (ended_at = ");
        out.push_bind_param::<BigInt, _>(&self.mark)?;
        out.push_sql(" AND rn > ");
        out.push_bind_param::<BigInt, _>(&self.tail)?;
        out.push_sql(") ORDER BY ended_at ASC, id ASC LIMIT ");
        out.push_bind_param::<BigInt, _>(&self.limit)?;
        Ok(())
    }
}

impl Query for SessionsPending {
    type SqlType = (
        Text,
        Nullable<Integer>,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        BigInt,
        Nullable<BigInt>,
    );
}

impl RunQueryDsl<SqliteConnection> for SessionsPending {}

/// `ON CONFLICT DO UPDATE … WHERE` — Diesel 2.3's `DoUpdate` has no WHERE clause, so an
/// unchanged embedding would be rewritten and `embedded_at` would move.
#[derive(QueryId)]
pub(crate) struct UpsertNoteEmbedding {
    pub note_id: i32,
    pub model: String,
    pub dims: i32,
    pub text_hash: String,
    pub vector: Vec<u8>,
}

impl RunQueryDsl<SqliteConnection> for UpsertNoteEmbedding {}

impl QueryFragment<Sqlite> for UpsertNoteEmbedding {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "INSERT INTO note_embeddings (note_id, model, dims, text_hash, vector) VALUES (",
        );
        out.push_bind_param::<Integer, _>(&self.note_id)?;
        out.push_sql(", ");
        out.push_bind_param::<Text, _>(&self.model)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.dims)?;
        out.push_sql(", ");
        out.push_bind_param::<Text, _>(&self.text_hash)?;
        out.push_sql(", ");
        out.push_bind_param::<Binary, _>(&self.vector)?;
        out.push_sql(
            ") ON CONFLICT(note_id) DO UPDATE SET \
               model = excluded.model, \
               dims = excluded.dims, \
               text_hash = excluded.text_hash, \
               embedded_at = unixepoch(), \
               vector = excluded.vector \
             WHERE excluded.text_hash != note_embeddings.text_hash \
                OR excluded.model != note_embeddings.model \
                OR excluded.dims != note_embeddings.dims",
        );
        Ok(())
    }
}
