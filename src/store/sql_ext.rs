//! T163: one Diesel extension for SQL the typed query builder cannot express — recursive
//! CTEs today, FTS5 `MATCH`/`bm25` if a later slice needs them. This is a hand-written
//! [`diesel::query_builder::QueryFragment`], bound one placeholder at a time through the
//! public `AstPass::push_bind_param`, never the banned `sql_query`/`sql::<>`/`batch_execute`
//! entry points (AGENTS.md, D13).
//!
//! Deserialize results the same way `sql_query` callers already do: a
//! `#[derive(QueryableByName)]` row struct and `.load::<Row>(conn)`.

use diesel::QueryResult;
use diesel::query_builder::{AstPass, Query, QueryFragment, QueryId};
use diesel::serialize::ToSql;
use diesel::sql_types::{HasSqlType, Untyped};
use diesel::sqlite::Sqlite;
use std::marker::PhantomData;

/// One already-typed, owned bind value. Boxed so [`RawQuery`] can mix `Text`/`Integer`/...
/// binds in a single query, in the order their `?` placeholders appear.
struct BoundValue<ST, U> {
    value: U,
    ty: PhantomData<ST>,
}

impl<ST, U> QueryFragment<Sqlite> for BoundValue<ST, U>
where
    Sqlite: HasSqlType<ST>,
    U: ToSql<ST, Sqlite>,
{
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        pass.push_bind_param::<ST, U>(&self.value)
    }
}

/// A raw-SQL query for shapes the DSL cannot express (recursive CTEs, `MATCH`/`bm25`).
/// `sql` is split on `?`; each [`RawQuery::bind`] call fills the next placeholder, left to
/// right — the same order as writing `sql_query(...).bind(...).bind(...)`, just through a
/// query fragment this module owns instead of Diesel's raw-SQL entry point.
#[must_use = "Queries are only executed when calling `load`, `get_result` or similar."]
pub struct RawQuery {
    parts: Vec<String>,
    binds: Vec<Box<dyn QueryFragment<Sqlite>>>,
}

impl RawQuery {
    pub fn new(sql: &str) -> Self {
        RawQuery {
            parts: sql.split('?').map(str::to_string).collect(),
            binds: Vec::new(),
        }
    }

    pub fn bind<ST, U>(mut self, value: U) -> Self
    where
        ST: 'static,
        Sqlite: HasSqlType<ST>,
        U: ToSql<ST, Sqlite> + 'static,
    {
        self.binds.push(Box::new(BoundValue::<ST, U> {
            value,
            ty: PhantomData,
        }));
        self
    }
}

impl QueryId for RawQuery {
    type QueryId = ();
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl Query for RawQuery {
    type SqlType = Untyped;
}

impl QueryFragment<Sqlite> for RawQuery {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.unsafe_to_cache_prepared();
        debug_assert_eq!(
            self.parts.len(),
            self.binds.len() + 1,
            "RawQuery: `?` placeholder count must match the number of `.bind()` calls"
        );
        for (i, part) in self.parts.iter().enumerate() {
            out.push_sql(part);
            if let Some(bind) = self.binds.get(i) {
                bind.walk_ast(out.reborrow())?;
            }
        }
        Ok(())
    }
}

impl<Conn> diesel::query_dsl::RunQueryDsl<Conn> for RawQuery {}
