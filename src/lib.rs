//! # rusty_av2f — AV2F, an **experimental** still-image container for AV2
//!
//! AV2F packages a single AV2 still picture in an ISOBMFF/HEIF file, in the
//! same shape AVIF uses for AV1: `ftyp` + `meta` + `mdat`, one image item, with
//! a codec configuration property.
//!
//! ## Read this before producing files
//!
//! **This is not an AOM standard.** AVIF is defined by a published AOM
//! specification that fixes its brand, item type and configuration record. No
//! equivalent document exists for AV2 — so AV2F's four-character codes are
//! *chosen*, not *specified*, and every one of them lives in [`fourcc`] so a
//! real specification can be adopted by editing one file.
//!
//! Concretely: **files written by this crate are readable by this crate and by
//! nothing else.** Do not ship them anywhere that expects interoperability.
//! They are useful for pipeline work, for measuring AV2 against AVIF on still
//! images, and for having the container ready the day a specification lands.
//!
//! ## Payload restriction, and why
//!
//! [`encode`] requires an AV2 still picture coded with the **full**
//! still-picture header (`avmenc --full-still-picture-hdr`, i.e.
//! `single_picture_header_flag = 0`).
//!
//! The compact alternative is what a still-image format would naturally use,
//! but `rusty_av2d` does not yet decode it bit-exactly and currently refuses
//! it, so writing such a payload would produce a file our own decoder rejects.
//! Encoding the full-header form sidesteps that entirely and is byte-identical
//! against the reference decoder today. When the compact form lands, relax the
//! check in [`encode`] and set [`Config::full_still_picture_header`] to false.
//!
//! ## Example
//!
//! ```no_run
//! use rusty_av2f::{encode, decode, Config, Params};
//!
//! # fn main() -> Result<(), rusty_av2f::Error> {
//! # let av2_still: Vec<u8> = Vec::new();
//! let file = encode(&Params { width: 432, height: 240, config: Config::default() }, &av2_still)?;
//! let img = decode(&file)?;
//! assert_eq!((img.width, img.height), (432, 240));
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]

pub mod boxes;
pub mod config;
pub mod fourcc;

use boxes::{bx, find, full_bx, push_u16, push_u32, rd_u16, rd_u32, walk};
pub use config::{Config, Subsampling};
use fourcc::{BRAND, COMPATIBLE_BRANDS, CONFIG_BOX, ITEM_TYPE};

/// Everything that can go wrong reading or writing an AV2F file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Not an AV2F file (no `ftyp`, or no [`fourcc::BRAND`]).
    NotAv2f,
    /// Structurally malformed: a required box is missing or truncated.
    Malformed(&'static str),
    /// Well-formed but carries something this crate will not handle.
    Unsupported(String),
    /// Zero width/height, or an empty payload.
    Invalid(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::NotAv2f => write!(f, "not an AV2F file"),
            Error::Malformed(w) => write!(f, "malformed AV2F: {w}"),
            Error::Unsupported(w) => write!(f, "unsupported AV2F: {w}"),
            Error::Invalid(w) => write!(f, "invalid AV2F: {w}"),
        }
    }
}

impl std::error::Error for Error {}

/// What [`encode`] needs beyond the payload itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Params {
    pub width: u32,
    pub height: u32,
    pub config: Config,
}

/// A decoded AV2F file: the image description, and the coded AV2 payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image<'a> {
    pub width: u32,
    pub height: u32,
    pub config: Config,
    /// The AV2 still-picture bitstream, ready for a decoder.
    pub payload: &'a [u8],
}

/// Score `data` as an AV2F file, 0–100, in the shape format probes conventionally use.
///
/// 100 for our brand; 50 for a generic HEIF file, which *might* be AV2F from a
/// different writer but cannot be assumed to be.
pub fn probe(data: &[u8]) -> i32 {
    if data.len() < 12 || &data[4..8] != b"ftyp" {
        return 0;
    }
    let size = match rd_u32(data, 0) {
        Some(s) => (s as usize).clamp(12, data.len()),
        None => return 0,
    };
    if &data[8..12] == BRAND {
        return 100;
    }
    let mut i = 16;
    while i + 4 <= size {
        if &data[i..i + 4] == BRAND {
            return 100;
        }
        i += 4;
    }
    match &data[8..12] {
        b"mif1" | b"miaf" => 50,
        _ => 0,
    }
}

/// Package an AV2 still picture as an AV2F file.
///
/// `payload` must be the AV2 bitstream for a single still picture coded with
/// the full still-picture header — see the crate docs for why.
pub fn encode(params: &Params, payload: &[u8]) -> Result<Vec<u8>, Error> {
    if params.width == 0 || params.height == 0 {
        return Err(Error::Invalid("image dimensions are zero"));
    }
    if payload.is_empty() {
        return Err(Error::Invalid("payload is empty"));
    }
    if !params.config.full_still_picture_header {
        return Err(Error::Unsupported(
            "compact still-picture header: rusty_av2d does not decode it bit-exactly yet, \
             so writing it would produce a file our own decoder refuses. Encode with \
             `avmenc --full-still-picture-hdr`."
                .into(),
        ));
    }

    // --- iprp: ispe, av2C, pixi, and the association tying them to item 1 ---
    let mut ipco = Vec::new();
    ipco.extend_from_slice(&ispe(params.width, params.height)); // property #1
    ipco.extend_from_slice(&params.config.to_box()); // property #2
    ipco.extend_from_slice(&pixi(params.config)); // property #3
    let ipco = bx(b"ipco", &ipco);

    let mut iprp = Vec::new();
    iprp.extend_from_slice(&ipco);
    iprp.extend_from_slice(&ipma());
    let iprp = bx(b"iprp", &iprp);

    let (iloc, off_field_in_iloc) = build_iloc(payload.len() as u32);

    let mut meta_body = Vec::new();
    meta_body.extend_from_slice(&hdlr());
    meta_body.extend_from_slice(&pitm());
    let iloc_start = meta_body.len();
    meta_body.extend_from_slice(&iloc);
    meta_body.extend_from_slice(&iinf());
    meta_body.extend_from_slice(&iprp);
    let mut meta = full_bx(b"meta", 0, 0, &meta_body);

    // Patch the extent offset now that the layout is known. The meta FullBox
    // body starts 12 bytes into the box (8 header + 4 version/flags).
    let ftyp = ftyp();
    let off_field = 12 + iloc_start + off_field_in_iloc;
    let mdat_data_offset = ftyp.len() + meta.len() + 8; // +8 for the mdat header
    meta.get_mut(off_field..off_field + 4)
        .ok_or(Error::Malformed("iloc offset field out of range"))?
        .copy_from_slice(&(mdat_data_offset as u32).to_be_bytes());

    let mut file = Vec::with_capacity(ftyp.len() + meta.len() + 8 + payload.len());
    file.extend_from_slice(&ftyp);
    file.extend_from_slice(&meta);
    file.extend_from_slice(&bx(b"mdat", payload));
    Ok(file)
}

/// Parse an AV2F file, returning the image description and a borrow of the payload.
pub fn decode(data: &[u8]) -> Result<Image<'_>, Error> {
    if probe(data) < 100 {
        return Err(Error::NotAv2f);
    }
    let top = walk(data);
    let meta = find(&top, b"meta").ok_or(Error::Malformed("no meta box"))?;
    // meta is a FullBox: skip version+flags to reach its children.
    let meta_children = walk(meta.body.get(4..).ok_or(Error::Malformed("short meta"))?);

    // Confirm the item is ours before trusting anything else about it.
    let iinf = find(&meta_children, b"iinf").ok_or(Error::Malformed("no iinf box"))?;
    if !iinf
        .body
        .windows(4)
        .any(|w| w == ITEM_TYPE.as_slice())
    {
        return Err(Error::Unsupported(format!(
            "item type is not {}",
            String::from_utf8_lossy(ITEM_TYPE)
        )));
    }

    let iprp = find(&meta_children, b"iprp").ok_or(Error::Malformed("no iprp box"))?;
    let ipco = find(&walk(iprp.body), b"ipco")
        .map(|b| b.body.to_vec())
        .ok_or(Error::Malformed("no ipco box"))?;
    let props = walk(&ipco);

    let ispe = find(&props, b"ispe").ok_or(Error::Malformed("no ispe property"))?;
    let width = rd_u32(ispe.body, 4).ok_or(Error::Malformed("short ispe"))?;
    let height = rd_u32(ispe.body, 8).ok_or(Error::Malformed("short ispe"))?;
    if width == 0 || height == 0 {
        return Err(Error::Invalid("image dimensions are zero"));
    }

    let cfg_box = find(&props, CONFIG_BOX).ok_or(Error::Malformed("no av2C property"))?;
    let config = Config::from_body(cfg_box.body).ok_or_else(|| {
        Error::Unsupported("av2C record was not written by this crate".into())
    })?;
    if !config.full_still_picture_header {
        return Err(Error::Unsupported(
            "payload uses the compact still-picture header, which rusty_av2d does not yet \
             decode bit-exactly"
                .into(),
        ));
    }

    // iloc gives the payload's absolute offset and length.
    let iloc = find(&meta_children, b"iloc").ok_or(Error::Malformed("no iloc box"))?;
    let (off, len) = parse_iloc(iloc.body).ok_or(Error::Malformed("unsupported iloc layout"))?;
    let payload = data
        .get(off..off.checked_add(len).ok_or(Error::Malformed("iloc extent overflows"))?)
        .ok_or(Error::Malformed("iloc extent is outside the file"))?;
    if payload.is_empty() {
        return Err(Error::Invalid("payload is empty"));
    }

    Ok(Image {
        width,
        height,
        config,
        payload,
    })
}

// ---------------------------------------------------------------------------
// Box builders
// ---------------------------------------------------------------------------

fn ftyp() -> Vec<u8> {
    let mut b = Vec::new();
    b.extend_from_slice(BRAND);
    push_u32(&mut b, 0); // minor_version
    for brand in COMPATIBLE_BRANDS {
        b.extend_from_slice(brand);
    }
    bx(b"ftyp", &b)
}

fn hdlr() -> Vec<u8> {
    let mut b = Vec::new();
    push_u32(&mut b, 0); // pre_defined
    b.extend_from_slice(b"pict"); // handler_type
    push_u32(&mut b, 0);
    push_u32(&mut b, 0);
    push_u32(&mut b, 0); // reserved[3]
    b.push(0); // name = ""
    full_bx(b"hdlr", 0, 0, &b)
}

fn pitm() -> Vec<u8> {
    let mut b = Vec::new();
    push_u16(&mut b, 1); // primary item_ID
    full_bx(b"pitm", 0, 0, &b)
}

fn iinf() -> Vec<u8> {
    let mut infe = Vec::new();
    push_u16(&mut infe, 1); // item_ID
    push_u16(&mut infe, 0); // item_protection_index
    infe.extend_from_slice(ITEM_TYPE);
    infe.push(0); // item_name = ""
    let infe = full_bx(b"infe", 2, 0, &infe);

    let mut b = Vec::new();
    push_u16(&mut b, 1); // entry_count
    b.extend_from_slice(&infe);
    full_bx(b"iinf", 0, 0, &b)
}

fn ispe(width: u32, height: u32) -> Vec<u8> {
    let mut b = Vec::new();
    push_u32(&mut b, width);
    push_u32(&mut b, height);
    full_bx(b"ispe", 0, 0, &b)
}

fn pixi(cfg: Config) -> Vec<u8> {
    let n = cfg.subsampling.channels();
    let mut b = Vec::with_capacity(1 + n as usize);
    b.push(n);
    for _ in 0..n {
        b.push(cfg.bit_depth);
    }
    full_bx(b"pixi", 0, 0, &b)
}

fn ipma() -> Vec<u8> {
    let mut b = Vec::new();
    push_u32(&mut b, 1); // entry_count
    push_u16(&mut b, 1); // item_ID (version 0 → u16)
    b.push(3); // association_count
               // essential(1) << 7 | property_index(7); indices are 1-based into ipco.
    b.push(1); // ispe  (#1), not essential
    b.push(0x80 | 2); // av2C (#2), ESSENTIAL — a reader that cannot parse it must not proceed
    b.push(3); // pixi  (#3), not essential
    full_bx(b"ipma", 0, 0, &b)
}

/// `iloc` for a single item with a single extent. Returns the box and the
/// offset, within it, of the 4-byte extent-offset field to patch.
fn build_iloc(length: u32) -> (Vec<u8>, usize) {
    let mut body = Vec::new();
    body.push((4 << 4) | 4); // offset_size = 4, length_size = 4
    body.push(0); // base_offset_size = 0, reserved
    push_u16(&mut body, 1); // item_count
    push_u16(&mut body, 1); // item_ID
    push_u16(&mut body, 0); // data_reference_index
    push_u16(&mut body, 1); // extent_count
    let off_field = body.len();
    push_u32(&mut body, 0); // extent_offset — patched by `encode`
    push_u32(&mut body, length); // extent_length
    (full_bx(b"iloc", 0, 0, &body), 12 + off_field)
}

/// Read back the single extent written by [`build_iloc`].
fn parse_iloc(body: &[u8]) -> Option<(usize, usize)> {
    // body: version+flags(4), then the fields laid out by build_iloc.
    let sizes = *body.get(4)?;
    if sizes != ((4 << 4) | 4) || *body.get(5)? != 0 {
        return None; // a layout we did not write
    }
    if rd_u16(body, 6)? != 1 {
        return None; // more than one item
    }
    if rd_u16(body, 12)? != 1 {
        return None; // more than one extent
    }
    let off = rd_u32(body, 14)? as usize;
    let len = rd_u32(body, 18)? as usize;
    Some((off, len))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            width: 432,
            height: 240,
            config: Config::default(),
        }
    }

    #[test]
    fn round_trips_a_payload_exactly() {
        let payload: Vec<u8> = (0..5000u32).map(|i| (i % 251) as u8).collect();
        let file = encode(&params(), &payload).unwrap();
        let img = decode(&file).unwrap();
        assert_eq!(img.width, 432);
        assert_eq!(img.height, 240);
        assert_eq!(img.config, Config::default());
        assert_eq!(
            img.payload, &payload[..],
            "the coded payload must survive the container byte-for-byte"
        );
    }

    #[test]
    fn probes_its_own_output() {
        let file = encode(&params(), &[1, 2, 3, 4]).unwrap();
        assert_eq!(probe(&file), 100);
        assert_eq!(&file[4..8], b"ftyp");
        assert_eq!(&file[8..12], BRAND);
    }

    #[test]
    fn every_fourcc_comes_from_the_fourcc_module() {
        // The whole experimental surface is meant to be swappable in one file.
        // If a code is hard-coded elsewhere, this catches it.
        let file = encode(&params(), &[9; 64]).unwrap();
        let find_all = |needle: &[u8; 4]| {
            file.windows(4).filter(|w| *w == needle.as_slice()).count()
        };
        assert!(find_all(BRAND) >= 2, "brand appears as major + compatible");
        assert_eq!(find_all(ITEM_TYPE), 1, "item type appears once, in infe");
        assert_eq!(find_all(CONFIG_BOX), 1, "config box type appears once");
    }

    #[test]
    fn rejects_the_compact_still_picture_header() {
        let cfg = Config {
            full_still_picture_header: false,
            ..Config::default()
        };
        let p = Params {
            config: cfg,
            ..params()
        };
        assert!(matches!(encode(&p, &[1, 2, 3]), Err(Error::Unsupported(_))));
    }

    #[test]
    fn rejects_zero_dimensions_and_empty_payload() {
        let p = Params {
            width: 0,
            ..params()
        };
        assert!(matches!(encode(&p, &[1]), Err(Error::Invalid(_))));
        assert!(matches!(encode(&params(), &[]), Err(Error::Invalid(_))));
    }

    #[test]
    fn rejects_foreign_files() {
        assert!(matches!(decode(b"not a file at all"), Err(Error::NotAv2f)));
        // A real AVIF must not be mistaken for AV2F.
        let mut avif = vec![0, 0, 0, 32];
        avif.extend_from_slice(b"ftypavif");
        avif.extend_from_slice(&[0; 20]);
        assert!(matches!(decode(&avif), Err(Error::NotAv2f)));
    }

    #[test]
    fn truncation_never_panics() {
        let payload: Vec<u8> = (0..2000u32).map(|i| i as u8).collect();
        let file = encode(&params(), &payload).unwrap();
        for cut in 0..file.len() {
            // Every prefix must produce an error, not a panic.
            let _ = decode(&file[..cut]);
        }
    }

    #[test]
    fn corrupt_bytes_never_panic() {
        let payload: Vec<u8> = (0..1500u32).map(|i| i as u8).collect();
        let file = encode(&params(), &payload).unwrap();
        let mut rng = 0x2545F4914F6CDD1Du64;
        for _ in 0..4000 {
            let mut f = file.clone();
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            let at = (rng >> 33) as usize % f.len();
            f[at] ^= ((rng >> 11) & 0xFF) as u8;
            let _ = decode(&f);
        }
    }

    #[test]
    fn subsampling_and_depth_survive() {
        for ss in [
            Subsampling::Yuv420,
            Subsampling::Yuv422,
            Subsampling::Yuv444,
            Subsampling::Mono,
        ] {
            for bd in [8u8, 10, 12] {
                let cfg = Config {
                    bit_depth: bd,
                    subsampling: ss,
                    full_still_picture_header: true,
                };
                let p = Params {
                    config: cfg,
                    ..params()
                };
                let file = encode(&p, &[7; 32]).unwrap();
                assert_eq!(decode(&file).unwrap().config, cfg, "{ss:?} {bd}-bit");
            }
        }
    }
}
