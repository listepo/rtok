//! The smallest MCP-tool plugin, registered from outside the catalogue.
//!
//! This is the shape a third-party crate uses (D6): depend on the `rtok` library, implement
//! `Plugin`, and build a registry with `Registry::from_plugins`. Run: `make example`.

use anyhow::Result;
use rtok::config::Config;
use rtok::plugins::Registry;
use rtok::{Manifest, Plugin, Surface, ToolDef};
use serde_json::json;

/// One plugin, one tool.
struct Echo;

impl Plugin for Echo {
    fn manifest(&self) -> Manifest {
        Manifest {
            id: "echo",
            surfaces: &[Surface::Mcp],
            default_on: true,
        }
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

    print!("{}", reg.table());
    println!("mcp tools: {}", tools.join(", "));
    Ok(())
}
