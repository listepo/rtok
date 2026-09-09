//! A complete plugin, in one file: `cargo run -p rtok-plugin-sdk --example shrink`.
//!
//! `shrink` watches for recursive `ls` — a command whose output is usually thousands of
//! lines the model did not ask for — and rewrites it to the shallow form. It is the smallest
//! thing that obeys all three rules at once:
//!
//! - **Fail open**: every step that can be missing is a `?`, so a surprising event returns
//!   `None` and the host runs the command unchanged.
//! - **Lossless**: the original command is archived before it is replaced, and its id travels
//!   in the reason the model sees, so `rtok expand <id>` gives it back.
//! - **Measured**: the saving is a `Measurement` row, or it did not happen.
//!
//! The host here is [`MemoryHost`], which keeps those rows in memory. Inside `rtok` the same
//! plugin gets the real host and writes to the real store; nothing in the plugin changes.

use rtok_plugin_sdk::testing::MemoryHost;
use rtok_plugin_sdk::{
    Class, Ctx, DashboardPage, Manifest, Measurement, Plugin, PreToolDecision, PreToolUse, Surface,
};
use serde::Deserialize;
use serde_json::json;

/// The plugin's own `[plugins.shrink]` section. A plugin declares whatever struct it wants;
/// a missing section is `Default`, which is why every field carries one.
#[derive(Deserialize, Default)]
struct Config {
    /// Estimated tokens the shallow listing is assumed to cost. Configuration exists so the
    /// operator can tune what the plugin claims, not so the plugin can be turned into
    /// something else.
    #[serde(default)]
    assumed_after_tokens: u32,
}

struct Shrink;

impl Plugin for Shrink {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "shrink",
            surfaces: &[Surface::Hook],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new(
            "Shrink",
            "Rewrites recursive `ls` to the shallow form.",
            true,
        )
    }

    fn pre_tool(&self, ev: &PreToolUse, cx: &Ctx) -> Option<PreToolDecision> {
        let cmd = ev.tool_input.get("command")?.as_str()?;
        if ev.tool_name != "Bash" || !cmd.split_whitespace().any(|w| w == "-R" || w == "-r") {
            return None;
        }
        if !cmd.trim_start().starts_with("ls ") {
            return None;
        }

        // Lossless first: nothing is replaced before the original is retrievable.
        let archive_id = cx.put_archive(cmd.as_bytes()).ok()?;
        let short: String = cmd
            .split_whitespace()
            .filter(|w| *w != "-R" && *w != "-r")
            .collect::<Vec<_>>()
            .join(" ");

        let cfg = cx.plugin_config::<Config>("shrink");
        let est_before = cx.estimate(cmd, Class::Code) * 200; // what the recursion would print
        let est_after = cfg.assumed_after_tokens.max(1);
        cx.record(&Measurement {
            plugin: "shrink",
            kind: "rewrite",
            before_bytes: cmd.len() as u64,
            after_bytes: short.len() as u64,
            est_before,
            est_after,
            ref_id: Some(archive_id.clone()),
            call_id: None,
        })
        .ok()?;
        cx.log("info", "plugin", "shrink", &format!("rewrote {cmd:?}"));

        Some(PreToolDecision::Rewrite {
            input: json!({ "command": short }),
            reason: format!("shrink: recursive listing dropped; original is expand({archive_id})"),
        })
    }
}

fn main() {
    let host = MemoryHost::with_config(json!({
        "plugins": { "shrink": { "assumed_after_tokens": 60 } }
    }));
    let cx = Ctx::new(&host);

    for command in ["ls -R src", "ls src", "grep -R todo src"] {
        let input = json!({ "command": command });
        let ev = PreToolUse {
            tool_name: "Bash",
            tool_input: &input,
        };
        match Shrink.pre_tool(&ev, &cx) {
            Some(PreToolDecision::Rewrite { input, reason }) => {
                println!(
                    "{command:>18}  ->  {}\n{:>20}{reason}",
                    input["command"], ""
                );
            }
            Some(PreToolDecision::Deny { reason }) => {
                println!("{command:>18}  ->  denied: {reason}")
            }
            None => println!("{command:>18}  ->  unchanged"),
        }
    }

    for m in host.recorded() {
        println!(
            "\nmeasurement: {}/{} {} -> {} tokens, ref {:?}",
            m.plugin, m.kind, m.est_before, m.est_after, m.ref_id
        );
        // Lossless, demonstrated rather than asserted: the id gives the original back.
        let id = m.ref_id.as_deref().unwrap();
        let original = cx.get_archive(id).unwrap().unwrap();
        println!("expand({id}) = {:?}", String::from_utf8_lossy(&original));
    }
}
