//! Guards for what keeps the Windows CI job honest (T82, T93, T108).

use std::path::{Path, PathBuf};

/// Return the repository root used by each Windows CI guard.
fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// T93: `build.rs` links the bins with an 8 MiB main-thread stack on Windows. Without it
/// the debug `rtok.exe` overflowed its 1 MiB default on nearly every command — this names
/// the cause directly instead of through a hundred crashed e2e tests.
#[cfg(windows)]
#[test]
fn rtok_exe_reserves_an_8_mib_main_thread_stack() {
    let exe = std::fs::read(env!("CARGO_BIN_EXE_rtok")).unwrap();
    let u32_at = |o: usize| u32::from_le_bytes(exe[o..o + 4].try_into().unwrap());
    let pe = u32_at(0x3c) as usize;
    assert_eq!(&exe[pe..pe + 4], b"PE\0\0", "not a PE image");
    // COFF header is 20 bytes; SizeOfStackReserve sits at offset 72 of the optional
    // header in both PE32 (u32) and PE32+ (u64).
    let opt = pe + 4 + 20;
    let reserve = match u16::from_le_bytes([exe[opt], exe[opt + 1]]) {
        0x20b => u64::from_le_bytes(exe[opt + 72..opt + 80].try_into().unwrap()),
        _ => u64::from(u32_at(opt + 72)),
    };
    assert!(reserve >= 8 << 20, "stack reserve is {reserve} bytes");
}

/// T82: files compared byte for byte must reach the working tree as LF on every OS;
/// `.gitattributes` (`eol=lf`) is what beats Windows' `core.autocrlf=true`.
#[test]
fn byte_compared_files_are_lf_in_the_working_tree() {
    let mut crlf = Vec::new();
    for dir in ["tests/trycmd", "skills"] {
        for entry in ignore::WalkBuilder::new(repo().join(dir)).build() {
            let path = entry.unwrap().into_path();
            if path.is_file() && std::fs::read(&path).unwrap().contains(&b'\r') {
                crlf.push(path);
            }
        }
    }
    assert!(crlf.is_empty(), "CR bytes in {crlf:?}");
}

/// T82 / T83: every entry of the `cfg(windows)` exclusion list names a test that exists.
/// A renamed or deleted test would otherwise leave a line that skips nothing, and the
/// list would never read as empty when T83 is done.
#[test]
fn windows_exclusion_list_names_only_existing_tests() {
    let config = std::fs::read_to_string(repo().join(".config/nextest.toml")).unwrap();
    let start = config
        .find("platform = 'cfg(windows)'")
        .expect("the cfg(windows) override");
    let block = &config[start..];
    let filter = block
        .split("'''")
        .nth(1)
        .expect("a ''' default-filter block");

    let sources = rust_sources();
    let binary = regex::Regex::new(r"binary\((\w+)\)").unwrap();
    let exact = regex::Regex::new(r"test\(=([\w:]+)\)").unwrap();
    let alternation = regex::Regex::new(r"test\(/\^[\w:]*\(([\w|]+)\)\$/\)").unwrap();

    let mut missing = Vec::new();
    for c in binary.captures_iter(filter) {
        if !repo().join(format!("tests/{}.rs", &c[1])).is_file() {
            missing.push(format!("binary {}", &c[1]));
        }
    }
    let names = exact
        .captures_iter(filter)
        .map(|c| c[1].rsplit("::").next().unwrap().to_string())
        .chain(
            alternation
                .captures_iter(filter)
                .flat_map(|c| c[1].split('|').map(str::to_string).collect::<Vec<_>>()),
        );
    let mut count = 0;
    for name in names {
        count += 1;
        let def = format!("fn {name}(");
        if !sources.iter().any(|s| s.contains(&def)) {
            missing.push(format!("test {name}"));
        }
    }
    assert!(
        count > 0,
        "parsed no test names — the filter format changed"
    );
    assert!(missing.is_empty(), "stale exclusions: {missing:?}");
}

/// Collect every Rust source file whose test names can appear in nextest filters.
fn rust_sources() -> Vec<String> {
    let mut out = Vec::new();
    for dir in ["src", "tests", "crates"] {
        for entry in ignore::WalkBuilder::new(repo().join(dir)).build() {
            let path = entry.unwrap().into_path();
            if path.extension().is_some_and(|e| e == "rs") {
                out.push(read(&path));
            }
        }
    }
    out
}

/// Read source text for exclusion-list matching, treating unreadable files as empty.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}
