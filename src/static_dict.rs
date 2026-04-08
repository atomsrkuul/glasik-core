// static_dict.rs -- Load pre-trained static dictionary from binary format
//
// Binary format (gn_static_dict.bin):
//   magic:   b"GNSD" (4 bytes)
//   version: u32 LE (4 bytes)
//   count:   u32 LE (4 bytes)
//   entries: [blen(u8) + bytes + freq(u64 LE) + saving(u64 LE)] * count

use std::io::{Cursor, Read};

pub const STATIC_DICT_BYTES: &[u8] = include_bytes!("../scripts/gn_static_dict.bin");

pub fn load_static_dict() -> Vec<(Vec<u8>, u64, u64)> {
    parse_dict(STATIC_DICT_BYTES).unwrap_or_default()
}

pub fn parse_dict(data: &[u8]) -> Result<Vec<(Vec<u8>, u64, u64)>, String> {
    let mut cur = Cursor::new(data);

    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if &magic != b"GNSD" {
        return Err("bad magic".into());
    }

    let mut buf4 = [0u8; 4];
    cur.read_exact(&mut buf4).map_err(|e| e.to_string())?;
    let _version = u32::from_le_bytes(buf4);

    cur.read_exact(&mut buf4).map_err(|e| e.to_string())?;
    let count = u32::from_le_bytes(buf4) as usize;

    let mut entries = Vec::with_capacity(count);
    let mut buf8 = [0u8; 8];

    for _ in 0..count {
        let mut blen = [0u8; 1];
        cur.read_exact(&mut blen).map_err(|e| e.to_string())?;
        let mut bytes = vec![0u8; blen[0] as usize];
        cur.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        cur.read_exact(&mut buf8).map_err(|e| e.to_string())?;
        let freq = u64::from_le_bytes(buf8);
        cur.read_exact(&mut buf8).map_err(|e| e.to_string())?;
        let saving = u64::from_le_bytes(buf8);
        entries.push((bytes, freq, saving));
    }

    Ok(entries)
}
