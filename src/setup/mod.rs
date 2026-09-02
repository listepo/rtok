//! Host installers (`rtok setup <host>`).

pub mod claude;
pub mod codex;
pub mod cursor;
pub mod migrate;
pub mod opencode;

pub(crate) fn openai_proxy_url(cfg: &crate::config::Config) -> String {
    format!("http://{}:{}/v1", cfg.proxy.bind, cfg.proxy.port)
}
