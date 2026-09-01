//! Plugin registry (plan T0.4). One module per catalogue id, each behind a Cargo
//! feature of the same name (`default = all`). Order here is dispatch order.

use crate::config::Config;
use crate::plugin::{Manifest, Plugin};

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

#[cfg(feature = "guard")]
pub mod guard;

#[cfg(feature = "memory")]
pub mod memory;

#[cfg(feature = "graph")]
pub mod graph;

#[cfg(feature = "toon")]
pub mod toon;

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

    /// The `rtok plugins` table.
    pub fn table(&self) -> String {
        let mut out = format!("{:<9}{:<9}surfaces\n", "id", "enabled");
        for (m, on) in self.manifests() {
            let surfaces: Vec<&str> = m.surfaces.iter().map(|s| s.as_str()).collect();
            out.push_str(&format!(
                "{:<9}{:<9}{}\n",
                m.id,
                if on { "on" } else { "off" },
                surfaces.join(",")
            ));
        }
        out
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
        }
        let reg = Registry::from_plugins(vec![Box::new(Ext)], &Config::default());
        let on: Vec<&str> = reg.enabled().map(|p| p.manifest().id).collect();
        assert_eq!(on, ["ext"]);
    }
}
