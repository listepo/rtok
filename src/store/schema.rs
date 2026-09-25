//! Diesel `table!` macros for the six 0001 tables (plan T13.1).
//! `notes_fts` is a VIRTUAL TABLE — FTS5 `MATCH` / `bm25` have no Diesel DSL form (T163.3).

#![allow(unused)]

diesel::table! {
    events (id) {
        id -> Integer,
        ts -> BigInt,
        session -> Text,
        event -> Text,
        tool -> Nullable<Text>,
        plugin -> Nullable<Text>,
        ms -> Nullable<Double>,
    }
}

diesel::table! {
    measurements (id) {
        id -> Integer,
        ts -> BigInt,
        session -> Text,
        plugin -> Text,
        kind -> Text,
        before_bytes -> BigInt,
        after_bytes -> BigInt,
        est_before -> Integer,
        est_after -> Integer,
        ref_id -> Nullable<Text>,
        call_id -> Nullable<Integer>,
        once_key -> Nullable<Text>,
    }
}

diesel::table! {
    archive (id) {
        id -> Text,
        ts -> BigInt,
        session -> Text,
        tool -> Nullable<Text>,
        bytes -> BigInt,
        path -> Text,
        sha256 -> Text,
        // 0019 (T127): the context that wrote this row — a sub-agent's `agent_id`, or NULL
        // for the main window. Scopes `archive_in_session` so a pointer never names a body
        // a different context never saw.
        agent_id -> Nullable<Text>,
    }
}

// Composite PK (session, tool_use_id) since 0014: a repeated tool_use_id in a second
// session is a second decision, not an ignored insert.
diesel::table! {
    archive_decisions (session, tool_use_id) {
        session -> Text,
        tool_use_id -> Text,
        archive_id -> Text,
        pointer -> Text,
        expanded_ts -> Nullable<BigInt>,
        ts -> BigInt,
    }
}

diesel::table! {
    read_cache (session, path) {
        session -> Text,
        path -> Text,
        sha256 -> Text,
        ts -> BigInt,
        archive_id -> Nullable<Text>,
    }
}

diesel::table! {
    note_embeddings (note_id) {
        note_id -> Integer,
        model -> Text,
        dims -> Integer,
        text_hash -> Text,
        embedded_at -> BigInt,
        vector -> Binary,
    }
}

diesel::table! {
    notes (id) {
        id -> Integer,
        ts -> BigInt,
        project -> Nullable<Text>,
        kind -> Text,
        title -> Text,
        body -> Text,
        retired -> Nullable<BigInt>,
        superseded_by -> Nullable<Integer>,
        pinned -> Integer,
        // 0017 (T69.2); written with `sql_query`, listed so T104's drift guard holds.
        uses -> Integer,
        last_used -> Nullable<BigInt>,
    }
}

diesel::table! {
    usage (id) {
        id -> Integer,
        ts -> BigInt,
        session -> Text,
        model -> Nullable<Text>,
        input -> BigInt,
        cache_create -> BigInt,
        cache_read -> BigInt,
        output -> BigInt,
        call_id -> Nullable<Integer>,
        api -> Text,
    }
}

diesel::table! {
    otel_export (stream) {
        stream -> Text,
        mark -> BigInt,
    }
}

diesel::table! {
    hosts (id) {
        id -> Integer,
        slug -> Text,
        kind -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    providers (id) {
        id -> Integer,
        slug -> Text,
        name -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    models (id) {
        id -> Integer,
        provider_id -> Integer,
        slug -> Text,
        created_at -> BigInt,
    }
}

diesel::table! {
    sessions (id) {
        id -> Text,
        host_id -> Nullable<Integer>,
        project -> Nullable<Text>,
        cwd -> Nullable<Text>,
        source -> Nullable<Text>,
        started_at -> BigInt,
        ended_at -> Nullable<BigInt>,
    }
}

diesel::table! {
    calls (id) {
        id -> Integer,
        ts -> BigInt,
        session_id -> Text,
        host_id -> Nullable<Integer>,
        provider_id -> Nullable<Integer>,
        model_id -> Nullable<Integer>,
        plugin -> Nullable<Text>,
        surface -> Text,
        kind -> Text,
        parent_id -> Nullable<Integer>,
        name -> Nullable<Text>,
        ms -> Nullable<Double>,
        ok -> Integer,
        error -> Nullable<Text>,
    }
}

diesel::table! {
    call_io (call_id) {
        call_id -> Integer,
        request_bytes -> BigInt,
        response_bytes -> BigInt,
        request_sha256 -> Nullable<Text>,
        response_sha256 -> Nullable<Text>,
        request_json -> Nullable<Text>,
        response_json -> Nullable<Text>,
        request_archive -> Nullable<Text>,
        response_archive -> Nullable<Text>,
        // T211: exact bytes, written only when the inline body is not valid UTF-8.
        request_raw -> Nullable<Binary>,
        response_raw -> Nullable<Binary>,
    }
}

diesel::table! {
    tokens (id) {
        id -> Integer,
        ts -> BigInt,
        call_id -> Integer,
        plugin -> Nullable<Text>,
        phase -> Text,
        source -> Text,
        #[sql_name = "tokens"]
        n_tokens -> BigInt,
        bytes -> Nullable<BigInt>,
        input -> Nullable<BigInt>,
        output -> Nullable<BigInt>,
        cache_create -> Nullable<BigInt>,
        cache_read -> Nullable<BigInt>,
    }
}

diesel::table! {
    logs (id) {
        id -> Integer,
        ts -> BigInt,
        level -> Text,
        source -> Text,
        name -> Text,
        session -> Nullable<Text>,
        call_id -> Nullable<Integer>,
        plugin -> Nullable<Text>,
        message -> Text,
        fields -> Nullable<Text>,
    }
}

// 0018 (T69.6): hand-edit guard digest, keyed by name.
diesel::table! {
    kv (key) {
        key -> Text,
        value -> Text,
    }
}

diesel::table! {
    symbols (id) {
        id -> Integer,
        path -> Text,
        name -> Text,
        kind -> Text,
        line -> Integer,
        is_def -> Integer,
        file_sha -> Text,
        root -> Text,
        mtime -> BigInt,
        size -> BigInt,
        end_line -> Integer,
        scope -> Text,
    }
}

// 0011 + 0016 (T68.3): one fingerprint and last index time per root.
diesel::table! {
    extractor (root) {
        root -> Text,
        fingerprint -> Text,
        indexed_at -> Nullable<BigInt>,
    }
}

// 0016 (T68.3): hook-staled files, listed until the next index replaces their rows.
diesel::table! {
    symbol_stale (root, path) {
        root -> Text,
        path -> Text,
    }
}

diesel::joinable!(archive_decisions -> archive (archive_id));
diesel::joinable!(models -> providers (provider_id));
diesel::joinable!(sessions -> hosts (host_id));
diesel::joinable!(calls -> hosts (host_id));
diesel::joinable!(calls -> providers (provider_id));
diesel::joinable!(calls -> models (model_id));
diesel::joinable!(calls -> sessions (session_id));
diesel::joinable!(call_io -> calls (call_id));
diesel::joinable!(tokens -> calls (call_id));
diesel::joinable!(logs -> calls (call_id));
diesel::joinable!(measurements -> calls (call_id));
diesel::joinable!(usage -> calls (call_id));
diesel::joinable!(note_embeddings -> notes (note_id));

diesel::allow_tables_to_appear_in_same_query!(
    events,
    measurements,
    archive,
    archive_decisions,
    read_cache,
    notes,
    note_embeddings,
    usage,
    hosts,
    providers,
    models,
    sessions,
    calls,
    call_io,
    tokens,
    logs,
    symbols,
    extractor,
    symbol_stale,
);
