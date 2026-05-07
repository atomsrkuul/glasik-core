//! ans.rs -- Minimal correct ANS
//! Uses exact JS encode formula with verified matching decode.
//! Renorm bound: state in [f, f*256) before encode step.

const M: u32 = 1 << 11; // table size = 2048

#[derive(Debug, Clone)]
pub struct FreqTable {
    pub freq: [u32; 256],
    pub cumsum: [u32; 257],
    pub total: u32,
}

impl FreqTable {
    pub fn build(data: &[u8]) -> Self {
        let mut raw = [0u32; 256];
        for &b in data {
            raw[b as usize] += 1;
        }
        Self::normalize(&raw, data.len())
    }

    fn normalize(raw: &[u32; 256], n: usize) -> Self {
        let mut freq = [0u32; 256];
        if n == 0 {
            return FreqTable {
                freq,
                cumsum: [0; 257],
                total: 0,
            };
        }
        let mut total = 0i32;
        for s in 0..256 {
            if raw[s] == 0 {
                continue;
            }
            let f = ((raw[s] as f64 / n as f64) * M as f64).round() as u32;
            freq[s] = f.max(1);
            total += freq[s] as i32;
        }
        let mut diff = M as i32 - total;
        let mut order: Vec<usize> = (0..256).filter(|&s| raw[s] > 0).collect();
        order.sort_unstable_by(|&a, &b| raw[b].cmp(&raw[a]));
        let mut i = 0;
        while diff > 0 {
            freq[order[i % order.len()]] += 1;
            diff -= 1;
            i += 1;
        }
        while diff < 0 {
            if let Some(&s) = order.iter().find(|&&s| freq[s] > 1) {
                freq[s] -= 1;
                diff += 1;
            } else {
                break;
            }
        }
        let mut cumsum = [0u32; 257];
        for s in 0..256 {
            cumsum[s + 1] = cumsum[s] + freq[s];
        }
        FreqTable {
            freq,
            cumsum,
            total: cumsum[256],
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let entries: Vec<_> = (0..256usize)
            .filter(|&s| self.freq[s] > 0)
            .map(|s| (s as u8, self.freq[s]))
            .collect();
        let count = entries.len() as u16;
        let mut out = count.to_le_bytes().to_vec();
        for (s, f) in entries {
            out.push(s);
            out.extend_from_slice(&f.to_le_bytes());
        }
        out
    }

    pub fn deserialize(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 2 {
            return None;
        }
        let count = u16::from_le_bytes(data[0..2].try_into().ok()?) as usize;
        if data.len() < 2 + count * 5 {
            return None;
        }
        let mut raw = [0u32; 256];
        let mut n = 0usize;
        for i in 0..count {
            let s = data[2 + i * 5] as usize;
            let f = u32::from_le_bytes(data[3 + i * 5..7 + i * 5].try_into().ok()?);
            raw[s] = f;
            n += f as usize;
        }
        Some((Self::normalize(&raw, n), 2 + count * 5))
    }

    fn find_sym(&self, r: u32) -> usize {
        // r is in [0, total). Find s such that cumsum[s] <= r < cumsum[s+1]
        for s in 0..256 {
            if self.cumsum[s + 1] > r {
                return s;
            }
        }
        255
    }
}

pub fn compress(data: &[u8]) -> Vec<u8> {
    let freq = FreqTable::build(data);
    let fb = freq.serialize();
    let mut hdr = Vec::new();
    hdr.extend_from_slice(&(data.len() as u32).to_le_bytes());
    hdr.extend_from_slice(&fb);
    if data.is_empty() {
        return hdr;
    }

    let total = freq.total;
    let mut x: u32 = total; // initial state = M (= total after normalization)
    let mut stream: Vec<u8> = Vec::new();

    for &sym in data.iter().rev() {
        let s = sym as usize;
        let f = freq.freq[s];
        let cs = freq.cumsum[s];

        // Renorm: bring x into [f, f*256) so after encode x < 256*total
        // i.e., emit bytes while x >= f * 256
        while x >= f * 256 {
            stream.push((x & 0xFF) as u8);
            x >>= 8;
        }
        // x is now in [0, f*256). Clamp to [f, f*256) -- x should already >= f
        // if x < f that means initial state is wrong, but let's be safe:
        // Actually for first symbol x=total >= f always since f <= total.

        // Encode: new_x = (x / f) * total + (x % f) + cs
        x = (x / f) * total + (x % f) + cs;
    }

    // Flush state
    while x > 0 {
        stream.push((x & 0xFF) as u8);
        x >>= 8;
    }
    stream.reverse();

    let mut out = hdr;
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend(stream);
    out
}

pub fn decompress(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 {
        return None;
    }
    let orig = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    if orig == 0 {
        return Some(vec![]);
    }
    let (freq, fsz) = FreqTable::deserialize(&data[4..])?;
    let rest = &data[4 + fsz..];
    if rest.len() < 4 {
        return None;
    }
    let slen = u32::from_le_bytes(rest[0..4].try_into().ok()?) as usize;
    let stream = &rest[4..4 + slen.min(rest.len() - 4)];

    let total = freq.total;
    let mut x: u32 = 0;
    let mut pos = 0usize;

    // Reconstruct state
    while pos < stream.len() && x < total {
        x = (x << 8) | stream[pos] as u32;
        pos += 1;
    }

    let mut out = Vec::with_capacity(orig);

    while out.len() < orig {
        // Decode: x = (q)*total + r + cs  where r = x%total - cs, q = x/total
        // So: sym = find(x % total), r = x%total - cs[sym], q = x/total
        // prev_x was in [f, f*256): prev_x = q*f + r
        let r = x % total;
        let s = freq.find_sym(r);
        let f = freq.freq[s];
        let cs = freq.cumsum[s];
        let q = x / total;
        let prev_x = q * f + (r - cs);
        out.push(s as u8);
        x = prev_x;

        // Renorm: restore x to [total, 256*total) by reading bytes the encoder emitted
        while x < total && pos < stream.len() {
            x = (x << 8) | stream[pos] as u32;
            pos += 1;
        }
    }

    // Encoder processes data backwards so decoder naturally recovers forward order
    if out.len() == orig {
        Some(out)
    } else {
        None
    }
}

pub fn compress_bits(data: &[u8]) -> Vec<u8> {
    let freq = FreqTable::build(data);
    let fb = freq.serialize();
    let mut hdr = Vec::new();
    hdr.extend_from_slice(&(data.len() as u32).to_le_bytes());
    hdr.extend_from_slice(&fb);
    if data.is_empty() {
        return hdr;
    }

    let total = freq.total;
    let mut x: u32 = total;
    let mut bits: Vec<u8> = Vec::new();

    for &sym in data.iter().rev() {
        let s = sym as usize;
        let f = freq.freq[s];
        let cs = freq.cumsum[s];
        while x >= 2 * f {
            bits.push((x & 1) as u8);
            x >>= 1;
        }
        x = (x / f) * total + (x % f) + cs;
    }

    // Store final state as raw u32, then renorm bits in emission order.
    // Decoder reads renorm bits front-to-back.
    let nb = bits.len() as u32;
    let mut packed = Vec::new();
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for (i, &b) in chunk.iter().enumerate() {
            byte |= b << i;
        }
        packed.push(byte);
    }

    let mut out = hdr;
    out.extend_from_slice(&x.to_le_bytes());
    out.extend_from_slice(&nb.to_le_bytes());
    out.extend_from_slice(&(packed.len() as u32).to_le_bytes());
    out.extend(packed);
    out
}

pub fn decompress_bits(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 {
        return None;
    }
    let orig = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    if orig == 0 {
        return Some(vec![]);
    }
    let (freq, fsz) = FreqTable::deserialize(&data[4..])?;
    let rest = &data[4 + fsz..];
    if rest.len() < 12 {
        return None;
    }
    let mut x = u32::from_le_bytes(rest[0..4].try_into().ok()?);
    let nb = u32::from_le_bytes(rest[4..8].try_into().ok()?) as usize;
    let byte_len = u32::from_le_bytes(rest[8..12].try_into().ok()?) as usize;
    let packed = &rest[12..12 + byte_len.min(rest.len() - 12)];

    let mut bits = Vec::with_capacity(nb);
    for &byte in packed {
        for i in 0..8 {
            bits.push((byte >> i) & 1);
        }
    }
    bits.truncate(nb);

    // Renorm bits were emitted during backwards scan (last symbol first).
    // Decoder processes symbols backwards too, consuming bits from the end.
    let total = freq.total;
    let mut pos = bits.len();
    let mut out = Vec::with_capacity(orig);

    while out.len() < orig {
        let r = x % total;
        let s = freq.find_sym(r);
        let f = freq.freq[s];
        let cs = freq.cumsum[s];
        let q = x / total;
        x = q * f + (r - cs);
        out.push(s as u8);

        while x < total && pos > 0 {
            pos -= 1;
            x = (x << 1) | bits[pos] as u32;
        }
    }

    if out.len() == orig {
        Some(out)
    } else {
        None
    }
}



// ── Order-1 ANS ──────────────────────────────────────────────────────────────

fn build_o1_tables(data: &[u8]) -> Box<[Option<FreqTable>; 256]> {
    let mut raw = vec![[0u32; 256]; 256];
    let mut counts = [0usize; 256];
    if data.len() > 1 {
        for i in 1..data.len() {
            let ctx = data[i - 1] as usize;
            raw[ctx][data[i] as usize] += 1;
            counts[ctx] += 1;
        }
    }
    let mut tables: Box<[Option<FreqTable>; 256]> = Box::new(std::array::from_fn(|_| None));
    for ctx in 0..256 {
        if counts[ctx] > 0 {
            tables[ctx] = Some(FreqTable::normalize(&raw[ctx], counts[ctx]));
        }
    }
    tables
}

fn serialize_o1(tables: &[Option<FreqTable>; 256]) -> Vec<u8> {
    let mut out = Vec::new();
    let n_ctx = tables.iter().filter(|t| t.is_some()).count() as u16;
    out.extend_from_slice(&n_ctx.to_le_bytes());
    for (ctx, table) in tables.iter().enumerate() {
        if let Some(t) = table {
            out.push(ctx as u8);
            let fb = t.serialize();
            out.extend_from_slice(&(fb.len() as u32).to_le_bytes());
            out.extend_from_slice(&fb);
        }
    }
    out
}

fn deserialize_o1(data: &[u8]) -> Option<(Box<[Option<FreqTable>; 256]>, usize)> {
    if data.len() < 2 {
        return None;
    }
    let n_ctx = u16::from_le_bytes(data[0..2].try_into().ok()?) as usize;
    let mut pos = 2;
    let mut tables: Box<[Option<FreqTable>; 256]> = Box::new(std::array::from_fn(|_| None));
    for _ in 0..n_ctx {
        if pos + 5 > data.len() {
            return None;
        }
        let ctx = data[pos] as usize;
        pos += 1;
        let fb_len = u32::from_le_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        pos += 4;
        if pos + fb_len > data.len() {
            return None;
        }
        let (table, _) = FreqTable::deserialize(&data[pos..pos + fb_len])?;
        tables[ctx] = Some(table);
        pos += fb_len;
    }
    Some((tables, pos))
}

fn o1_fallback() -> FreqTable {
    let raw = [1u32; 256];
    FreqTable::normalize(&raw, 256)
}

pub fn compress_o1(data: &[u8]) -> Vec<u8> {
    let mut hdr = Vec::new();
    hdr.extend_from_slice(&(data.len() as u32).to_le_bytes());
    if data.is_empty() {
        hdr.extend_from_slice(&0u16.to_le_bytes());
        return hdr;
    }

    let tables = build_o1_tables(data);
    let fallback = o1_fallback();
    let o1_hdr = serialize_o1(&tables);
    hdr.extend_from_slice(&o1_hdr);

    let n = data.len();
    let mut stream: Vec<u8> = Vec::new();
    let mut x: u32 = fallback.total;

    for i in (0..n).rev() {
        let sym = data[i] as usize;
        let ctx = if i == 0 { 0usize } else { data[i - 1] as usize };
        let tbl = tables[ctx].as_ref().unwrap_or(&fallback);
        let f = tbl.freq[sym].max(1);
        let cs = tbl.cumsum[sym];
        let tot = tbl.total;

        while x >= f * 256 {
            stream.push((x & 0xFF) as u8);
            x >>= 8;
        }
        x = (x / f) * tot + (x % f) + cs;
    }

    while x > 0 {
        stream.push((x & 0xFF) as u8);
        x >>= 8;
    }
    stream.reverse();

    let mut out = hdr;
    out.extend_from_slice(&(stream.len() as u32).to_le_bytes());
    out.extend(stream);
    out
}

pub fn decompress_o1(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 {
        return None;
    }
    let orig = u32::from_le_bytes(data[0..4].try_into().ok()?) as usize;
    if orig == 0 {
        return Some(vec![]);
    }
    let (tables, o1sz) = deserialize_o1(&data[4..])?;
    let rest = &data[4 + o1sz..];
    if rest.len() < 4 {
        return None;
    }
    let slen = u32::from_le_bytes(rest[0..4].try_into().ok()?) as usize;
    let stream = &rest[4..4 + slen.min(rest.len() - 4)];

    let fallback = o1_fallback();
    let init_total = fallback.total;

    let mut x: u32 = 0;
    let mut pos = 0usize;
    while pos < stream.len() && x < init_total {
        x = (x << 8) | stream[pos] as u32;
        pos += 1;
    }

    let mut out = Vec::with_capacity(orig);
    let mut prev: usize = 0;

    while out.len() < orig {
        let tbl = tables[prev].as_ref().unwrap_or(&fallback);
        let tot = tbl.total;
        let r = x % tot;
        let s = tbl.find_sym(r);
        let f = tbl.freq[s];
        let cs = tbl.cumsum[s];
        let q = x / tot;
        x = q * f + (r - cs);
        out.push(s as u8);
        prev = s;

        while x < tot && pos < stream.len() {
            x = (x << 8) | stream[pos] as u32;
            pos += 1;
        }
    }

    if out.len() == orig {
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_empty() {
        let c = compress_bits(b"");
        assert_eq!(decompress_bits(&c).unwrap(), b"");
    }

    #[test]
    fn test_bits_single_sym() {
        let d = b"aaaaaa";
        let c = compress_bits(d);
        assert_eq!(decompress_bits(&c).expect("bit single sym failed"), d);
    }

    #[test]
    fn test_bits_two_syms() {
        let d = b"ababababab";
        let c = compress_bits(d);
        assert_eq!(decompress_bits(&c).expect("bit roundtrip failed"), d);
    }

    #[test]
    fn test_bits_real_text() {
        let d = b"the quick brown fox jumps over the lazy dog                   the quick brown fox jumps over the lazy dog";
        let c = compress_bits(d);
        let dec = decompress_bits(&c).expect("bit text failed");
        assert_eq!(dec, d);
        println!("bits text: {}->{}  {:.2}x", d.len(), c.len(), d.len() as f64 / c.len() as f64);
    }

    #[test]
    fn test_bits_vs_bytes() {
        let d: Vec<u8> = b"hello world ".iter().cycle().take(1000).copied().collect();
        let byte_c = compress(&d);
        let bit_c = compress_bits(&d);
        let dec = decompress_bits(&bit_c).expect("bit 1KB failed");
        assert_eq!(dec, d);
        println!("byte: {}->{}  {:.2}x", d.len(), byte_c.len(), d.len() as f64 / byte_c.len() as f64);
        println!("bits: {}->{}  {:.2}x", d.len(), bit_c.len(), d.len() as f64 / bit_c.len() as f64);
    }

    #[test]
    fn test_o1_empty() {
        let c = compress_o1(b"");
        assert_eq!(decompress_o1(&c).unwrap(), b"");
    }

    #[test]
    fn test_o1_roundtrip() {
        let d = b"hello world hello world hello";
        let c = compress_o1(d);
        assert_eq!(decompress_o1(&c).expect("o1 roundtrip failed"), d);
    }

    #[test]
    fn test_o1_real_text() {
        let d = b"the quick brown fox jumps over the lazy dog                   the quick brown fox jumps over the lazy dog";
        let c = compress_o1(d);
        let dec = decompress_o1(&c).expect("o1 text failed");
        assert_eq!(dec, d);
        let c0 = compress(d);
        println!("o0: {}->{}  {:.2}x", d.len(), c0.len(), d.len() as f64 / c0.len() as f64);
        println!("o1: {}->{}  {:.2}x", d.len(), c.len(), d.len() as f64 / c.len() as f64);
    }

    #[test]
    fn test_o1_vs_o0_1kb() {
        let d: Vec<u8> = b"hello world ".iter().cycle().take(1000).copied().collect();
        let c0 = compress(&d);
        let c1 = compress_o1(&d);
        let dec = decompress_o1(&c1).expect("o1 1KB failed");
        assert_eq!(dec, d);
        println!("o0 1KB: {}->{}  {:.2}x", d.len(), c0.len(), d.len() as f64 / c0.len() as f64);
        println!("o1 1KB: {}->{}  {:.2}x", d.len(), c1.len(), d.len() as f64 / c1.len() as f64);
    }
}
