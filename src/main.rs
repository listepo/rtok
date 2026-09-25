fn main() -> anyhow::Result<()> {
    if let Err(e) = rtok::cli::run() {
        let cfg = rtok::config::Config::load_lenient(None, None);
        rtok::log::append(&cfg, "error", "cli", "run", &format!("{e:#}"));
        return Err(e);
    }
    Ok(())
}
