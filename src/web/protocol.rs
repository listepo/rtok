// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! The `/ws` contract (T310.2): the frames the server sends and the messages a client may send.
//! Rust is the one source of truth — `web/src/api/ws.schema.json` is generated from these types
//! (and [`model::Snapshot`]), and the SPA's `snapshot.gen.ts` from that schema.

use schemars::JsonSchema;
use schemars::generate::SchemaSettings;
use serde::{Deserialize, Serialize};

use super::model::Snapshot;

/// Committed schema, relative to the repository root.
pub const SCHEMA_PATH: &str = "web/src/api/ws.schema.json";

/// A frame the server pushes besides the [`Snapshot`] itself.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerFrame {
    /// A one-line notice for the operator (refused key, failed `set`, unknown archive id).
    Message { text: String },
    /// The archived payload a client asked for with [`ClientMessage::Expand`].
    Expand { id: String, text: String },
}

impl ServerFrame {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// A message a client sends over `/ws`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum ClientMessage {
    /// Ask for the archived payload behind an archive id.
    Expand { expand: String },
    /// Flip an allowlisted boolean key (`plugins.<id>.enabled`).
    Set { set: SetRequest },
    /// Change the project registry (T329.12, T329.20): select a project, or link two of them.
    Project { project: ProjectRequest },
}

/// The registry writes the graph page offers; `<project>` is an id or a
/// root path, as in `rtok graph projects`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum ProjectRequest {
    Select {
        project: String,
    },
    Link {
        from: String,
        to: String,
        #[serde(default)]
        both: bool,
    },
    Unlink {
        from: String,
        to: String,
        #[serde(default)]
        both: bool,
    },
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct SetRequest {
    #[serde(default)]
    pub key: String,
    pub value: bool,
}

/// Root of the schema: one property per direction, so every type lands in `$defs` once.
#[derive(JsonSchema)]
#[schemars(title = "WsProtocol")]
#[allow(dead_code)] // never built: exists only to be described
struct WsProtocol {
    snapshot: Snapshot,
    server: ServerFrame,
    client: ClientMessage,
}

/// The schema as committed: pretty JSON with a trailing newline.
pub fn schema_json() -> String {
    let schema = SchemaSettings::draft2020_12()
        .for_serialize()
        .into_generator()
        .into_root_schema_for::<WsProtocol>();
    let mut out = serde_json::to_string_pretty(&schema).unwrap_or_default();
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fails when a `/ws` type changed without regenerating; `RTOK_BLESS=1` rewrites the file,
    /// then `npm --prefix web run gen:api` regenerates the TS from it.
    #[test]
    fn committed_schema_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SCHEMA_PATH);
        let want = schema_json();
        if std::env::var_os("RTOK_BLESS").is_some() {
            std::fs::write(&path, &want).unwrap();
            return;
        }
        let have = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            have == want,
            "{SCHEMA_PATH} is stale: run `RTOK_BLESS=1 cargo nextest run -E 'test(committed_schema_is_current)'` then `npm --prefix web run gen:api`"
        );
    }

    #[test]
    fn client_messages_parse() {
        let m: ClientMessage = serde_json::from_str(r#"{"expand":"abc"}"#).unwrap();
        assert!(matches!(m, ClientMessage::Expand { expand } if expand == "abc"));
        let m: ClientMessage =
            serde_json::from_str(r#"{"set":{"key":"plugins.x.enabled","value":true}}"#).unwrap();
        assert!(
            matches!(m, ClientMessage::Set { set } if set.value && set.key == "plugins.x.enabled")
        );
        assert!(serde_json::from_str::<ClientMessage>(r#"{"set":{"value":"yes"}}"#).is_err());
        let m: ClientMessage =
            serde_json::from_str(r#"{"project":{"action":"select","project":"2"}}"#).unwrap();
        assert!(matches!(
            m,
            ClientMessage::Project {
                project: ProjectRequest::Select { .. }
            }
        ));
        let m: ClientMessage =
            serde_json::from_str(r#"{"project":{"action":"link","from":"1","to":"2"}}"#).unwrap();
        assert!(matches!(
            m,
            ClientMessage::Project {
                project: ProjectRequest::Link { both: false, .. }
            }
        ));
        assert!(serde_json::from_str::<ClientMessage>(r#"{"project":{"action":"drop"}}"#).is_err());
    }

    #[test]
    fn server_frames_carry_their_type() {
        let f = ServerFrame::Expand {
            id: "a".into(),
            text: "b".into(),
        }
        .to_json();
        assert_eq!(f, r#"{"type":"expand","id":"a","text":"b"}"#);
        let f = ServerFrame::Message { text: "hi".into() }.to_json();
        assert_eq!(f, r#"{"type":"message","text":"hi"}"#);
    }
}
