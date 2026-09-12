//! Plugin registry (plan T0.4). One module per catalogue id, each behind a Cargo
//! feature of the same name (`default = all`). Order here is dispatch order.

use crate::config::Config;
use rtok_plugin_sdk::{DashboardPage, Manifest, Plugin};
#[cfg(feature = "measure")]
pub mod measure;

#[cfg(feature = "cmd")]
pub mod cmd;

#[cfg(feature = "read")]
pub mod read;

#[cfg(feature = "archive")]
pub mod archive;

#[cfg(feature = "proxy")]
pub mod proxy;

#[cfg(feature = "inject")]
pub mod inject;

/// Not a catalogue plugin (plan T2.5). Used by `inject` on PreCompact / compact restore.
#[cfg(feature = "inject")]
pub mod checkpoint;

#[cfg(feature = "guard")]
pub mod guard;

#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "graph")]
pub mod graph;

#[cfg(feature = "toon")]
pub mod toon;

#[cfg(feature = "wasm-host")]
mod wasm;

/// Every compiled-in plugin, in dispatch order.
#[allow(clippy::vec_init_then_push, unused_mut)] // each push is cfg-gated by a feature
pub fn all() -> Vec<Box<dyn Plugin>> {
    let mut v: Vec<Box<dyn Plugin>> = Vec::new();
    #[cfg(feature = "measure")]
    v.push(Box::new(measure::Measure));
    #[cfg(feature = "cmd")]
    v.push(Box::new(cmd::Cmd));
    #[cfg(feature = "read")]
    v.push(Box::new(read::Read));
    #[cfg(feature = "archive")]
    v.push(Box::new(archive::Archive));
    #[cfg(feature = "proxy")]
    v.push(Box::new(proxy::Proxy));
    #[cfg(feature = "inject")]
    v.push(Box::new(inject::Inject));
    #[cfg(feature = "guard")]
    v.push(Box::new(guard::Guard));
    #[cfg(feature = "memory")]
    v.push(Box::new(memory::Memory));
    #[cfg(feature = "graph")]
    v.push(Box::new(graph::Graph));
    #[cfg(feature = "toon")]
    v.push(Box::new(toon::Toon));
    v
}

/// Compiled-in plugins plus their enabled state from config.
pub struct Registry {
    plugins: Vec<(Box<dyn Plugin>, bool)>,
}

impl Registry {
    /// Registry over every compiled-in plugin.
    pub fn new(config: &Config) -> Self {
        Self::from_plugins(all(), config)
    }

    /// Registry over a caller-supplied plugin list. This is the entry point for a crate that
    /// embeds `rtok` as a library and registers its own plugins; see `examples/mcp_tool.rs`.
    pub fn from_plugins(plugins: Vec<Box<dyn Plugin>>, config: &Config) -> Self {
        let plugins = plugins
            .into_iter()
            .map(|p| {
                let m = p.manifest();
                let on = config.plugin_enabled(m.id, m.default_on);
                (p, on)
            })
            .collect();
        #[cfg(feature = "wasm-host")]
        let plugins = wasm::append(plugins, config);
        Self { plugins }
    }

    /// Enabled plugins in dispatch order.
    pub fn enabled(&self) -> impl Iterator<Item = &dyn Plugin> {
        self.plugins
            .iter()
            .filter(|(_, on)| *on)
            .map(|(p, _)| p.as_ref())
    }

    /// `(manifest, enabled)` for every compiled-in plugin.
    pub fn manifests(&self) -> Vec<(Manifest, bool)> {
        self.plugins
            .iter()
            .map(|(p, on)| (p.manifest(), *on))
            .collect()
    }

    /// `(manifest, enabled, page)` for every compiled-in plugin. The page comes from the
    /// plugin itself (D23, T23.2) — the operator surfaces never look copy up by id.
    pub fn pages(&self) -> Vec<(Manifest, bool, DashboardPage)> {
        self.plugins
            .iter()
            .map(|(p, on)| (p.manifest(), *on, p.dashboard_page()))
            .collect()
    }

    /// The `rtok plugins` table — one formatter with the CLI's rendering of the model's
    /// Plugins page ([`crate::render::plugins_table`], T15.11), so the two cannot drift.
    pub fn table(&self) -> String {
        let rows: Vec<(&str, bool, Vec<&str>)> = self
            .manifests()
            .into_iter()
            .map(|(m, on)| (m.id, on, m.surfaces.iter().map(|s| s.as_str()).collect()))
            .collect();
        crate::render::plugins_table(&rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CATALOGUE;

    #[test]
    fn registry_matches_catalogue() {
        let reg = Registry::new(&Config::default());
        let got: Vec<(&str, bool)> = reg
            .manifests()
            .iter()
            .map(|(m, _)| (m.id, m.default_on))
            .collect();
        assert_eq!(
            got,
            CATALOGUE.to_vec(),
            "plugins/mod.rs and config::CATALOGUE drifted"
        );
    }

    #[test]
    fn config_overrides_default_on() {
        let mut cfg = Config::default();
        cfg.plugins.cmd.enabled = false;
        cfg.plugins.toon.enabled = true;
        let reg = Registry::new(&cfg);
        let on: Vec<&str> = reg.enabled().map(|p| p.manifest().id).collect();
        assert!(!on.contains(&"cmd"));
        assert!(on.contains(&"toon"));
        assert!(reg.table().contains("cmd      off"));
    }

    /// The external-plugin path: a plugin that is not in the catalogue, registered by hand.
    #[test]
    fn from_plugins_takes_external_plugins() {
        struct Ext;
        impl Plugin for Ext {
            fn manifest(&self) -> Manifest {
                Manifest {
                    id: "ext",
                    surfaces: &[crate::plugin::Surface::Mcp],
                    default_on: true,
                }
            }
            fn dashboard_page(&self) -> DashboardPage {
                DashboardPage::new("Ext", "An out-of-tree plugin.", false)
            }
        }
        let reg = Registry::from_plugins(vec![Box::new(Ext)], &Config::default());
        let on: Vec<&str> = reg.enabled().map(|p| p.manifest().id).collect();
        assert_eq!(on, ["ext"]);
    }
}
