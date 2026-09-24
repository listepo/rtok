//! T137: what an `image` content block costs. Pixel size comes from the PNG `IHDR` or the
//! JPEG `SOF` header (fixed-offset parse, no image dependency); tokens follow the provider's
//! published formula.

/// Base64 characters decoded to find the header: a JPEG `SOF` sits after EXIF/ICC segments,
/// a few KB into a screenshot; the cap bounds the work per block.
const HEADER_B64: usize = 64 * 1024;

/// Anthropic vision docs, "Resolution and token cost"
/// (https://platform.claude.com/docs/en/build-with-claude/vision, read 2026-09-24): an image
/// costs `⌈width / 28⌉ × ⌈height / 28⌉` visual tokens after downscaling to the model's
/// limits. High-resolution tier (Claude 4.7 and later): long edge 2576 px, 4784 tokens.
const PATCH: u64 = 28;
const LONG_EDGE: u64 = 2576;
const MAX_TOKENS: u64 = 4784;

/// Estimated visual tokens for a `w`×`h` image. An image over the token cap is billed at the
/// cap — the docs' downscale lands a few tokens under it (4784 for 3840×2160).
pub fn tokens(w: u32, h: u32) -> u64 {
    let (mut w, mut h) = (u64::from(w), u64::from(h));
    let long = w.max(h);
    if long > LONG_EDGE {
        w = w * LONG_EDGE / long;
        h = h * LONG_EDGE / long;
    }
    (w.div_ceil(PATCH) * h.div_ceil(PATCH)).min(MAX_TOKENS)
}

/// Decoded byte length of base64 `data` (padding and whitespace skipped).
pub fn decoded_len(data: &str) -> u64 {
    data.bytes().filter(|&c| sextet(c).is_some()).count() as u64 * 3 / 4
}

/// `(width, height)` of a base64 PNG or JPEG; `None` for other formats or a truncated header.
pub fn dims(data: &str) -> Option<(u32, u32)> {
    let b = decode(&data.as_bytes()[..data.len().min(HEADER_B64)]);
    if b.starts_with(b"\x89PNG\r\n\x1a\n") && b.get(12..16) == Some(b"IHDR") {
        return Some((be32(&b, 16)?, be32(&b, 20)?));
    }
    if !b.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut i = 2;
    while i + 3 < b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let m = b[i + 1];
        match m {
            0xFF => i += 1,
            0x01 | 0xD0..=0xD8 => i += 2,
            // SOF0–SOF15 except DHT (C4), JPG (C8) and DAC (CC): precision, height, width.
            0xC0..=0xCF if !matches!(m, 0xC4 | 0xC8 | 0xCC) => {
                let h = be16(&b, i + 5)?;
                let w = be16(&b, i + 7)?;
                return Some((w, h));
            }
            _ => i += 2 + be16(&b, i + 2)? as usize,
        }
    }
    None
}

fn be16(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from(u16::from_be_bytes(
        b.get(i..i + 2)?.try_into().ok()?,
    )))
}

fn be32(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

fn sextet(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// Standard or URL-safe base64; anything outside the alphabet is skipped. Twelve lines
/// instead of a crate for the header bytes of a stats row.
fn decode(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0u32);
    for v in s.iter().copied().filter_map(sextet) {
        acc = ((acc << 6) | u32::from(v)) & 0xFFFF;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn b64(bytes: &[u8]) -> String {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for c in bytes.chunks(3) {
            let n = (u32::from(c[0]) << 16)
                | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
                | u32::from(*c.get(2).unwrap_or(&0));
            for k in 0..=c.len() {
                s.push(A[(n >> (18 - 6 * k) & 63) as usize] as char);
            }
        }
        while !s.len().is_multiple_of(4) {
            s.push('=');
        }
        s
    }

    /// PNG signature + IHDR for a `w`×`h` image, base64.
    pub(crate) fn png(w: u32, h: u32) -> String {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        b.extend(w.to_be_bytes());
        b.extend(h.to_be_bytes());
        b.extend([8, 2, 0, 0, 0]);
        b64(&b)
    }

    /// JPEG SOI, an APP0 segment, then SOF0 for a `w`×`h` image, base64.
    pub(crate) fn jpeg(w: u16, h: u16) -> String {
        let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 16];
        b.extend(b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0");
        b.extend([0xFF, 0xC0, 0, 17, 8]);
        b.extend(h.to_be_bytes());
        b.extend(w.to_be_bytes());
        b.extend([3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
        b64(&b)
    }

    #[test]
    fn png_and_jpeg_headers_give_their_size() {
        assert_eq!(dims(&png(1920, 1080)), Some((1920, 1080)));
        assert_eq!(dims(&jpeg(800, 600)), Some((800, 600)));
        assert_eq!(dims("R0lGODlh"), None);
        assert_eq!(decoded_len(&png(1, 1)), 29);
    }

    /// The docs' table: 200², 1000², 1920×1080 and 3840×2160 on the high-resolution tier.
    #[test]
    fn tokens_match_the_published_table() {
        assert_eq!(tokens(200, 200), 64);
        assert_eq!(tokens(1000, 1000), 1296);
        assert_eq!(tokens(1920, 1080), 2691);
        assert_eq!(tokens(3840, 2160), 4784);
    }
}
