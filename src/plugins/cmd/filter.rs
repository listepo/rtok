//! `rtok filter --stdin`: compress stdin without executing (plan T10.2).

use super::formatters;

/// Filter `input` as if it were the stdout of `cmd`. Fail-open: never error.
pub fn run(cmd: &str, input: &str) -> String {
    let argv: Vec<String> = cmd.split_whitespace().map(str::to_string).collect();
    formatters::compress(&argv, input, 0, "").0
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
}
