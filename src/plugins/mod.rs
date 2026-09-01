//! Plugin registry (plan T0.4). One module per catalogue id, each behind a Cargo
//! feature of the same name (`default = all`). Order here is dispatch order.

use crate::config::Config;
use crate::plugin::{Kind, Manifest, Plugin};

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
    pub fn new(config: &Config) -> Self {
        let plugins = all()
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
        let mut out = format!("{:<9}{:<9}{:<9}surfaces\n", "id", "kind", "enabled");
        for (m, on) in self.manifests() {
            let kind = match m.kind {
                Kind::Native => "native",
                Kind::Adapter => "adapter",
            };
            let surfaces: Vec<&str> = m.surfaces.iter().map(|s| s.as_str()).collect();
            out.push_str(&format!(
                "{:<9}{:<9}{:<9}{}\n",
                m.id,
                kind,
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
        cfg.plugins.entry("cmd".into()).or_default().enabled = Some(false);
        cfg.plugins.entry("toon".into()).or_default().enabled = Some(true);
        let reg = Registry::new(&cfg);
        let on: Vec<&str> = reg.enabled().map(|p| p.manifest().id).collect();
        assert!(!on.contains(&"cmd"));
        assert!(on.contains(&"toon"));
        assert!(reg.table().contains("cmd      native   off"));
    }
}
