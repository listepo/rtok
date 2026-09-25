//! Embedded `migrations/<version>/up.sql` plus the one-time bridge from `schema_migrations`.
//!
//! Diesel 2.3 records applied versions in `__diesel_schema_migrations`. Databases opened
//! before T163.4 recorded `NNNN.sql` names in `schema_migrations` instead. Those names map
//! onto the directory version (`0001.sql` → `0001_schema_v1`) and are marked applied, so
//! `up.sql` does not run again.

use anyhow::Result;
use diesel::migration::{Migration, MigrationSource};
use diesel::prelude::*;
use diesel::query_builder::{AstPass, Query, QueryFragment, QueryId};
use diesel::sql_types::Text;
use diesel::sqlite::{Sqlite, SqliteConnection};
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn bridge_legacy(conn: &mut SqliteConnection) -> Result<()> {
    // Creates `__diesel_schema_migrations` when missing. The CREATE lives in diesel.
    conn.applied_migrations().map_err(|e| anyhow::anyhow!(e))?;
    let names: Vec<String> = match LegacyNames.load(conn) {
        Ok(names) => names,
        Err(diesel::result::Error::DatabaseError(_, info))
            if info.message().contains("no such table") =>
        {
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };
    if names.is_empty() {
        return Ok(());
    }
    let embedded = embedded_migrations()?;
    for name in &names {
        let prefix = name.split('.').next().unwrap_or(name.as_str());
        let version = embedded.iter().find_map(|migration| {
            let version = migration.name().version().to_string();
            let matches = version.split('_').next().unwrap_or(version.as_str()) == prefix;
            matches.then_some(version)
        });
        let Some(version) = version else {
            anyhow::bail!("schema_migrations name {name} has no Diesel version");
        };
        MarkVersion { version }.execute(conn)?;
    }
    Ok(())
}

pub fn has_pending(conn: &mut SqliteConnection) -> Result<bool> {
    conn.has_pending_migration(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!(e))
}

#[cfg(test)]
pub fn versions() -> Result<Vec<String>> {
    let mut versions = embedded_migrations()?
        .into_iter()
        .map(|migration| migration.name().version().to_string())
        .collect::<Vec<_>>();
    versions.sort();
    Ok(versions)
}

/// Drop Diesel's version rows and write the pre-T163.4 `NNNN.sql` names, so the next
/// `Store::open` takes the bridge instead of running `up.sql` again.
#[cfg(test)]
pub fn revert_to_legacy_bookkeeping(conn: &mut SqliteConnection) -> Result<()> {
    DeleteDieselVersions.execute(conn)?;
    CreateLegacy.execute(conn)?;
    for version in versions()? {
        let prefix = version.split('_').next().unwrap_or(version.as_str());
        InsertLegacy {
            name: format!("{prefix}.sql"),
        }
        .execute(conn)?;
    }
    Ok(())
}

pub fn run_pending(conn: &mut SqliteConnection) -> Result<usize> {
    let ran = conn
        .run_pending_migrations(MIGRATIONS)
        .map_err(|e| anyhow::anyhow!(e))?;
    Ok(ran.len())
}

/// Apply every embedded migration whose numeric prefix is less than `tag` (`"0015"`).
#[cfg(test)]
pub fn run_before(conn: &mut SqliteConnection, tag: &str) -> Result<()> {
    conn.applied_migrations().map_err(|e| anyhow::anyhow!(e))?;
    for migration in embedded_migrations()? {
        let version = migration.name().version().to_string();
        let prefix = version.split('_').next().unwrap_or(version.as_str());
        if prefix >= tag {
            break;
        }
        conn.run_migration(migration.as_ref())
            .map_err(|e| anyhow::anyhow!(e))?;
    }
    Ok(())
}

fn embedded_migrations() -> Result<Vec<Box<dyn Migration<Sqlite>>>> {
    MigrationSource::<Sqlite>::migrations(&MIGRATIONS).map_err(|e| anyhow::anyhow!(e))
}

#[derive(QueryId)]
struct LegacyNames;

impl QueryFragment<Sqlite> for LegacyNames {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("SELECT name FROM schema_migrations ORDER BY name");
        Ok(())
    }
}

impl Query for LegacyNames {
    type SqlType = Text;
}

impl RunQueryDsl<SqliteConnection> for LegacyNames {}

#[derive(QueryId)]
struct MarkVersion {
    version: String,
}

impl QueryFragment<Sqlite> for MarkVersion {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("INSERT OR IGNORE INTO __diesel_schema_migrations (version) VALUES (");
        out.push_bind_param::<Text, _>(&self.version)?;
        out.push_sql(")");
        Ok(())
    }
}

impl RunQueryDsl<SqliteConnection> for MarkVersion {}

#[cfg(test)]
#[derive(QueryId)]
struct DeleteDieselVersions;

#[cfg(test)]
impl QueryFragment<Sqlite> for DeleteDieselVersions {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("DELETE FROM __diesel_schema_migrations");
        Ok(())
    }
}

#[cfg(test)]
impl RunQueryDsl<SqliteConnection> for DeleteDieselVersions {}

#[cfg(test)]
#[derive(QueryId)]
struct CreateLegacy;

#[cfg(test)]
impl QueryFragment<Sqlite> for CreateLegacy {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                name TEXT PRIMARY KEY,
                applied_at INTEGER NOT NULL DEFAULT (unixepoch()))",
        );
        Ok(())
    }
}

#[cfg(test)]
impl RunQueryDsl<SqliteConnection> for CreateLegacy {}

#[cfg(test)]
#[derive(QueryId)]
struct InsertLegacy {
    name: String,
}

#[cfg(test)]
impl QueryFragment<Sqlite> for InsertLegacy {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("INSERT INTO schema_migrations (name) VALUES (");
        out.push_bind_param::<Text, _>(&self.name)?;
        out.push_sql(")");
        Ok(())
    }
}

#[cfg(test)]
impl RunQueryDsl<SqliteConnection> for InsertLegacy {}
