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

// `COALESCE` for an upsert `DO UPDATE SET` (T163.5): not one of Diesel's built-ins.
#[diesel::declare_sql_function]
extern "SQL" {
    fn coalesce<T: diesel::sql_types::SqlType + diesel::sql_types::SingleValue>(
        x: diesel::sql_types::Nullable<T>,
        y: diesel::sql_types::Nullable<T>,
    ) -> diesel::sql_types::Nullable<T>;
}

// SQLite `length()`: character count of a TEXT value, not byte count.
diesel::define_sql_function!(fn length(x: Text) -> BigInt);

// `SUM` as `Nullable<BigInt>`: Diesel's `sum()` widens to `Nullable<Numeric>`.
diesel::define_sql_function! {
    #[aggregate]
    #[sql_name = "SUM"]
    fn sum_bigint(x: BigInt) -> Nullable<BigInt>;
}

/// `substr(text, start, length)` — not one of Diesel's built-ins (T163.6).
#[diesel::declare_sql_function]
extern "SQL" {
    fn substr(
        x: diesel::sql_types::Text,
        start: diesel::sql_types::Integer,
        length: diesel::sql_types::Integer,
    ) -> diesel::sql_types::Text;
}

// SQLite `unixepoch()` — not a Diesel built-in.
diesel::define_sql_function!(fn unixepoch() -> Nullable<BigInt>);

/// `PRAGMA` does not accept a bound parameter, and `journal_mode` is ignored when Diesel
/// runs it as a prepared statement (`execute` left the file in `delete` mode). `sqlite3_exec`
/// is what applies it. The text is a literal this module builds.
fn exec_pragma(conn: &mut SqliteConnection, sql: &str) -> QueryResult<()> {
    use diesel::connection::SimpleConnection;
    conn.batch_execute(sql)
}

/// Milliseconds are a duration we computed, written as digits.
pub(crate) fn busy_timeout(conn: &mut SqliteConnection, ms: u128) -> QueryResult<()> {
    exec_pragma(conn, &format!("PRAGMA busy_timeout = {ms}"))
}

pub(crate) fn pragma_journal_wal(conn: &mut SqliteConnection) -> QueryResult<()> {
    exec_pragma(conn, "PRAGMA journal_mode = WAL")
}

pub(crate) fn pragma_synchronous_normal(conn: &mut SqliteConnection) -> QueryResult<()> {
    exec_pragma(conn, "PRAGMA synchronous = NORMAL")
}

pub(crate) fn pragma_foreign_keys_on(conn: &mut SqliteConnection) -> QueryResult<()> {
    exec_pragma(conn, "PRAGMA foreign_keys = ON")
}

#[cfg(test)]
pub(crate) fn pragma_query_only_on(conn: &mut SqliteConnection) -> QueryResult<()> {
    exec_pragma(conn, "PRAGMA query_only = ON")
}

/// `PRAGMA journal_mode` — a read, one column, no DSL form.
#[cfg(test)]
#[derive(QueryId)]
pub(crate) struct JournalMode;

#[cfg(test)]
impl QueryFragment<Sqlite> for JournalMode {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("PRAGMA journal_mode");
        Ok(())
    }
}

#[cfg(test)]
impl Query for JournalMode {
    type SqlType = Text;
}

#[cfg(test)]
impl RunQueryDsl<SqliteConnection> for JournalMode {}

/// Expression conflict target `COALESCE(project, '')` — Diesel's `on_conflict` names columns only.
#[derive(QueryId)]
pub(crate) struct UpsertNote {
    pub project: Option<String>,
    pub kind: String,
    pub title: String,
    pub body: String,
}

impl QueryFragment<Sqlite> for UpsertNote {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("INSERT INTO notes (project, kind, title, body) VALUES (");
        out.push_bind_param::<Nullable<Text>, _>(&self.project)?;
        out.push_sql(", ");
        out.push_bind_param::<Text, _>(&self.kind)?;
        out.push_sql(", ");
        out.push_bind_param::<Text, _>(&self.title)?;
        out.push_sql(", ");
        out.push_bind_param::<Text, _>(&self.body)?;
        out.push_sql(
            ") ON CONFLICT (COALESCE(project, ''), kind, title) DO UPDATE SET \
                 body = excluded.body, \
                 ts = unixepoch(), \
                 retired = NULL, \
                 superseded_by = NULL \
             RETURNING id",
        );
        Ok(())
    }
}

impl Query for UpsertNote {
    type SqlType = Integer;
}

impl RunQueryDsl<SqliteConnection> for UpsertNote {}

/// FTS5 `MATCH` and `bm25()` — no form in Diesel 2.3's typed DSL.
#[derive(QueryId)]
pub(crate) struct SearchNotes {
    pub query: String,
    pub limit: i32,
}

impl QueryFragment<Sqlite> for SearchNotes {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "SELECT n.id, n.title, substr(n.body, 1, 120) \
             FROM notes_fts f JOIN notes n ON n.id = f.rowid \
             WHERE notes_fts MATCH ",
        );
        out.push_bind_param::<Text, _>(&self.query)?;
        out.push_sql(" AND n.retired IS NULL ORDER BY bm25(notes_fts) LIMIT ");
        out.push_bind_param::<Integer, _>(&self.limit)?;
        Ok(())
    }
}

impl Query for SearchNotes {
    type SqlType = (Integer, Text, Text);
}

impl RunQueryDsl<SqliteConnection> for SearchNotes {}

const OLD_CALLS: &str = "(SELECT id FROM calls WHERE ts < ?1)";

/// `?1` is reused inside one statement. T163.8 replaces these with `diesel::delete`.
fn bind_cutoff(conn: &mut SqliteConnection, sql: String, cutoff: i64) -> QueryResult<usize> {
    diesel::sql_query(sql)
        .bind::<BigInt, _>(cutoff)
        .execute(conn)
}

pub(crate) fn purge_related(conn: &mut SqliteConnection, cutoff: i64) -> QueryResult<()> {
    for sql in [
        format!("DELETE FROM logs WHERE ts < ?1 OR call_id IN {OLD_CALLS}"),
        format!("DELETE FROM tokens WHERE ts < ?1 OR call_id IN {OLD_CALLS}"),
        format!("DELETE FROM call_io WHERE call_id IN {OLD_CALLS}"),
        format!("UPDATE usage SET call_id = NULL WHERE call_id IN {OLD_CALLS}"),
        format!("UPDATE measurements SET call_id = NULL WHERE call_id IN {OLD_CALLS}"),
        format!("UPDATE calls SET parent_id = NULL WHERE parent_id IN {OLD_CALLS}"),
    ] {
        bind_cutoff(conn, sql, cutoff)?;
    }
    Ok(())
}

pub(crate) fn delete_old_calls(conn: &mut SqliteConnection, cutoff: i64) -> QueryResult<usize> {
    bind_cutoff(conn, "DELETE FROM calls WHERE ts < ?1".to_string(), cutoff)
}

pub(crate) fn purge_archive(conn: &mut SqliteConnection, id: &str) -> QueryResult<()> {
    for sql in [
        "DELETE FROM archive_decisions WHERE archive_id = ?1",
        "UPDATE read_cache SET archive_id = NULL WHERE archive_id = ?1",
        "DELETE FROM archive WHERE id = ?1",
    ] {
        diesel::sql_query(sql).bind::<Text, _>(id).execute(conn)?;
    }
    Ok(())
}
