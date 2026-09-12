//! First-party Gate P32 example guest — built to `wasm32-unknown-unknown`, not in `all()`.
#![no_std]

#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn rtok_estimate(ptr: i32, len: i32, class: i32) -> i32;
    fn rtok_record_measurement(ptr: i32, len: i32) -> i32;
}

static MANIFEST: &[u8] = br#"{"id":"wasm-demo","surfaces":["mcp"],"default_on":true,"title":"WASM Demo","summary":"Example out-of-tree plugin.","saves_tokens":true}"#;
static MCP_TOOLS: &[u8] =
    br#"[{"name":"demo","description":"Record a demo measurement.","input_schema":{"type":"object"}}]"#;
static BEFORE: &[u8] = b"012345678901234567";
static AFTER: &[u8] = b"done";

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[unsafe(no_mangle)]
pub extern "C" fn rtok_manifest() -> i64 {
    pack(MANIFEST.as_ptr() as i32, MANIFEST.len() as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn rtok_mcp_tools() -> i64 {
    pack(MCP_TOOLS.as_ptr() as i32, MCP_TOOLS.len() as i32)
}

static TOOL_OUT: &[u8] = br#"{"ok":true}"#;

/// `name` / `args` ignored for the demo; returns packed `(ptr, len)` of JSON in guest memory.
#[unsafe(no_mangle)]
pub extern "C" fn rtok_on_mcp_tool(
    _name_ptr: i32,
    _name_len: i32,
    _args_ptr: i32,
    _args_len: i32,
    _out_ptr: i32,
    _out_cap: i32,
) -> i64 {
    let est_before = unsafe { rtok_estimate(BEFORE.as_ptr() as i32, BEFORE.len() as i32, 0) };
    let est_after = unsafe { rtok_estimate(AFTER.as_ptr() as i32, AFTER.len() as i32, 0) };
    let mut buf = [0u8; 160];
    let n = write_measurement(&mut buf, est_before, est_after);
    if unsafe { rtok_record_measurement(buf.as_ptr() as i32, n as i32) } != 0 {
        return -1;
    }
    pack(TOOL_OUT.as_ptr() as i32, TOOL_OUT.len() as i32)
}

fn pack(ptr: i32, len: i32) -> i64 {
    ((len as u64) << 32 | (ptr as u32) as u64) as i64
}

fn write_measurement(buf: &mut [u8], est_before: i32, est_after: i32) -> usize {
    let head =
        br#"{"plugin":"wasm-demo","kind":"demo","before_bytes":17,"after_bytes":4,"est_before":"#;
    let mid = br#","est_after":"#;
    let tail = br#","ref_id":null,"call_id":null}"#;
    let mut i = 0;
    i += copy(head, &mut buf[i..]);
    i += write_i32(est_before, &mut buf[i..]);
    i += copy(mid, &mut buf[i..]);
    i += write_i32(est_after, &mut buf[i..]);
    i += copy(tail, &mut buf[i..]);
    i
}

fn copy(src: &[u8], dst: &mut [u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n
}

fn write_i32(n: i32, dst: &mut [u8]) -> usize {
    let mut v = n.unsigned_abs();
    let mut tmp = [0u8; 11];
    let mut len = 0usize;
    if v == 0 {
        tmp[0] = b'0';
        len = 1;
    } else {
        while v > 0 {
            tmp[len] = b'0' + (v % 10) as u8;
            len += 1;
            v /= 10;
        }
        tmp[..len].reverse();
    }
    let start = if n < 0 { 1 } else { 0 };
    if n < 0 {
        dst[0] = b'-';
    }
    let need = start + len;
    dst[start..need].copy_from_slice(&tmp[..len]);
    need
}
