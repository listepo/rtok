//! `rtok filter --stdin`: compress stdin without executing (plan T10.2, T70.1).

use crate::config::Config;
use rtok_plugin_sdk::{Archive, Class, Measurement};

use super::{formatters, rules};

/// Filter `input` as if it were the stdout of `cmd`. Fail-open: never error.
pub fn run(cmd: &str, input: &str) -> String {
    let settings = Config::load_with(None, None)
        .map(|c| rules::Settings::from_config(&c))
        .unwrap_or_else(|_| rules::Settings::builtin());
    compress_only(&settings, cmd, input)
}

/// Archive (when the shortening dropped something), compress, trailer, measure.
/// A store error falls back to [`run`].
pub fn run_with_store(cfg: &Config, cmd: &str, input: &str) -> String {
    let settings = rules::Settings::from_config(cfg);
    let argv: Vec<String> = cmd.split_whitespace().map(str::to_string).collect();
    let cx = match crate::plugin::Runtime::open(cfg.clone(), "filter") {
        Ok(cx) => cx,
        Err(_) => return compress_only(&settings, cmd, input),
    };
    // The id is the body's sha256, so the filter can name it before any store write.
    let id = crate::store::hex_sha256(input.as_bytes());
    let (filtered, kind) = formatters::compress(&settings, &argv, input, 0, &id);
    let lines = if input.is_empty() {
        0
    } else {
        input.lines().count() as u32
    };
    // T160: a whitespace/ANSI-only change has nothing to expand — no trailer, and then
    // nothing references the id, so the archive row is skipped too. An inline
    // `expand <id>` marker already is the pointer and suppresses the trailer, but the
    // archive it names must exist.
    let named = super::run::names_the_id(&filtered, &id);
    let pointer = super::run::needs_pointer(
        lines,
        cfg.plugins.cmd.trailer_min_lines,
        input.as_bytes(),
        &filtered,
        named,
    );
    if (pointer || named) && cx.put_archive(input.as_bytes()).is_err() {
        return compress_only(&settings, cmd, input);
    }
    let mut out = filtered;
    if pointer {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(&format!(
            "[rtok {id} · {lines} lines · expand: rtok expand {id}]\n"
        ));
    }
    let family = formatters::family(&argv);
    let _ = cx.record(&Measurement {
        plugin: "cmd",
        kind,
        before_bytes: input.len() as u64,
        after_bytes: out.len() as u64,
        est_before: cx.estimate(input, Class::Code),
        est_after: cx.estimate(&out, Class::Code),
        ref_id: (pointer || named).then(|| format!("{family}:{id}")),
        call_id: None,
    });
    out
}

fn compress_only(settings: &rules::Settings, cmd: &str, input: &str) -> String {
    let argv: Vec<String> = cmd.split_whitespace().map(str::to_string).collect();
    formatters::compress(settings, &argv, input, 0, "").0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_from_stdin_drops_boilerplate() {
        let raw = "\
On branch main
Changes not staged for commit:
  (use \"git add <file>...\" to update what will be committed)
	modified:   src/lib.rs
";
        let out = run("git status", raw);
        assert!(out.contains("On branch main"), "{out}");
        assert!(out.contains("modified:   src/lib.rs"), "{out}");
        assert!(!out.contains("Changes not staged"), "{out}");
    }

    fn eighty_lines() -> String {
        (0..80)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn read_family_records_a_rule_row_and_expand_trailer() {
        let (cfg, _dir) = crate::testutil::config("filter-read");
        let raw = eighty_lines();
        let out = run_with_store(&cfg, "read src/lib.rs", &raw);
        assert!(out.contains("expand"), "{out}");
        assert!(out.contains("rtok expand"), "{out}");
        assert!(out.len() < raw.len(), "shortened:\n{out}");
        let cx = crate::plugin::Runtime::open(cfg, "filter").unwrap();
        let rows = cx.store.list_measurements("cmd").unwrap();
        assert_eq!(rows.len(), 1, "{rows:?}");
        assert_eq!(rows[0].kind, "rule");
        assert!(
            rows[0].ref_id.as_deref().unwrap_or("").starts_with("read:"),
            "{:?}",
            rows[0].ref_id
        );
    }

    #[test]
    fn grep_find_ls_each_record_a_family_row() {
        let (cfg, _dir) = crate::testutil::config("filter-families");
        let raw = eighty_lines();
        for cmd in ["grep foo", "find *.rs", "ls src"] {
            let out = run_with_store(&cfg, cmd, &raw);
            assert!(out.contains("expand"), "{cmd}: {out}");
        }
        let cx = crate::plugin::Runtime::open(cfg, "filter").unwrap();
        let rows = cx.store.list_measurements("cmd").unwrap();
        assert_eq!(rows.len(), 3, "{rows:?}");
        let families: Vec<&str> = rows
            .iter()
            .map(|r| {
                r.ref_id
                    .as_deref()
                    .unwrap_or("")
                    .split(':')
                    .next()
                    .unwrap_or("")
            })
            .collect();
        assert_eq!(families, ["grep", "find", "ls"]);
        // All three are rule-engine families: `format()` has no `ls`/`find` arm, they
        // compact through `[ls]` / `[find]` (`group = "dir"`) in `rules/default.toml`.
        assert_eq!(
            rows.iter().map(|r| r.kind.as_str()).collect::<Vec<_>>(),
            ["rule"; 3]
        );
    }
}
