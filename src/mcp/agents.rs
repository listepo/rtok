// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-or-later
// Licensed under GPL-3.0 or later; see https://www.gnu.org/licenses/gpl-3.0.html

//! T284 (D34): MCP `agents_list`, `agent_show` and `agent_status_set`, the agent-facing side
//! of `rtok agents sessions` / `show` / `status`. The first two return the CLI's `--json`
//! (one model function, [`crate::web::model::agents`]); only the status writer needs the
//! session's linked agent, and it can only set its own.

use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::plugin::{Runtime, ToolDef};
use crate::store::AgentDetail;
use crate::web::model;

pub fn list_def() -> ToolDef {
    ToolDef {
        name: "agents_list",
        description: "Every rtok agent session with its host, state, activity, status, claimed worktrees and unread message count. all=true adds ended ones.",
        input_schema: json!({"type":"object","properties":{"all":{"type":"boolean"}}}),
    }
}

pub fn show_def() -> ToolDef {
    ToolDef {
        name: "agent_show",
        description: "One rtok agent by id or id prefix: host, model, cwd, claimed worktrees, activity, status, sub-agents, unread messages.",
        input_schema: json!({"type":"object","properties":{"id":{"type":"string"}},"required":["id"]}),
    }
}

pub fn status_def() -> ToolDef {
    ToolDef {
        name: "agent_status_set",
        description: "Say what this session is busy with (max 120 chars, plain text), shown in agents_list and agent_show. Blank text (a single space) clears it.",
        input_schema: json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}),
    }
}

fn now() -> i64 {
    crate::log::now() as i64
}

pub fn list(cx: &Runtime, args: &Value) -> Result<String> {
    let all = args["all"].as_bool().unwrap_or(false);
    Ok(serde_json::to_string(&model::agent_sessions(
        &cx.config,
        all,
        now(),
    )?)?)
}

pub fn show(cx: &Runtime, args: &Value) -> Result<String> {
    let Some(id) = args["id"].as_str().filter(|s| !s.is_empty()) else {
        bail!("`id` is required");
    };
    Ok(serde_json::to_string(&model::agent_show(
        &cx.config,
        id,
        now(),
    )?)?)
}

pub fn set_status(cx: &Runtime, me: &AgentDetail, args: &Value) -> Result<String> {
    let Some(text) = args["text"].as_str() else {
        bail!("`text` is required");
    };
    Ok(match model::set_status(&cx.config, Some(&me.id), text)? {
        Some(text) => format!("status: {text}"),
        None => "status cleared".into(),
    })
}
