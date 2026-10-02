// Copyright (c) 2026 Ivan Tugay
// SPDX-License-Identifier: GPL-3.0-only
// Licensed under GPL-3.0 only; see https://www.gnu.org/licenses/gpl-3.0.html

//! T288: an agent's undelivered messages ride its next `UserPromptSubmit` / `PostToolUse`
//! context. Each is framed by [`crate::render::agent_message_frame`] (T287), the batch is
//! capped at `[agents] push_bytes`, and the text goes through the event's existing budgeted
//! path (`inject::apply` or `cap_budget`). Afterwards only the frames that survived that path
//! are marked delivered — never read, so `rtok agents inbox` still lists them as unread.

use crate::plugin::{Injection, Runtime};
use crate::render::{agent_message_end, agent_message_frame};
use crate::store::Message;

/// What one event pushes.
pub(super) struct Push {
    pub(super) text: String,
    framed: Vec<i32>,
    /// A frame that alone is over the room: it can never be pushed, so the "more" line that
    /// names it is its delivery (otherwise that line would repeat on every event).
    oversized: Vec<i32>,
    more: Option<String>,
}

/// `None` when there is no caller agent (`[agents] enabled = false`, or no host id — `dispatch`
/// folds both into `agent`), nothing is undelivered, or the store fails (fail open, D1).
/// One indexed query (`messages_to_agent`); nothing else runs on an empty inbox.
pub(super) fn pending(cx: &Runtime, agent: Option<&str>) -> Option<Push> {
    let rows = cx.store.undelivered(agent?).ok()?;
    (!rows.is_empty()).then(|| fit(&rows, cx.config.agents.push_bytes as usize))
}

fn more_line(n: usize) -> String {
    format!("… and {n} more: run rtok agents inbox")
}

/// Frames in send order while they fit `cap` bytes; once one does not, it and every later one
/// wait for the next event, and room is kept for the one line that counts them.
fn fit(rows: &[Message], cap: usize) -> Push {
    let frames: Vec<String> = rows.iter().map(agent_message_frame).collect();
    let room = if frames.iter().map(String::len).sum::<usize>() <= cap {
        cap
    } else {
        cap.saturating_sub(more_line(rows.len()).len() + 1)
    };
    let mut push = Push {
        text: String::new(),
        framed: Vec::new(),
        oversized: Vec::new(),
        more: None,
    };
    let mut full = false;
    for (m, frame) in rows.iter().zip(&frames) {
        if frame.len() > room {
            push.oversized.push(m.id);
        } else if !full && push.text.len() + frame.len() <= room {
            push.text.push_str(frame);
            push.framed.push(m.id);
        } else {
            full = true;
        }
    }
    let rest = rows.len() - push.framed.len();
    if rest > 0 {
        let line = more_line(rest);
        push.text.push_str(&line);
        push.more = Some(line);
    }
    push.text.truncate(push.text.trim_end().len());
    push
}

impl Push {
    /// Offered like any other injection; top priority, as the agent id line (T283).
    pub(super) fn injection(&self) -> Injection {
        Injection {
            plugin: "agent_message",
            text: self.text.clone(),
            priority: 10,
        }
    }

    /// Mark delivered what reached `out`, the context the budgeted path finally emitted: a
    /// frame whose closing line is there, and the oversized ones when the "more" line is.
    /// Anything the budget cut stays undelivered and is pushed on a later event.
    pub(super) fn settle(&self, cx: &Runtime, out: &str) {
        let mut ids: Vec<i32> = self
            .framed
            .iter()
            .copied()
            .filter(|&id| {
                let end = agent_message_end(id);
                out.lines().any(|l| l == end)
            })
            .collect();
        if self.more.as_deref().is_some_and(|l| out.contains(l)) {
            ids.extend(&self.oversized);
        }
        let _ = cx.store.mark_delivered(&ids);
    }
}
