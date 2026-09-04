//! Diesel `table!` macros for the six 0001 tables (plan T13.1).
//! `notes_fts` is a VIRTUAL TABLE — queried with `sql_query`, not modelled here.

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
    notes (id) {
        id -> Integer,
        ts -> BigInt,
        project -> Nullable<Text>,
        kind -> Text,
        title -> Text,
        body -> Text,
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

diesel::allow_tables_to_appear_in_same_query!(
    events,
    measurements,
    archive,
    read_cache,
    notes,
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
);
