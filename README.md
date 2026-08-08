# rusty_av2f

A still-image container for **AV2**, in the shape AVIF uses for AV1: one coded
picture stored as an item in an ISOBMFF/HEIF file. Pure Rust, no `unsafe`, no
dependencies.

[![crates.io](https://img.shields.io/crates/v/rusty_av2f.svg)](https://crates.io/crates/rusty_av2f)
[![docs.rs](https://docs.rs/rusty_av2f/badge.svg)](https://docs.rs/rusty_av2f)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> **⚠️ Not an AOM standard, and not interoperable.** AV2F's four-character codes
> are *chosen by this crate*, not specified by anyone. Files it writes are
> readable by it and by nothing else. Read
> [the next section](#read-this-before-you-use-it) before producing files you
> intend to keep.

```rust
use rusty_av2f::{encode, decode, Params, Config};

// `payload` is an AV2 still-picture bitstream (e.g. an IVF frame's payload).
let params = Params { width: 432, height: 240, config: Config::default() };
let file = encode(&params, payload)?;

let img = decode(&file)?;
assert_eq!(img.payload, payload);       // the container is lossless
```

---

## Read this before you use it

**AV2F is not a standard.** AVIF exists because AOM published a specification
that fixes its brand (`avif`), its item type (`av01`) and its configuration
record (`av1C`). **There is no equivalent document for AV2** — none that we have
found, at any rate. Every four-character code this crate writes is therefore
*chosen*, not *specified*:

| Role | AVIF (normative) | AV2F (ours, provisional) |
|---|---|---|
| Major brand | `avif` | `av2f` |
| Item type | `av01` | `av02` |
| Config box | `av1C` | `av2C` |
| Extension | `.avif` | `.av2f` |

They all live in [`src/fourcc.rs`](src/fourcc.rs) and nowhere else, so adopting a
real specification later is an edit to one file. Nothing in the reader or writer
hard-codes a code; they read those constants.

Consequences, stated plainly:

- **Files written here are readable here and by nothing else.** No browser, no
  phone, no image viewer will open them. A conforming HEIF reader can walk the
  box structure (we write the standard `mif1`/`miaf` compatible brands) but will
  not recognize the codec.
- **The format may change without a compatibility story.** If AOM publishes
  something different, this crate will follow it, and files written by earlier
  versions will stop being readable. There is no migration promise.
- **The `av2C` record's layout is ours too** — deliberately plain (a version
  byte, then one field per byte) rather than bit-packed, because it exists to be
  replaced. Unlike `av1C`, it is not a mirror of the codec's sequence header.

What it is genuinely good for: pipeline plumbing, storing AV2 stills next to AVIF
stills for size comparisons, and having the container work already done when a
specification does arrive.

## Payload restriction: full still-picture header only

AV2 can code a still picture two ways: with the ordinary frame headers, or with
the compact form signalled by `single_picture_header_flag` (AV2's rename of AV1's
`reduced_still_picture_header`). The compact form is the natural choice for an
image format — it is what AVIF uses.

**This crate writes and reads only the full-header form**, and [`encode`] refuses
a `Config` that claims otherwise. The reason is specific and temporary:
[`rusty_av2d`](https://crates.io/crates/rusty_av2d), the decoder these files are
meant for, does not yet parse the compact header bit-exactly and refuses it
outright. Writing the compact form would produce files our own decoder rejects,
so the restriction is enforced at the writer rather than discovered at the reader.
It lifts when that parse lands.

Produce a suitable payload with:

```
avmenc --full-still-picture-hdr --limit=1 ... -o still.ivf
```

and hand `encode` the IVF frame's payload.

## What's in the file

The minimum HEIF structure for a single image item — the same skeleton AVIF uses,
so the layout is familiar in a hex dump:

```
ftyp   major brand av2f, compatible av2f/mif1/miaf
meta
  hdlr   'pict'
  pitm   item 1
  iinf   one infe, item type av02
  iprp
    ipco   ispe (dimensions), pixi (bit depth per channel), av2C (config)
    ipma   binds all three to item 1
  iloc   offset + length of the payload in mdat
mdat   the AV2 bitstream, byte for byte
```

## Robustness

The parser is written to fail rather than guess, and is tested that way:

- Box sizes of 0 ("to end of file") and 1 ("64-bit size follows") are **rejected**
  — neither appears in what this crate writes, and accepting them means trusting a
  length the parser cannot bound.
- `truncation_never_panics` feeds every prefix of a valid file.
- `corrupt_bytes_never_panic` feeds several thousand single-byte mutations.
- All arithmetic on parsed lengths is checked; a bounds test written as
  `off + len > buf.len()` is itself an overflow bug, so the code uses
  `checked_add`.

## Status

**Experimental — 0.x, and the format is provisional.** Published so it can be
depended on normally, not because the codes are settled. Treat a version bump as
potentially format-breaking until an AOM specification exists; if one appears and
says something different, this crate follows it and older files stop being
readable.

```toml
[dependencies]
rusty_av2f = "0.1"
```

## Related

- [`rusty_av2d`](https://crates.io/crates/rusty_av2d) — the pure-Rust AV2 decoder
  these files feed, byte-identical to AOM's `avmdec` across its conformance corpus.
- `rff-format-av2f` in
  [remade_ffmpeg_rs](https://github.com/Remade-With-Rust/remade_ffmpeg_rs) — the
  demuxer/muxer that plugs this crate into that engine.

## License

MIT. (Original code — unlike `rusty_av2d`, which is BSD-2-Clause because it
carries a fork lineage.)
