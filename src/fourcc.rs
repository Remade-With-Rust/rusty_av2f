//! **The entire experimental surface of this format, in one place.**
//!
//! AV2F is not an AOM standard. AVIF is defined by a published AOM
//! specification that fixes its brand (`avif`), item type (`av01`) and
//! configuration box (`av1C`); **no equivalent document exists for AV2** — or
//! at least none that this crate's authors have seen. Every four-character code
//! below is therefore *chosen*, not *specified*.
//!
//! They are gathered here, and nowhere else, so that the day a real
//! specification appears the change is this file and nothing more. Nothing in
//! the reader or writer hard-codes a four-character code; they all read these
//! constants.
//!
//! Until then, files produced by this crate are readable by this crate and by
//! nothing else. See the crate README.

/// Major brand written into `ftyp`, and the brand a reader requires.
///
/// Mirrors AVIF's `avif`. Chosen, not specified.
pub const BRAND: &[u8; 4] = b"av2f";

/// Compatible brands written into `ftyp` after the major brand.
///
/// `mif1`/`miaf` are the generic HEIF/MIAF brands and *are* standard — a
/// conforming HEIF reader can at least walk the box structure, even though it
/// will not know the codec. `av2f` repeats the major brand, which is
/// conventional.
pub const COMPATIBLE_BRANDS: [&[u8; 4]; 3] = [b"av2f", b"mif1", b"miaf"];

/// `infe` item type for the coded image item.
///
/// Mirrors AVIF's `av01`. Chosen, not specified.
pub const ITEM_TYPE: &[u8; 4] = b"av02";

/// Codec configuration property box type.
///
/// Mirrors AVIF's `av1C`. Chosen, not specified — and unlike `av1C`, whose
/// layout is normative, the layout of this box is defined only by
/// [`crate::config`] in this crate.
pub const CONFIG_BOX: &[u8; 4] = b"av2C";

/// Recommended file extension.
pub const EXTENSION: &str = "av2f";

/// Marker byte written into the configuration record so a future reader can
/// tell files made by this crate apart from anything a real specification
/// later defines. Bumped only if the *layout* of our config record changes.
pub const CONFIG_VERSION: u8 = 1;
