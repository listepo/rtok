//! Diesel models for the action store (plan T13.2, decision D13).

use diesel::prelude::*;

use super::schema::{call_io, calls, hosts, logs, models, providers, sessions, tokens};

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = hosts)]
pub struct Host {
    pub id: i32,
    pub slug: String,
    pub kind: String,
    pub created_at: i64,
}

#[derive(Debug, Queryable, Selectable, Identifiable)]
#[diesel(table_name = providers)]
pub struct Provider {
    pub id: i32,
    pub slug: String,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(Provider))]
#[diesel(table_name = models)]
pub struct Model {
    pub id: i32,
    pub provider_id: i32,
    pub slug: String,
    pub created_at: i64,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(Host))]
#[diesel(table_name = sessions)]
pub struct Session {
    pub id: String,
    pub host_id: Option<i32>,
    pub project: Option<String>,
    pub cwd: Option<String>,
    pub source: Option<String>,
    pub started_at: i64,
    pub ended_at: Option<i64>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(Host))]
#[diesel(belongs_to(Provider))]
#[diesel(belongs_to(Model))]
#[diesel(belongs_to(Session))]
#[diesel(table_name = calls)]
pub struct Call {
    pub id: i32,
    pub ts: i64,
    pub session_id: String,
    pub host_id: Option<i32>,
    pub provider_id: Option<i32>,
    pub model_id: Option<i32>,
    pub plugin: Option<String>,
    pub surface: String,
    pub kind: String,
    pub parent_id: Option<i32>,
    pub name: Option<String>,
    pub ms: Option<f64>,
    pub ok: i32,
    pub error: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(primary_key(call_id))]
#[diesel(belongs_to(Call, foreign_key = call_id))]
#[diesel(table_name = call_io)]
pub struct CallIo {
    pub call_id: i32,
    pub request_bytes: i64,
    pub response_bytes: i64,
    pub request_sha256: Option<String>,
    pub response_sha256: Option<String>,
    pub request_json: Option<String>,
    pub response_json: Option<String>,
    pub request_archive: Option<String>,
    pub response_archive: Option<String>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(Call))]
#[diesel(table_name = tokens)]
pub struct TokenRow {
    pub id: i32,
    pub ts: i64,
    pub call_id: i32,
    pub plugin: Option<String>,
    pub phase: String,
    pub source: String,
    pub n_tokens: i64,
    pub bytes: Option<i64>,
    pub input: Option<i64>,
    pub output: Option<i64>,
    pub cache_create: Option<i64>,
    pub cache_read: Option<i64>,
}

#[derive(Debug, Queryable, Selectable, Identifiable, Associations)]
#[diesel(belongs_to(Call))]
#[diesel(table_name = logs)]
pub struct LogRow {
    pub id: i32,
    pub ts: i64,
    pub level: String,
    pub source: String,
    pub name: String,
    pub session: Option<String>,
    pub call_id: Option<i32>,
    pub plugin: Option<String>,
    pub message: String,
    pub fields: Option<String>,
}
