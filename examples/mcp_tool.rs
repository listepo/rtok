//! The smallest MCP-tool plugin, registered from outside the catalogue.
//!
//! This is the shape a third-party crate uses (D6): depend on the `rtok` library, implement
//! `Plugin`, and build a registry with `Registry::from_plugins`. Run: `just example`.
//!
//! Serving the tool records one `Measurement` — because a saving that is not a row does
//! not exist (D3). Echo moves no bytes, so before and after are equal; a real tool reports
//! what it saved here.

use anyhow::Result;
use rtok::config::Config;
use rtok::plugin::{Ctx, Measurement, Runtime};
use rtok::plugins::Registry;
use rtok::tokens::Class;
use rtok::{DashboardPage, Manifest, Plugin, Surface, ToolDef};
use serde_json::json;

/// One plugin, one tool.
struct Echo;

impl Echo {
    /// The tool itself: return the given text and record the call, the way `hello_plugin`
    /// records its deny. Recording never fails the call: a broken store is `.ok()`-ed away
    /// (fail open) and the text still goes back.
    fn call(&self, text: &str, cx: &Ctx) -> String {
        let est = cx.estimate(text, Class::Code);
        cx.record(&Measurement {
            plugin: "echo",
            kind: "tool",
            before_bytes: text.len() as u64,
            after_bytes: text.len() as u64,
            est_before: est,
            est_after: est,
            ref_id: None,
            call_id: None,
        })
        .ok();
        text.to_string()
    }
}

impl Plugin for Echo {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "echo",
            surfaces: &[Surface::Mcp],
            default_on: true,
        }
    }

    fn dashboard_page(&self) -> DashboardPage {
        DashboardPage::new("Echo", "Reference MCP tool for plugin authors.", false)
    }

    fn mcp_tools(&self) -> Vec<ToolDef> {
        vec![ToolDef {
            name: "echo",
            description: "Return the given text. Reference tool for plugin authors.",
            input_schema: json!({
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"],
            }),
        }]
    }
}

fn main() -> Result<()> {
    // No catalogue plugin is compiled in here — only what this binary registers.
    let reg = Registry::from_plugins(vec![Box::new(Echo)], &Config::default());

    let tools: Vec<&str> = reg
        .enabled()
        .flat_map(|p| p.mcp_tools())
        .map(|t| t.name)
        .collect();
    assert_eq!(tools, ["echo"], "external plugin's tool was not listed");

    // One call through the tool, end to end like `hello_plugin` drives its hooks:
    // the call leaves exactly one measurement row.
    let rt = Runtime::in_memory("example-session")?;
    let out = Echo.call("hello", &Ctx::new(&rt));
    assert_eq!(out, "hello");
    println!("echo \"hello\" → {out:?}");

    let rows: i64 = rt.store.measurement_count("echo")?;
    println!("measurement rows: {rows}");
    assert_eq!(rows, 1, "the tool call must leave exactly one measurement");

    print!("{}", reg.table());
    println!("mcp tools: {}", tools.join(", "));
    Ok(())
}
