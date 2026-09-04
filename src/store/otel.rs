//! T16.2 (D19): what the OpenTelemetry exporter reads — per-stream watermarks and the rows
//! past them. A second `impl Store`; `src/otel/` never sees SQL.

use anyhow::Result;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::{BigInt, Integer, Nullable, Text};
use diesel::sqlite::SqliteConnection;

use super::Store;
use super::models::{Call, CallIo, LogRow, Session, TokenRow};
use super::schema::{
    call_io, calls, hosts, logs, measurements, models, otel_export, providers, sessions, tokens,
    usage,
};

/// Everything a span needs beside its `calls` row.
#[derive(Debug, Default)]
pub struct CallDetail {
    pub io: Option<CallIo>,
    pub usage: Option<CallUsage>,
    pub tokens: Vec<TokenRow>,
    pub measurements: Vec<CallMeasurement>,
    pub host: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
}

/// The `usage` row of one proxied request.
#[derive(Debug, Queryable)]
pub struct CallUsage {
    pub api: String,
    pub input: i64,
    pub cache_create: i64,
    pub cache_read: i64,
    pub output: i64,
}

/// One `measurements` row attributed to a call.
#[derive(Debug, Queryable)]
pub struct CallMeasurement {
    pub plugin: String,
    pub kind: String,
    pub before_bytes: i64,
    pub after_bytes: i64,
    pub est_before: i32,
    pub est_after: i32,
    pub ref_id: Option<String>,
}

/// `usage` summed per model and wire (T16.7).
#[derive(Debug, QueryableByName)]
pub struct TokenTotal {
    #[diesel(sql_type = Nullable<Text>)]
    pub model: Option<String>,
    #[diesel(sql_type = Text)]
    pub api: String,
    #[diesel(sql_type = BigInt)]
    pub input: i64,
    #[diesel(sql_type = BigInt)]
    pub cache_create: i64,
    #[diesel(sql_type = BigInt)]
    pub cache_read: i64,
    #[diesel(sql_type = BigInt)]
    pub output: i64,
}

/// `measurements` saved tokens per plugin and kind (T16.7).
#[derive(Debug, QueryableByName)]
pub struct SavedTotal {
    #[diesel(sql_type = Text)]
    pub plugin: String,
    #[diesel(sql_type = Text)]
    pub kind: String,
    #[diesel(sql_type = BigInt)]
    pub saved: i64,
}

/// `calls` counted per surface, kind and outcome (T16.7).
#[derive(Debug, QueryableByName)]
pub struct CallTotal {
    #[diesel(sql_type = Text)]
    pub surface: String,
    #[diesel(sql_type = Text)]
    pub kind: String,
    #[diesel(sql_type = Integer)]
    pub ok: i32,
    #[diesel(sql_type = BigInt)]
    pub n: i64,
}

#[derive(QueryableByName)]
struct Ts {
    #[diesel(sql_type = BigInt)]
    ts: i64,
}

fn clamp(id: i64) -> i32 {
    i32::try_from(id).unwrap_or(i32::MAX)
}

fn host_slug(conn: &mut SqliteConnection, id: i32) -> Result<Option<String>> {
    Ok(hosts::table
        .find(id)
        .select(hosts::slug)
        .first::<String>(conn)
        .optional()?)
}

impl Store {
    pub fn host_slug(&self, id: i32) -> Result<Option<String>> {
        let mut conn = self.lock()?;
        host_slug(&mut conn, id)
    }

    /// Rows past the `calls` and `logs` marks — what the next flushes will post.
    pub fn otel_pending(&self) -> Result<(i64, i64)> {
        let (c, l) = (self.otel_mark("calls")?, self.otel_mark("logs")?);
        let mut conn = self.lock()?;
        let calls = calls::table
            .filter(calls::id.gt(clamp(c)))
            .count()
            .get_result::<i64>(&mut *conn)?;
        let logs = logs::table
            .filter(logs::id.gt(clamp(l)))
            .count()
            .get_result::<i64>(&mut *conn)?;
        Ok((calls, logs))
    }

    /// Newest `logs` row from `source`, e.g. the exporter's last failure.
    pub fn last_log(&self, source: &str) -> Result<Option<LogRow>> {
        let mut conn = self.lock()?;
        Ok(logs::table
            .filter(logs::source.eq(source))
            .order(logs::id.desc())
            .select(LogRow::as_select())
            .first(&mut *conn)
            .optional()?)
    }

    pub fn otel_token_totals(&self) -> Result<Vec<TokenTotal>> {
        let mut conn = self.lock()?;
        Ok(sql_query(
            "SELECT model, api,
                    COALESCE(SUM(input),0) AS input,
                    COALESCE(SUM(cache_create),0) AS cache_create,
                    COALESCE(SUM(cache_read),0) AS cache_read,
                    COALESCE(SUM(output),0) AS output
             FROM usage GROUP BY model, api ORDER BY model, api",
        )
        .load(&mut *conn)?)
    }

    pub fn otel_saved_totals(&self) -> Result<Vec<SavedTotal>> {
        let mut conn = self.lock()?;
        Ok(sql_query(
            "SELECT plugin, kind, COALESCE(SUM(est_before - est_after),0) AS saved
             FROM measurements GROUP BY plugin, kind ORDER BY plugin, kind",
        )
        .load(&mut *conn)?)
    }

    pub fn otel_call_totals(&self) -> Result<Vec<CallTotal>> {
        let mut conn = self.lock()?;
        Ok(sql_query(
            "SELECT surface, kind, ok, COUNT(*) AS n
             FROM calls GROUP BY surface, kind, ok ORDER BY surface, kind, ok",
        )
        .load(&mut *conn)?)
    }

    /// Earliest `calls.ts`, the sums' start time; 0 on an empty ledger.
    pub fn otel_first_ts(&self) -> Result<i64> {
        let mut conn = self.lock()?;
        let rows: Vec<Ts> =
            sql_query("SELECT COALESCE(MIN(ts),0) AS ts FROM calls").load(&mut *conn)?;
        Ok(rows.first().map(|r| r.ts).unwrap_or(0))
    }

    /// Last value posted for `stream`; 0 before the first successful flush.
    pub fn otel_mark(&self, stream: &str) -> Result<i64> {
        let mut conn = self.lock()?;
        Ok(otel_export::table
            .find(stream)
            .select(otel_export::mark)
            .first::<i64>(&mut *conn)
            .optional()?
            .unwrap_or(0))
    }

    pub fn otel_advance(&self, stream: &str, mark: i64) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::insert_into(otel_export::table)
            .values((otel_export::stream.eq(stream), otel_export::mark.eq(mark)))
            .on_conflict(otel_export::stream)
            .do_update()
            .set(otel_export::mark.eq(mark))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// `calls` rows with `id > after`, ascending, at most `limit`.
    pub fn calls_after(&self, after: i64, limit: i64) -> Result<Vec<Call>> {
        let mut conn = self.lock()?;
        Ok(calls::table
            .filter(calls::id.gt(clamp(after)))
            .order(calls::id.asc())
            .limit(limit)
            .select(Call::as_select())
            .load(&mut *conn)?)
    }

    pub fn logs_after(&self, after: i64, limit: i64) -> Result<Vec<LogRow>> {
        let mut conn = self.lock()?;
        Ok(logs::table
            .filter(logs::id.gt(clamp(after)))
            .order(logs::id.asc())
            .limit(limit)
            .select(LogRow::as_select())
            .load(&mut *conn)?)
    }

    /// Sessions with `ended_at >= ts`: a tie is re-sent (same span id, same bytes), never lost.
    pub fn sessions_ended_after(&self, ts: i64) -> Result<Vec<Session>> {
        let mut conn = self.lock()?;
        Ok(sessions::table
            .filter(sessions::ended_at.ge(ts))
            .order(sessions::ended_at.asc())
            .select(Session::as_select())
            .load(&mut *conn)?)
    }

    pub fn end_session(&self, id: &str, ended_at: i64) -> Result<()> {
        let mut conn = self.lock()?;
        diesel::update(sessions::table.find(id))
            .set(sessions::ended_at.eq(ended_at))
            .execute(&mut *conn)?;
        Ok(())
    }

    /// The one read a span needs: io, usage, plugin token rows, measurements and the three slugs.
    pub fn call_detail(&self, call: &Call) -> Result<CallDetail> {
        let mut conn = self.lock()?;
        let io = call_io::table
            .find(call.id)
            .select(CallIo::as_select())
            .first(&mut *conn)
            .optional()?;
        let usage = usage::table
            .filter(usage::call_id.eq(call.id))
            .order(usage::id.desc())
            .select((
                usage::api,
                usage::input,
                usage::cache_create,
                usage::cache_read,
                usage::output,
            ))
            .first::<CallUsage>(&mut *conn)
            .optional()?;
        let tokens = tokens::table
            .filter(tokens::call_id.eq(call.id))
            .order(tokens::id.asc())
            .select(TokenRow::as_select())
            .load(&mut *conn)?;
        let measurements = measurements::table
            .filter(measurements::call_id.eq(call.id))
            .order(measurements::id.asc())
            .select((
                measurements::plugin,
                measurements::kind,
                measurements::before_bytes,
                measurements::after_bytes,
                measurements::est_before,
                measurements::est_after,
                measurements::ref_id,
            ))
            .load::<CallMeasurement>(&mut *conn)?;
        let host = match call.host_id {
            Some(id) => host_slug(&mut conn, id)?,
            None => None,
        };
        let provider = match call.provider_id {
            Some(id) => providers::table
                .find(id)
                .select(providers::slug)
                .first::<String>(&mut *conn)
                .optional()?,
            None => None,
        };
        let model = match call.model_id {
            Some(id) => models::table
                .find(id)
                .select(models::slug)
                .first::<String>(&mut *conn)
                .optional()?,
            None => None,
        };
        Ok(CallDetail {
            io,
            usage,
            tokens,
            measurements,
            host,
            provider,
            model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::Measurement;

    #[test]
    fn marks_start_at_zero_and_upsert() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(s.otel_mark("calls").unwrap(), 0);
        s.otel_advance("calls", 5).unwrap();
        s.otel_advance("calls", 7).unwrap();
        assert_eq!(s.otel_mark("calls").unwrap(), 7);
        assert_eq!(s.otel_mark("logs").unwrap(), 0);
    }

    #[test]
    fn rows_after_a_mark_ascend_and_honour_the_limit() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_session("s1", None, None, None, None).unwrap();
        for _ in 0..3 {
            s.insert_call(
                "s1",
                "hook",
                "hook",
                None,
                None,
                None,
                None,
                Some("PostToolUse"),
            )
            .unwrap();
        }
        let ids: Vec<i32> = s.calls_after(1, 10).unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids, vec![2, 3]);
        assert_eq!(s.calls_after(0, 1).unwrap().len(), 1);
        assert!(s.calls_after(3, 10).unwrap().is_empty());
        s.insert_log("info", "otel", "flush", "m", Some("s1"), Some(1), None)
            .unwrap();
        assert_eq!(s.logs_after(0, 10).unwrap().len(), 1);
        assert!(s.logs_after(1, 10).unwrap().is_empty());
    }

    #[test]
    fn ended_sessions_include_the_tie() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_session("s1", None, Some("p"), Some("/w"), Some("startup"))
            .unwrap();
        assert!(s.sessions_ended_after(0).unwrap().is_empty());
        s.end_session("s1", 1_700_000_000).unwrap();
        let rows = s.sessions_ended_after(1_700_000_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ended_at, Some(1_700_000_000));
        assert!(s.sessions_ended_after(1_700_000_001).unwrap().is_empty());
    }

    #[test]
    fn call_detail_joins_io_usage_tokens_measurements_and_slugs() {
        let s = Store::open_in_memory().unwrap();
        s.upsert_session("s1", None, None, None, None).unwrap();
        let (pid, mid) = s.upsert_model("anthropic", "claude-x").unwrap();
        let id = s
            .insert_call(
                "s1",
                "proxy",
                "api_request",
                None,
                Some(pid),
                Some(mid),
                None,
                Some("/v1/messages"),
            )
            .unwrap();
        s.insert_call_io(id, Some(br#"{"a":1}"#), Some(br#"{"b":2}"#), 1024, None)
            .unwrap();
        s.insert_usage("s1", Some("claude-x"), "anthropic", 100, 10, 20, 30, id)
            .unwrap();
        s.insert_tokens(id, Some("proxy"), "before", "est", 3)
            .unwrap();
        s.insert_measurement(
            "s1",
            &Measurement {
                plugin: "proxy",
                kind: "raw",
                before_bytes: 10,
                after_bytes: 5,
                est_before: 3,
                est_after: 1,
                ref_id: None,
                call_id: Some(id),
            },
        )
        .unwrap();
        let call = s.calls_after(0, 1).unwrap().remove(0);
        let d = s.call_detail(&call).unwrap();
        assert_eq!(d.io.unwrap().request_json.as_deref(), Some(r#"{"a":1}"#));
        let u = d.usage.unwrap();
        assert_eq!(
            (u.input, u.cache_create, u.cache_read, u.output),
            (100, 10, 20, 30)
        );
        assert_eq!(d.tokens[0].n_tokens, 3);
        assert_eq!(
            d.measurements[0].est_before - d.measurements[0].est_after,
            2
        );
        assert_eq!(d.provider.as_deref(), Some("anthropic"));
        assert_eq!(d.model.as_deref(), Some("claude-x"));
        assert_eq!(d.host, None);
    }
}
