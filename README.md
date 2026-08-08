# rusty_av2f

[![Remade With Rust](https://img.shields.io/badge/Remade%20With-Rust-000?logo=rust&logoColor=fff)](https://github.com/remade-with-rust)
[![By Mata Network](https://img.shields.io/badge/by-Mata%20Network-5b2be0)](https://www.mata.network)
[![crates.io](https://img.shields.io/crates/v/rusty_av2f.svg)](https://crates.io/crates/rusty_av2f)
[![docs.rs](https://docs.rs/rusty_av2f/badge.svg)](https://docs.rs/rusty_av2f)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue)](LICENSE)

A pure-Rust **still-image container for AV2** — one coded picture stored as an
item in an ISOBMFF/HEIF file, in the shape AVIF uses for AV1. Zero dependencies,
no C, no FFI, no `unsafe`.

- **Writer** — emits the minimum HEIF structure for a single image item
  (`ftyp` / `meta` / `iloc` / `ipco` with `ispe`+`pixi`+`av2C` / `mdat`), 250
  bytes of overhead over the coded payload.
- **Reader** — parses that structure back and hands you the AV2 bitstream
  **byte for byte**; the container is lossless by construction.
- **Fails rather than guesses** — unbounded box sizes are rejected, every parsed
  length is `checked_add`, and the tests feed it every truncation prefix plus
  several thousand byte mutations.
- **100% safe Rust**, and the whole crate is four small modules — the format
  surface is deliberately small enough to audit in one sitting.

> ### ⚠️ Not an AOM standard, and not interoperable
>
> AV2F's four-character codes are **chosen by this crate**, not specified by
> anyone. Files it writes are readable by it and by nothing else. Read
> [the next section](#read-this-before-you-use-it) before producing files you
> intend to keep.

---

## Read this before you use it

AVIF exists because AOM published a specification that fixes its brand (`avif`),
its item type (`av01`) and its configuration record (`av1C`). **There is no
equivalent document for AV2** — none that we have found. Every four-character
code below is therefore *chosen*, not *specified*:

| Role | AVIF (normative) | AV2F (ours, provisional) |
|---|---|---|
| Major brand | `avif` | `av2f` |
| Item type | `av01` | `av02` |
| Config box | `av1C` | `av2C` |
| Extension | `.avif` | `.av2f` |

They live in [`src/fourcc.rs`](src/fourcc.rs) and nowhere else — a unit test
(`every_fourcc_comes_from_the_fourcc_module`) enforces that nothing in the reader
or writer hard-codes one. Adopting a real specification later is an edit to one
file.

Consequences, stated plainly:

- **No browser, phone, or image viewer will open these files.** A conforming
  HEIF reader can walk the box structure — we write the standard `mif1`/`miaf`
  compatible brands — but will not recognise the codec.
- **The format may change without a compatibility story.** If AOM publishes
  something different, this crate follows it and files written by earlier
  versions stop being readable. There is no migration promise.
- **The `av2C` record's layout is ours too** — deliberately plain (a version
  byte, then one field per byte) rather than bit-packed, because it exists to be
  replaced. Unlike `av1C`, it is not a mirror of the codec's sequence header.

What it is genuinely good for: pipeline plumbing, storing AV2 stills next to AVIF
stills for size comparisons, and having the container work already done when a
specification does arrive.

## Usage

```rust
use rusty_av2f::{encode, decode, Params, Config};

// `payload` is an AV2 still-picture bitstream — e.g. an IVF frame's payload.
let params = Params { width: 432, height: 240, config: Config::default() };
let file = encode(&params, payload)?;

let img = decode(&file)?;
assert_eq!(img.payload, payload);   // the container is lossless
assert_eq!((img.width, img.height), (432, 240));
```

Sniff a file without parsing it, in the shape format probes conventionally use
(0–100):

```rust
if rusty_av2f::probe(&bytes) == 100 { /* it's ours */ }
```

## Payload restriction: full still-picture header only

AV2 can code a still picture two ways: with the ordinary frame headers, or with
the compact form signalled by `single_picture_header_flag` (AV2's rename of AV1's
`reduced_still_picture_header`). The compact form is the natural choice for an
image format — it is what AVIF uses.

**This crate writes and reads only the full-header form**, and `encode` refuses a
`Config` that claims otherwise. The reason is specific and temporary:
[`rusty_av2d`](https://crates.io/crates/rusty_av2d), the decoder these files are
meant for, does not yet parse the compact header bit-exactly and refuses it
outright. Writing the compact form would mint files our own decoder rejects, so
the restriction lives at the writer rather than being discovered at the reader.
It lifts when that parse lands.

Produce a suitable payload with AOM's reference encoder:

```sh
avmenc --codec=av2 --limit=1 --ivf --full-still-picture-hdr \
       --end-usage=q --qp=140 -o still.ivf source.y4m
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

A real 320×480 photograph encoded at qp 140 lands at **6,980 bytes**, of which
250 are container overhead. (The same picture as lossless PNG is 285,948 bytes —
not a like-for-like comparison, since one is lossy and one is not, but it is what
a real file looks like.)

## Robustness

The parser is written to fail rather than guess, and is tested that way:

- Box sizes of 0 ("to end of file") and 1 ("64-bit size follows") are
  **rejected** — neither appears in what this crate writes, and accepting them
  would mean trusting a length the parser cannot bound.
- `truncation_never_panics` feeds every prefix of a valid file.
- `corrupt_bytes_never_panic` feeds several thousand single-byte mutations.
- All arithmetic on parsed lengths is checked. A bounds test written as
  `off + len > buf.len()` is itself an overflow bug, so the code uses
  `checked_add`.

## Status

**Experimental — 0.x, and the format is provisional.** Published so it can be
depended on normally, not because the codes are settled. Treat any version bump
as potentially format-breaking until an AOM specification exists.

Verified end to end: a real AV2 still round-trips byte-identically through the
container, and the decoded pixels are **byte-identical to AOM's `avmdec`**.
Note that the container is only as useful as the decoder behind it — `rusty_av2d`
is a research preview with known gaps, so validate your own content rather than
assuming coverage.

## Part of Remade With Rust

`rusty_av2f` is the still-image container of
**[remade_ffmpeg_rs](https://github.com/Remade-With-Rust/remade_ffmpeg_rs)** — a
ground-up, permissively-licensed Rust rebuild of FFmpeg, where it is wired in as
the `av2f` format. Sister project:
**[FFAI](https://github.com/Remade-With-Rust/FFAI)** — media for an AI-first
world. More at **[github.com/remade-with-rust](https://github.com/remade-with-rust)**.

Closest sibling: [`rusty_av2d`](https://crates.io/crates/rusty_av2d), the pure-Rust
AV2 decoder that reads what this crate wraps. Other codec crates:
[`rusty_h264`](https://crates.io/crates/rusty_h264),
[`rusty_vp9`](https://crates.io/crates/rusty_vp9),
[`rusty_aac`](https://crates.io/crates/rusty_aac),
[`rusty_mp3`](https://crates.io/crates/rusty_mp3), and the
[rusty-av1-toolkit](https://github.com/Remade-With-Rust/rusty-av1-toolkit) forks.

## About Mata Network

<!-- ORG BOILERPLATE — keep identical across repos -->

[Mata Network](https://www.mata.network) builds sovereign, self-hostable
infrastructure. **Remade With Rust** is our open-source home for the
permissively-licensed building blocks that work depends on.

<!-- /ORG BOILERPLATE -->

## License

MIT. See [LICENSE](LICENSE). (Original code — unlike `rusty_av2d`, which is
BSD-2-Clause because it carries a fork lineage.)
