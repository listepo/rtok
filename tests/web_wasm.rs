//! T60.7: WASM bundle size gate for `rtok web`, and the checks that say *why* it grew.
//!
//! The artifact tests skip when `crates/rtok-webui/pkg` has not been built (CI builds it only
//! in the release job); the source tests (release profile, wasm-opt flags, Slint features,
//! bundled fonts) always run, because they are what decides the size in the first place.

use std::collections::{BTreeSet, HashMap};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Upper bound from the optimized build measured in `research.md` (T60.7, 2026-09-18; raised
/// 2026-09-25 after the T227–T232 pages). Still far below the ~10.5 MB a build without
/// wasm-opt produces, which is the failure this gate exists to catch.
const WASM_SIZE_GATE: u64 = 5_000_000;

/// Code section budget. Measured 2,645,655 B on 2026-09-25; generated Slint code for the
/// pages (`slint_generatedMainWindow::InnerComponent_*`) is what grows it.
const CODE_SECTION_BUDGET: u64 = 3_100_000;

/// Data section budget. Measured 1,654,974 B on 2026-09-25, of which about 1 MB is fonts:
/// the three bundled IBM Plex Mono weights and the Inter fallback Slint embeds on wasm.
const DATA_SECTION_BUDGET: u64 = 1_800_000;

/// Bytes of `crates/rtok-webui/ui/fonts/*.ttf` (522,756 B for three weights on 2026-09-25).
/// Every imported font is copied verbatim into the data section.
const BUNDLED_FONT_BUDGET: u64 = 540_000;

/// Custom sections a shipped bundle may carry. Anything else is dead weight in every
/// `rtok` binary: `name` (~3.7 MB) means wasm-opt never ran, `.debug_*` means DWARF, and
/// `__wasm_bindgen_unstable` means wasm-bindgen did not post-process the module.
const ALLOWED_CUSTOM_SECTIONS: [&str; 2] = ["producers", "target_features"];

/// Slint features the web UI builds with. Each extra feature pulls a backend, renderer
/// or parser into the module; add one here only with a measurement in `research.md`.
const ALLOWED_SLINT_FEATURES: [&str; 4] =
    ["backend-winit", "compat-1-2", "renderer-femtovg", "std"];

/// How many of the largest items a size failure lists.
const BREAKDOWN_TOP: usize = 8;

const WASM_MAGIC: &[u8; 4] = b"\0asm";
const WASM_HEADER_LEN: usize = 8;
const LEB_PAYLOAD_MASK: u8 = 0x7f;
const LEB_CONTINUE_BIT: u8 = 0x80;
const LEB_BITS_PER_BYTE: u32 = 7;

const SECTION_CUSTOM: u8 = 0;
const SECTION_IMPORT: u8 = 2;
const SECTION_CODE: u8 = 10;
const SECTION_DATA: u8 = 11;
const SECTION_NAMES: [&str; 13] = [
    "custom",
    "type",
    "import",
    "function",
    "table",
    "memory",
    "global",
    "export",
    "start",
    "element",
    "code",
    "data",
    "datacount",
];

/// Data segment flags (core spec 5.5.12): active in memory 0, passive, active with index.
const DATA_ACTIVE_MEM0: u32 = 0;
const DATA_PASSIVE: u32 = 1;
const DATA_ACTIVE_EXPLICIT: u32 = 2;
const OPCODE_END: u8 = 0x0b;

/// Import kinds (core spec 5.5.5) and the limits flag bit that says a maximum follows.
const IMPORT_FUNC: u8 = 0;
const IMPORT_TABLE: u8 = 1;
const IMPORT_MEMORY: u8 = 2;
const IMPORT_GLOBAL: u8 = 3;
const IMPORT_TAG: u8 = 4;
const LIMITS_HAS_MAX: u8 = 1;

/// `name` custom section subsection holding function names.
const NAME_SUBSECTION_FUNCTIONS: u8 = 1;
/// Longest function name a breakdown line prints.
const NAME_PRINT_LIMIT: usize = 120;

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn read(rel: &str) -> String {
    let path = repo(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn wasm_path() -> PathBuf {
    repo("crates/rtok-webui/pkg/rtok_webui_bg.wasm")
}

/// The built bundle, or `None` (with a skip note) when nothing has been built.
fn built_wasm() -> Option<Vec<u8>> {
    let path = wasm_path();
    if !path.is_file() {
        eprintln!(
            "skip: {} missing — run `just web` or tools/webui-bundle.sh",
            path.display()
        );
        return None;
    }
    Some(std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display())))
}

// ---------------------------------------------------------------- minimal wasm reader

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8], pos: usize) -> Self {
        Self { bytes, pos }
    }

    fn byte(&mut self) -> u8 {
        let b = *self
            .bytes
            .get(self.pos)
            .unwrap_or_else(|| panic!("truncated wasm at offset {}", self.pos));
        self.pos += 1;
        b
    }

    fn leb_u32(&mut self) -> u32 {
        let mut value = 0u32;
        let mut shift = 0u32;
        loop {
            let b = self.byte();
            value |= u32::from(b & LEB_PAYLOAD_MASK) << shift;
            if b & LEB_CONTINUE_BIT == 0 {
                return value;
            }
            shift += LEB_BITS_PER_BYTE;
        }
    }

    fn len(&mut self) -> usize {
        usize::try_from(self.leb_u32()).expect("u32 fits usize")
    }

    fn skip_bytes(&mut self) {
        let n = self.len();
        self.pos += n;
    }

    fn string(&mut self) -> String {
        let n = self.len();
        let s = String::from_utf8_lossy(&self.bytes[self.pos..self.pos + n]).into_owned();
        self.pos += n;
        s
    }

    fn skip_limits(&mut self) {
        let flags = self.byte();
        self.leb_u32();
        if flags & LIMITS_HAS_MAX != 0 {
            self.leb_u32();
        }
    }

    /// Skip a constant expression (an offset) up to and including its `end` opcode.
    fn skip_const_expr(&mut self) {
        while self.byte() != OPCODE_END {}
    }
}

struct Section {
    id: u8,
    /// Custom section name; empty for known sections.
    name: String,
    /// Whole payload size, name included for custom sections.
    size: u64,
    payload: std::ops::Range<usize>,
}

impl Section {
    fn label(&self) -> String {
        if self.id == SECTION_CUSTOM {
            format!("custom '{}'", self.name)
        } else {
            SECTION_NAMES
                .get(usize::from(self.id))
                .map_or_else(|| format!("section {}", self.id), |s| (*s).to_owned())
        }
    }
}

fn sections(wasm: &[u8]) -> Vec<Section> {
    assert!(
        wasm.len() >= WASM_HEADER_LEN && wasm.starts_with(WASM_MAGIC),
        "rtok_webui_bg.wasm is not a wasm module"
    );
    let mut r = Reader::new(wasm, WASM_HEADER_LEN);
    let mut out = Vec::new();
    while r.pos < wasm.len() {
        let id = r.byte();
        let size = r.len();
        let start = r.pos;
        let end = start + size;
        let name = if id == SECTION_CUSTOM {
            let n = r.len();
            String::from_utf8_lossy(&wasm[r.pos..r.pos + n]).into_owned()
        } else {
            String::new()
        };
        out.push(Section {
            id,
            name,
            size: size as u64,
            payload: start..end,
        });
        r.pos = end;
    }
    out
}

/// Sizes of the data segments, largest first.
fn data_segment_sizes(wasm: &[u8], data: &Section) -> Vec<(usize, u64)> {
    let mut r = Reader::new(wasm, data.payload.start);
    let count = r.len();
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        match r.leb_u32() {
            DATA_ACTIVE_MEM0 => r.skip_const_expr(),
            DATA_PASSIVE => {}
            DATA_ACTIVE_EXPLICIT => {
                r.leb_u32();
                r.skip_const_expr();
            }
            other => panic!("unknown data segment flag {other}"),
        }
        let len = r.len();
        r.pos += len;
        out.push((index, len as u64));
    }
    out.sort_by_key(|&(_, len)| std::cmp::Reverse(len));
    out
}

/// Sizes of the function bodies, largest first.
fn function_body_sizes(wasm: &[u8], code: &Section) -> Vec<(usize, u64)> {
    let mut r = Reader::new(wasm, code.payload.start);
    let count = r.len();
    let mut out = Vec::with_capacity(count);
    for index in 0..count {
        let len = r.len();
        r.pos += len;
        out.push((index, len as u64));
    }
    out.sort_by_key(|&(_, len)| std::cmp::Reverse(len));
    out
}

/// Number of imported functions: function indices in the `name` section count them first.
fn imported_function_count(wasm: &[u8], imports: &Section) -> usize {
    let mut r = Reader::new(wasm, imports.payload.start);
    let count = r.len();
    let mut funcs = 0;
    for _ in 0..count {
        r.skip_bytes();
        r.skip_bytes();
        match r.byte() {
            IMPORT_FUNC => {
                r.leb_u32();
                funcs += 1;
            }
            IMPORT_TABLE => {
                r.byte();
                r.skip_limits();
            }
            IMPORT_MEMORY => r.skip_limits(),
            IMPORT_GLOBAL => {
                r.byte();
                r.byte();
            }
            IMPORT_TAG => {
                r.byte();
                r.leb_u32();
            }
            other => panic!("unknown import kind {other}"),
        }
    }
    funcs
}

/// Function names by function index, when the module still has a `name` section — which
/// the shipped one only has if wasm-opt did not run, exactly when names help most.
fn function_names(wasm: &[u8], secs: &[Section]) -> HashMap<usize, String> {
    let mut names = HashMap::new();
    let Some(section) = secs
        .iter()
        .find(|s| s.id == SECTION_CUSTOM && s.name == "name")
    else {
        return names;
    };
    let mut r = Reader::new(wasm, section.payload.start);
    r.skip_bytes();
    while r.pos < section.payload.end {
        let id = r.byte();
        let size = r.len();
        let end = r.pos + size;
        if id == NAME_SUBSECTION_FUNCTIONS {
            for _ in 0..r.len() {
                let index = r.len();
                names.insert(index, r.string());
            }
        }
        r.pos = end;
    }
    names
}

/// What the bundle is made of — the text every size failure carries.
fn breakdown(wasm: &[u8]) -> String {
    let secs = sections(wasm);
    let mut text = String::from("section sizes:\n");
    let mut by_size: Vec<&Section> = secs.iter().collect();
    by_size.sort_by_key(|s| std::cmp::Reverse(s.size));
    for s in &by_size {
        let _ = writeln!(text, "  {:>10} B  {}", s.size, s.label());
    }
    if let Some(data) = secs.iter().find(|s| s.id == SECTION_DATA) {
        let _ = writeln!(
            text,
            "largest data segments (fonts, Unicode tables, strings):"
        );
        for (index, len) in data_segment_sizes(wasm, data)
            .into_iter()
            .take(BREAKDOWN_TOP)
        {
            let _ = writeln!(text, "  {len:>10} B  data[{index}]");
        }
    }
    if let Some(code) = secs.iter().find(|s| s.id == SECTION_CODE) {
        let names = function_names(wasm, &secs);
        let imported = secs
            .iter()
            .find(|s| s.id == SECTION_IMPORT)
            .map_or(0, |s| imported_function_count(wasm, s));
        let _ = writeln!(text, "largest function bodies:");
        for (index, len) in function_body_sizes(wasm, code)
            .into_iter()
            .take(BREAKDOWN_TOP)
        {
            let name = names
                .get(&(imported + index))
                .map(|n| format!(" {}", n.chars().take(NAME_PRINT_LIMIT).collect::<String>()))
                .unwrap_or_default();
            let _ = writeln!(text, "  {len:>10} B  code[{index}]{name}");
        }
    }
    text.push_str(
        "Names per function: `twiggy top -n 40 crates/rtok-webui/target/wasm32-unknown-unknown/release/rtok_webui.wasm` \
         (the pre-wasm-opt module keeps its name section). See research.md, T60.7.",
    );
    text
}

// ---------------------------------------------------------------- artifact checks

#[test]
fn wasm_bundle_is_under_the_measured_gate() {
    let Some(wasm) = built_wasm() else { return };
    let bytes = wasm.len() as u64;
    assert!(
        bytes <= WASM_SIZE_GATE,
        "rtok_webui_bg.wasm is {bytes} bytes, gate is {WASM_SIZE_GATE}\n{}",
        breakdown(&wasm)
    );
}

/// Separate budgets for code and data, so a failure names the half that grew: code means
/// new pages or a heavier dependency, data means fonts, embedded assets or tables.
#[test]
fn wasm_code_and_data_stay_within_their_budgets() {
    let Some(wasm) = built_wasm() else { return };
    let secs = sections(&wasm);
    let size_of = |id: u8| {
        secs.iter()
            .filter(|s| s.id == id)
            .map(|s| s.size)
            .sum::<u64>()
    };
    let code = size_of(SECTION_CODE);
    let data = size_of(SECTION_DATA);
    assert!(
        code <= CODE_SECTION_BUDGET,
        "code section is {code} B, budget {CODE_SECTION_BUDGET} B — new Slint pages or a \
         heavier dependency; check generated `InnerComponent_*` code and `cargo tree` \
         for crates/rtok-webui\n{}",
        breakdown(&wasm)
    );
    assert!(
        data <= DATA_SECTION_BUDGET,
        "data section is {data} B, budget {DATA_SECTION_BUDGET} B — a new font, an embedded \
         image or a Unicode/ICU table; the largest segments are listed below\n{}",
        breakdown(&wasm)
    );
}

/// A `name` or `.debug_*` section is the signature of a bundle wasm-opt never touched (or
/// a profile that turned debug info on): it is how the 10.5 MB build looked.
#[test]
fn wasm_bundle_carries_no_debug_or_name_sections() {
    let Some(wasm) = built_wasm() else { return };
    let unexpected: Vec<String> = sections(&wasm)
        .into_iter()
        .filter(|s| s.id == SECTION_CUSTOM && !ALLOWED_CUSTOM_SECTIONS.contains(&s.name.as_str()))
        .map(|s| format!("'{}' ({} B)", s.name, s.size))
        .collect();
    assert!(
        unexpected.is_empty(),
        "rtok_webui_bg.wasm carries custom sections {unexpected:?}; allowed: \
         {ALLOWED_CUSTOM_SECTIONS:?}. 'name' means wasm-opt did not run (check the \
         [package.metadata.wasm-pack] flags and that the script did not pass --no-opt/--dev); \
         '.debug_*' means the release profile emits debug info."
    );
}

// ---------------------------------------------------------------- source checks

fn webui_manifest() -> toml_edit::DocumentMut {
    read("crates/rtok-webui/Cargo.toml")
        .parse()
        .expect("crates/rtok-webui/Cargo.toml parses")
}

fn item_str(item: Option<&toml_edit::Item>) -> String {
    item.map(|i| i.to_string().trim().to_owned())
        .unwrap_or_default()
}

/// The webui crate is outside the workspace, so this profile is the one wasm-pack uses.
/// Every line of it was measured in research.md (T60.7).
#[test]
fn webui_release_profile_stays_size_first() {
    let doc = webui_manifest();
    let profile = doc
        .get("profile")
        .and_then(|p| p.get("release"))
        .expect("[profile.release] in crates/rtok-webui/Cargo.toml");
    let expect = [
        (
            "opt-level",
            "\"z\"",
            "\"s\" measured ~390 KB larger after wasm-opt",
        ),
        (
            "lto",
            "true",
            "without fat LTO dead Slint code survives into the module",
        ),
        (
            "codegen-units",
            "1",
            "more units block cross-crate inlining and dead-code removal",
        ),
        (
            "panic",
            "\"abort\"",
            "unwinding tables and landing pads bloat every function",
        ),
    ];
    for (key, want, why) in expect {
        let got = item_str(profile.get(key));
        assert_eq!(
            got, want,
            "crates/rtok-webui [profile.release] {key} = {got:?}, want {want}: {why}"
        );
    }
    for key in ["debug", "incremental"] {
        let got = item_str(profile.get(key));
        assert!(
            got.is_empty() || got == "false" || got == "0",
            "crates/rtok-webui [profile.release] {key} = {got}: debug info and incremental \
             builds make the release module larger"
        );
    }
}

/// wasm-pack's default is `-O`, and the release runners have no other wasm-opt: that is
/// how run 36165796413 shipped 4,522,155 B while a local build measured 4.35 MB.
#[test]
fn wasm_pack_runs_wasm_opt_for_size() {
    let doc = webui_manifest();
    let flags = doc
        .get("package")
        .and_then(|p| p.get("metadata"))
        .and_then(|m| m.get("wasm-pack"))
        .and_then(|w| w.get("profile"))
        .and_then(|p| p.get("release"))
        .and_then(|r| r.get("wasm-opt"))
        .and_then(toml_edit::Item::as_array)
        .expect("[package.metadata.wasm-pack.profile.release] wasm-opt = [...] in crates/rtok-webui/Cargo.toml");
    let flags: Vec<&str> = flags.iter().filter_map(toml_edit::Value::as_str).collect();
    assert!(
        flags.contains(&"-Oz"),
        "wasm-pack wasm-opt flags {flags:?} lack -Oz; wasm-pack falls back to -O (~170 KB larger)"
    );
    for keep in ["-g", "--debuginfo"] {
        assert!(
            !flags.contains(&keep),
            "wasm-opt flag {keep} keeps the name section (~3.7 MB) in the shipped module"
        );
    }

    let script = read("tools/webui-bundle.sh");
    let build = script
        .lines()
        .find(|l| l.trim_start().starts_with("wasm-pack build"))
        .expect("wasm-pack build in tools/webui-bundle.sh");
    assert!(
        build.contains("--release"),
        "webui-bundle.sh builds without --release: {build}"
    );
    for bad in ["--no-opt", "--dev", "--profiling", "--debug"] {
        assert!(
            !build.contains(bad),
            "webui-bundle.sh passes {bad} to wasm-pack, which skips or weakens wasm-opt: {build}"
        );
    }
}

/// Each Slint feature can pull a backend, renderer or parser into the module.
#[test]
fn webui_slint_features_stay_minimal() {
    let doc = webui_manifest();
    let slint = doc
        .get("dependencies")
        .and_then(|d| d.get("slint"))
        .expect("slint in crates/rtok-webui [dependencies]");
    assert_eq!(
        item_str(slint.get("default-features")),
        "false",
        "slint default features pull in backends and renderers the web UI never uses"
    );
    let features: BTreeSet<&str> = slint
        .get("features")
        .and_then(toml_edit::Item::as_array)
        .expect("slint features array")
        .iter()
        .filter_map(toml_edit::Value::as_str)
        .collect();
    let allowed: BTreeSet<&str> = ALLOWED_SLINT_FEATURES.into_iter().collect();
    let extra: Vec<&&str> = features.difference(&allowed).collect();
    assert!(
        extra.is_empty(),
        "new slint features {extra:?} in crates/rtok-webui; measure the bundle before and \
         after, record it in research.md (T60.7), then add them to ALLOWED_SLINT_FEATURES"
    );
}

fn font_dir() -> PathBuf {
    repo("crates/rtok-webui/ui/fonts")
}

fn bundled_fonts() -> Vec<(String, u64)> {
    let mut fonts: Vec<(String, u64)> = std::fs::read_dir(font_dir())
        .expect("crates/rtok-webui/ui/fonts")
        .map(|e| e.expect("font dir entry").path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
        })
        .map(|p| {
            let size = p.metadata().expect("font metadata").len();
            let name = p
                .file_name()
                .and_then(|n| n.to_str())
                .expect("utf-8 name")
                .to_owned();
            (name, size)
        })
        .collect();
    fonts.sort();
    fonts
}

/// Fonts are the largest single thing in the data section: each weight is ~175 KB.
#[test]
fn bundled_fonts_stay_within_budget() {
    let fonts = bundled_fonts();
    let total: u64 = fonts.iter().map(|(_, size)| size).sum();
    assert!(
        total <= BUNDLED_FONT_BUDGET,
        "ui/fonts holds {total} B of fonts, budget {BUNDLED_FONT_BUDGET} B: {fonts:?}. \
         Every imported weight lands in rtok_webui_bg.wasm verbatim."
    );
}

/// A font on disk that no `.slint` file imports is not embedded, but one that is imported
/// and never referenced still is; keep the two lists equal so neither drifts.
#[test]
fn every_bundled_font_is_imported_and_every_import_exists() {
    let ui = repo("crates/rtok-webui/ui");
    let mut imported = BTreeSet::new();
    for entry in std::fs::read_dir(&ui).expect("crates/rtok-webui/ui") {
        let path = entry.expect("ui entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("slint") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read .slint");
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("import \"") {
                let file = rest.split('"').next().unwrap_or_default();
                if let Some(name) = Path::new(file).file_name().and_then(|n| n.to_str())
                    && file.contains("fonts/")
                {
                    imported.insert(name.to_owned());
                }
            }
        }
    }
    let on_disk: BTreeSet<String> = bundled_fonts().into_iter().map(|(n, _)| n).collect();
    assert_eq!(
        imported, on_disk,
        "fonts imported by crates/rtok-webui/ui/*.slint and files in ui/fonts differ"
    );
}
