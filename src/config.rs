//! The `av2C` configuration record.
//!
//! **Layout defined by this crate, not by any specification.** AVIF's `av1C` is
//! normative and bit-packed to match AV1's sequence header; there is no
//! published equivalent for AV2, so this record is deliberately plain: one
//! version byte, then one field per byte. It is easy to read in a hex dump and
//! trivial to replace wholesale when a real record is specified.
//!
//! It carries no information that is not already in the AV2 sequence header
//! inside the payload — it exists so a reader can describe the image without
//! decoding it, which is the job `av1C` does for AVIF.

use crate::boxes::{bx, push_u32};
use crate::fourcc::{CONFIG_BOX, CONFIG_VERSION};

/// Chroma subsampling of the coded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsampling {
    /// 4:2:0
    Yuv420,
    /// 4:2:2
    Yuv422,
    /// 4:4:4
    Yuv444,
    /// Monochrome (luma only)
    Mono,
}

impl Subsampling {
    fn to_byte(self) -> u8 {
        match self {
            Subsampling::Yuv420 => 0,
            Subsampling::Yuv422 => 1,
            Subsampling::Yuv444 => 2,
            Subsampling::Mono => 3,
        }
    }

    fn from_byte(b: u8) -> Option<Subsampling> {
        Some(match b {
            0 => Subsampling::Yuv420,
            1 => Subsampling::Yuv422,
            2 => Subsampling::Yuv444,
            3 => Subsampling::Mono,
            _ => return None,
        })
    }

    /// Channel count, for the `pixi` property.
    pub fn channels(self) -> u8 {
        match self {
            Subsampling::Mono => 1,
            _ => 3,
        }
    }
}

/// What the `av2C` box records about the coded image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub bit_depth: u8,
    pub subsampling: Subsampling,
    /// True when the payload uses AV2's **full** still-picture header
    /// (`single_picture_header_flag = 0`); false for the compact form.
    ///
    /// Informational — both forms encode and decode. (The historical
    /// full-only restriction was lifted with `rusty_av2d` 0.2.5.)
    pub full_still_picture_header: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            bit_depth: 8,
            subsampling: Subsampling::Yuv420,
            full_still_picture_header: true,
        }
    }
}

impl Config {
    /// Serialize as the `av2C` property box.
    pub fn to_box(self) -> Vec<u8> {
        let mut b = Vec::with_capacity(8);
        b.push(CONFIG_VERSION);
        b.push(self.bit_depth);
        b.push(self.subsampling.to_byte());
        b.push(self.full_still_picture_header as u8);
        push_u32(&mut b, 0); // reserved, must be zero
        bx(CONFIG_BOX, &b)
    }

    /// Parse an `av2C` box body. Returns `None` on any field this crate did not
    /// write, rather than guessing at a record it does not understand.
    pub fn from_body(body: &[u8]) -> Option<Config> {
        if body.len() < 8 || *body.first()? != CONFIG_VERSION {
            return None;
        }
        Some(Config {
            bit_depth: *body.get(1)?,
            subsampling: Subsampling::from_byte(*body.get(2)?)?,
            full_still_picture_header: *body.get(3)? != 0,
        })
    }
}
