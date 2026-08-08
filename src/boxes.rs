//! Minimal ISOBMFF box writing and walking.
//!
//! Only what a single-item still image needs. The structure mirrors AVIF's, so
//! a HEIF reader can walk the boxes even though it will not know the codec.

/// Wrap `body` in a box: `[u32 size][4cc type][body]`.
pub fn bx(typ: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&((8 + body.len()) as u32).to_be_bytes());
    out.extend_from_slice(typ);
    out.extend_from_slice(body);
    out
}

/// Wrap `body` in a FullBox: [`bx`] plus a leading version + 24-bit flags.
/// The body therefore begins 12 bytes into the returned box.
pub fn full_bx(typ: &[u8; 4], version: u8, flags: u32, body: &[u8]) -> Vec<u8> {
    let mut b = Vec::with_capacity(4 + body.len());
    b.push(version);
    b.extend_from_slice(&flags.to_be_bytes()[1..]);
    b.extend_from_slice(body);
    bx(typ, &b)
}

pub fn push_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub fn push_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

pub fn rd_u16(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*d.get(at)?, *d.get(at + 1)?]))
}

pub fn rd_u32(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *d.get(at)?,
        *d.get(at + 1)?,
        *d.get(at + 2)?,
        *d.get(at + 3)?,
    ]))
}

/// One box found by [`walk`].
pub struct Box_<'a> {
    pub typ: [u8; 4],
    /// Payload, excluding the 8-byte header.
    pub body: &'a [u8],
    /// Offset of `body` within the buffer `walk` was given.
    pub body_at: usize,
}

/// Iterate the boxes laid out consecutively in `data`.
///
/// Stops at the first malformed header rather than guessing, so a truncated or
/// hostile file yields the boxes read so far instead of a panic. A size of 0
/// ("to end of file") and 1 ("64-bit size follows") are both rejected: neither
/// occurs in what this crate writes, and accepting them would mean trusting a
/// length this parser cannot bound.
pub fn walk(data: &[u8]) -> Vec<Box_<'_>> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= data.len() {
        let size = match rd_u32(data, i) {
            Some(s) => s as usize,
            None => break,
        };
        if size < 8 || i.checked_add(size).is_none_or(|e| e > data.len()) {
            break;
        }
        let mut typ = [0u8; 4];
        typ.copy_from_slice(&data[i + 4..i + 8]);
        out.push(Box_ {
            typ,
            body: &data[i + 8..i + size],
            body_at: i + 8,
        });
        i += size;
    }
    out
}

/// Find a direct child box by type.
pub fn find<'a>(boxes: &'a [Box_<'a>], typ: &[u8; 4]) -> Option<&'a Box_<'a>> {
    boxes.iter().find(|b| &b.typ == typ)
}
