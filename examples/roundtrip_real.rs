//! End-to-end: wrap a real AV2 still picture, read it back, and check the
//! payload is byte-identical. Run with the .ivf path as argv[1]; the IVF frame
//! payload is the AV2 bitstream.
//!
//!   cargo run --example roundtrip_real -- still_full.ivf

#[global_allocator]
static A: rusty_alloc_api::RustyAlloc = rusty_alloc_api::RustyAlloc;

use rusty_av2f::{decode, encode, Config, Params};

fn main() {
    let path = std::env::args().nth(1).expect("usage: roundtrip_real <file.ivf>");
    let ivf = std::fs::read(&path).expect("read ivf");
    assert_eq!(&ivf[0..4], b"DKIF", "not an IVF file");
    let hdr_len = u16::from_le_bytes([ivf[6], ivf[7]]) as usize;
    let width = u16::from_le_bytes([ivf[12], ivf[13]]) as u32;
    let height = u16::from_le_bytes([ivf[14], ivf[15]]) as u32;
    // first frame: [u32 size][u64 pts][payload]
    let size = u32::from_le_bytes([
        ivf[hdr_len], ivf[hdr_len + 1], ivf[hdr_len + 2], ivf[hdr_len + 3],
    ]) as usize;
    let start = hdr_len + 12;
    let payload = &ivf[start..start + size];

    let params = Params { width, height, config: Config::default() };
    let file = encode(&params, payload).expect("encode av2f");
    std::fs::write("out.av2f", &file).expect("write");

    let img = decode(&file).expect("decode av2f");
    assert_eq!((img.width, img.height), (width, height));
    assert_eq!(img.payload, payload, "payload must survive byte-for-byte");

    println!("av2 payload : {} bytes ({}x{})", payload.len(), width, height);
    println!("av2f file   : {} bytes (overhead {} bytes)", file.len(), file.len() - payload.len());
    println!("round-trip  : payload byte-identical");
    std::fs::write("out_payload.bin", img.payload).expect("write payload");
}
