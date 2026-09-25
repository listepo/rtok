//! SQL the typed DSL cannot express. Each item names the construct Diesel 2.3 lacks.

use diesel::prelude::*;
use diesel::query_builder::{AstPass, Query, QueryFragment, QueryId};
use diesel::sql_types::{BigInt, Binary, Double, Integer, Nullable, Text};
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

/// `COUNT() OVER` and `ROW_NUMBER() OVER` — no window functions in Diesel 2.3's typed DSL.
#[derive(QueryId)]
pub(crate) struct UsageCtt;

impl QueryFragment<Sqlite> for UsageCtt {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "SELECT COALESCE(SUM(ctx * (total - rn)), 0) FROM (
                SELECT input + cache_create + cache_read AS ctx,
                       COUNT(*) OVER (PARTITION BY session) AS total,
                       ROW_NUMBER() OVER (PARTITION BY session ORDER BY ts, id) AS rn
                FROM usage)",
        );
        Ok(())
    }
}

impl Query for UsageCtt {
    type SqlType = BigInt;
}

impl RunQueryDsl<SqliteConnection> for UsageCtt {}

/// `JOIN` of a grouped `MIN` subquery — Diesel 2.3 cannot type this shape.
#[derive(QueryId)]
pub(crate) struct UsageCttTail {
    pub turns: i64,
}

impl QueryFragment<Sqlite> for UsageCttTail {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "SELECT u.input + u.cache_create + u.cache_read
             FROM usage u
             JOIN (SELECT session, MIN(ts) AS first_ts, MIN(id) AS first_id
                   FROM usage GROUP BY session) f ON f.session = u.session
             ORDER BY f.first_ts DESC, f.first_id DESC, u.ts DESC, u.id DESC
             LIMIT ",
        );
        out.push_bind_param::<BigInt, _>(&self.turns)?;
        Ok(())
    }
}

impl Query for UsageCttTail {
    type SqlType = BigInt;
}

impl RunQueryDsl<SqliteConnection> for UsageCttTail {}

/// Four CTEs, `UNION ALL`, and per-group `MAX(id)` subqueries — no form in Diesel 2.3.
#[derive(QueryId)]
pub(crate) struct RecentSessionTotals {
    pub since: i64,
    pub limit: i64,
}

impl QueryFragment<Sqlite> for RecentSessionTotals {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "WITH tot AS (
                 SELECT session AS sid, SUM(input) AS input, SUM(cache_create) AS cache_create,
                        SUM(cache_read) AS cache_read, SUM(output) AS output
                 FROM usage GROUP BY session
             ),
             last_u AS (
                 SELECT session AS sid, api, model FROM usage
                 WHERE id IN (SELECT MAX(id) FROM usage GROUP BY session)
             ),
             act AS (
                 SELECT sid, MAX(ts) AS ts FROM (
                     SELECT session AS sid, ts FROM usage
                     UNION ALL
                     SELECT session_id AS sid, ts FROM calls
                 ) GROUP BY sid
             ),
             prov AS (
                 SELECT c.session_id AS sid, p.slug AS provider
                 FROM calls c JOIN providers p ON p.id = c.provider_id
                 WHERE c.id IN (SELECT MAX(id) FROM calls
                                WHERE provider_id IS NOT NULL GROUP BY session_id)
             )
             SELECT s.id, h.slug, s.project,
                    prov.provider, last_u.api, last_u.model,
                    COALESCE(tot.input, 0),
                    COALESCE(tot.cache_create, 0),
                    COALESCE(tot.cache_read, 0),
                    COALESCE(tot.output, 0),
                    s.started_at,
                    COALESCE(act.ts, s.started_at),
                    s.ended_at
             FROM sessions s
             LEFT JOIN hosts h ON h.id = s.host_id
             LEFT JOIN tot ON tot.sid = s.id
             LEFT JOIN last_u ON last_u.sid = s.id
             LEFT JOIN act ON act.sid = s.id
             LEFT JOIN prov ON prov.sid = s.id
             WHERE s.started_at >= ",
        );
        out.push_bind_param::<BigInt, _>(&self.since)?;
        out.push_sql(" ORDER BY s.started_at DESC, s.id LIMIT ");
        out.push_bind_param::<BigInt, _>(&self.limit)?;
        Ok(())
    }
}

impl Query for RecentSessionTotals {
    type SqlType = (
        Text,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        BigInt,
        BigInt,
        BigInt,
        BigInt,
        BigInt,
        BigInt,
        Nullable<BigInt>,
    );
}

impl RunQueryDsl<SqliteConnection> for RecentSessionTotals {}

/// Correlated `MAX(id)` subquery in a `LEFT JOIN` — Diesel 2.3 has no typed form.
#[derive(QueryId)]
pub(crate) struct RecentCalls {
    pub limit: i64,
}

impl QueryFragment<Sqlite> for RecentCalls {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "SELECT c.id, c.ts, c.session_id, c.surface,
                    c.kind, c.plugin, c.name,
                    c.parent_id, c.ms, c.ok, c.error,
                    h.slug, p.slug, m.slug,
                    u.api, u.input, u.cache_create,
                    u.cache_read, u.output
             FROM calls c
             LEFT JOIN hosts h ON h.id = c.host_id
             LEFT JOIN providers p ON p.id = c.provider_id
             LEFT JOIN models m ON m.id = c.model_id
             LEFT JOIN usage u ON u.call_id = c.id
                  AND u.id = (SELECT MAX(id) FROM usage WHERE call_id = c.id)
             ORDER BY c.id DESC
             LIMIT ",
        );
        out.push_bind_param::<BigInt, _>(&self.limit)?;
        Ok(())
    }
}

impl Query for RecentCalls {
    type SqlType = (
        Integer,
        BigInt,
        Text,
        Text,
        Text,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Integer>,
        Nullable<Double>,
        Integer,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<Text>,
        Nullable<BigInt>,
        Nullable<BigInt>,
        Nullable<BigInt>,
        Nullable<BigInt>,
    );
}

impl RunQueryDsl<SqliteConnection> for RecentCalls {}

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
