//! backref.rs -- Token-level LZ77 backref pass for GN codon streams

const ESCAPE: u8 = 0x01;
const BACKREF_ESCAPE: u8 = 0x02;
const MIN_MATCH: usize = 6;
const MAX_MATCH: usize = 261;
const WINDOW_SIZE: usize = 32768;
const HASH_SIZE: usize = 65536;

#[inline]
fn encode_backref(out: &mut Vec<u8>, match_len: usize, offset: usize) {
    out.push(BACKREF_ESCAPE);
    out.push((match_len - 3) as u8);
    out.push((offset & 0xFF) as u8);
    out.push((offset >> 8) as u8);
}

#[inline]
fn is_boundary(data: &[u8], pos: usize) -> bool {
    if pos == 0 { return true; }
    data[pos - 1] != ESCAPE
}

#[inline]
fn hash4(data: &[u8], pos: usize) -> usize {
    if pos + 4 > data.len() { return 0; }
    let v = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
    ((v.wrapping_mul(2654435761)) >> 16) as usize & (HASH_SIZE - 1)
}

pub fn compress(input: &[u8]) -> Vec<u8> {
    let n = input.len();
    if n < MIN_MATCH { return input.to_vec(); }
    let mut out = Vec::with_capacity(n);
    let mut hash_table = vec![u32::MAX; HASH_SIZE];
    let mut pos = 0;
    while pos < n {
        if !is_boundary(input, pos) {
            out.push(input[pos]);
            pos += 1;
            continue;
        }
        if pos + 4 > n {
            out.push(input[pos]);
            pos += 1;
            continue;
        }
        let h = hash4(input, pos);
        let prev = hash_table[h] as usize;
        hash_table[h] = pos as u32;
        if prev != u32::MAX as usize
            && pos.saturating_sub(prev) <= WINDOW_SIZE
            && prev < pos
            && is_boundary(input, prev)
        {
            let offset = pos - prev;
            let max_len = (n - pos).min(MAX_MATCH);
            let mut match_len = 0;
            while match_len < max_len && input[prev + match_len] == input[pos + match_len] {
                match_len += 1;
                if match_len >= 2 && input[pos + match_len - 2] == ESCAPE {
                    match_len -= 1;
                    break;
                }
            }
            if match_len >= MIN_MATCH {
                encode_backref(&mut out, match_len, offset);
                pos += match_len;
            } else {
                out.push(input[pos]);
                pos += 1;
            }
        } else {
            out.push(input[pos]);
            pos += 1;
        }
    }
    out
}

pub fn decompress(input: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(input.len() * 2);
    let mut pos = 0;
    while pos < input.len() {
        if input[pos] == BACKREF_ESCAPE {
            if pos + 3 >= input.len() {
                out.extend_from_slice(&input[pos..]);
                break;
            }
            let match_len = input[pos + 1] as usize + 3;
            let offset = input[pos + 2] as usize | ((input[pos + 3] as usize) << 8);
            let start = out.len().saturating_sub(offset);
            for i in 0..match_len {
                let b = out[start + i];
                out.push(b);
            }
            pos += 4;
        } else {
            out.push(input[pos]);
            pos += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_roundtrip_no_match() {
        let data = b"hello world no repeats here at all";
        assert_eq!(decompress(&compress(data)), data);
    }
    #[test]
    fn test_roundtrip_with_match() {
        let data = b"the quick brown fox the quick brown fox the quick";
        let c = compress(data);
        assert!(c.len() < data.len());
        assert_eq!(decompress(&c), data);
    }
    #[test]
    fn test_escape_boundary() {
        let data: Vec<u8> = vec![0x01,0x55,b'h',b'e',b'l',b'l',b'o',0x01,0x55,b'h',b'e',b'l',b'l',b'o'];
        assert_eq!(decompress(&compress(&data)), data);
    }
}
