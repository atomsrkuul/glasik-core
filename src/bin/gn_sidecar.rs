//! GN Glasik Sidecar -- Rust binary replacing Python sidecar
//! Same stdin/stdout protocol as gn-glasik-sidecar.py v0.3
//! op: 0x01 = compress (SlidingV2 global window)
//! op: 0x02 = decompress (stateless)
//! op: 0x03 = gn_compress (per-call, no sliding)
//! op: 0x04 = window stats JSON
//! op: 0x05 = batch compress N chunks
//! op: 0x06 = pressurized compress (L3)
//! op: 0x07 = save snapshot
//! op: 0x08 = load snapshot

use std::io::{self, Read, Write};
use glasik_core::tokenizer::sliding_v2::SlidingTokenizerV2;
use glasik_core::pipeline;
use glasik_core::static_dict;

fn read_exact(reader: &mut impl Read, n: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; n];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn write_response(writer: &mut impl Write, data: &[u8]) -> io::Result<()> {
    let len = (data.len() as u32).to_le_bytes();
    writer.write_all(&len)?;
    writer.write_all(data)?;
    writer.flush()
}

fn window_stats_json(slider: &SlidingTokenizerV2, compress_calls: u64, total_input: u64, total_output: u64) -> Vec<u8> {
    let (entries, batches) = slider.stats();
    let ratio = if total_output > 0 { 
        (total_input as f64 / total_output as f64 * 1000.0) as u64 
    } else { 0 };
    format!(
        r#"{{"window_entries":{},"batches":{},"compress_calls":{},"total_input_kb":{},"total_output_kb":{},"ratio":{:.3}}}"#,
        entries, batches, compress_calls,
        total_input / 1024, total_output / 1024,
        if total_output > 0 { total_input as f64 / total_output as f64 } else { 0.0 }
    ).into_bytes()
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = io::BufReader::new(stdin.lock());
    let mut writer = io::BufWriter::new(stdout.lock());

    // Load static dict
    let static_entries = static_dict::load_static_dict();
    let mut slider = SlidingTokenizerV2::new_with_static(static_entries);

    // Auto-load snapshot if exists
    let snapshot_path = format!("{}/.openclaw/gn-window.snapshot", 
        std::env::var("HOME").unwrap_or_default());
    if std::path::Path::new(&snapshot_path).exists() {
        if let Ok(data) = std::fs::read_to_string(&snapshot_path) {
            if let Ok(d) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(entries) = d["entries"].as_array() {
                    let loaded: Vec<(Vec<u8>, u64, u64)> = entries.iter().filter_map(|e| {
                        let b = e["b"].as_array()?.iter().filter_map(|x| x.as_u64().map(|v| v as u8)).collect();
                        let f = e["f"].as_u64()?;
                        let s = e["s"].as_u64()?;
                        Some((b, f, s))
                    }).collect();
                    let n = loaded.len();
                    slider.import_dict(1, loaded);
                    eprintln!("GN-SIDECAR-RS: restored {} entries from snapshot", n);
                }
            }
        }
    }

    eprintln!("GN-GLASIK-SIDECAR-RS v0.3 ready (Rust binary, global sliding window)");

    let mut compress_calls: u64 = 0;
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;

    loop {
        // Read header: op(1) + len(4)
        let header = match read_exact(&mut reader, 5) {
            Ok(h) => h,
            Err(_) => break,
        };
        let op = header[0];
        let length = u32::from_le_bytes([header[1], header[2], header[3], header[4]]) as usize;
        let data = read_exact(&mut reader, length)?;

        let result: Vec<u8> = match op {
            0x01 => {
                // L2 compress via global sliding window + deflate
                let tokenized = slider.encode(&data);
                // Apply deflate on top of tokenized output
                use libdeflater::{Compressor, CompressionLvl};
                let mut comp = Compressor::new(CompressionLvl::default());
                let max = comp.deflate_compress_bound(tokenized.len());
                let mut deflated = vec![0u8; max];
                let compressed = match comp.deflate_compress(&tokenized, &mut deflated) {
                    Ok(n) => {
                        deflated.truncate(n);
                        if deflated.len() < tokenized.len() { deflated } else { tokenized }
                    }
                    Err(_) => tokenized
                };
                compress_calls += 1;
                total_input += data.len() as u64;
                total_output += compressed.len() as u64;
                compressed
            }
            0x02 => {
                // Decompress stateless
                match pipeline::decompress(&data) {
                    Ok(d) => d,
                    Err(_) => data.to_vec(),
                }
            }
            0x03 => {
                // Per-call gn_compress (no sliding state)
                pipeline::compress(&data)
            }
            0x04 => {
                // Window stats JSON
                window_stats_json(&slider, compress_calls, total_input, total_output)
            }
            0x05 => {
                // Batch compress N chunks with deflate
                use libdeflater::{Compressor, CompressionLvl};
                let mut pos = 0;
                let mut out = Vec::new();
                while pos + 4 <= data.len() {
                    let chunk_len = u32::from_le_bytes(
                        data[pos..pos+4].try_into().unwrap_or([0;4])
                    ) as usize;
                    pos += 4;
                    if pos + chunk_len > data.len() { break; }
                    let chunk = &data[pos..pos+chunk_len];
                    pos += chunk_len;
                    let tokenized = slider.encode(chunk);
                    let mut comp = Compressor::new(CompressionLvl::default());
                    let max = comp.deflate_compress_bound(tokenized.len());
                    let mut deflated = vec![0u8; max];
                    let compressed = match comp.deflate_compress(&tokenized, &mut deflated) {
                        Ok(n) => { deflated.truncate(n); if deflated.len() < tokenized.len() { deflated } else { tokenized } }
                        Err(_) => tokenized
                    };
                    compress_calls += 1;
                    total_input += chunk.len() as u64;
                    total_output += compressed.len() as u64;
                    out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
                    out.extend_from_slice(&compressed);
                }
                out
            }
            0x06 => {
                // L3 pressurized compress
                if data.is_empty() { vec![] } else {
                    let pk = data[0] as usize;
                    let mut pos = 1;
                    for _ in 0..pk {
                        if pos + 4 > data.len() { break; }
                        let wlen = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap_or([0;4])) as usize;
                        pos += 4;
                        if pos + wlen > data.len() { break; }
                        slider.encode(&data[pos..pos+wlen]);
                        pos += wlen;
                    }
                    if pos + 4 <= data.len() {
                        let tlen = u32::from_le_bytes(data[pos..pos+4].try_into().unwrap_or([0;4])) as usize;
                        pos += 4;
                        if pos + tlen <= data.len() {
                            let target = &data[pos..pos+tlen];
                            let compressed = slider.encode(target);
                            compress_calls += 1;
                            total_input += target.len() as u64;
                            total_output += compressed.len() as u64;
                            compressed
                        } else { vec![] }
                    } else { vec![] }
                }
            }
            0x07 => {
                // Save snapshot
                let path = String::from_utf8_lossy(&data).trim().to_string();
                let (_, entries) = slider.export_dict();
                match save_snapshot(&path, &entries) {
                    Ok(_) => b"ok".to_vec(),
                    Err(e) => e.to_string().into_bytes(),
                }
            }
            0x08 => {
                // Load snapshot
                let path = String::from_utf8_lossy(&data).trim().to_string();
                match load_snapshot(&path) {
                    Ok(entries) => {
                        let n = entries.len();
                        slider.import_dict(1, entries);
                        format!("loaded {} entries", n).into_bytes()
                    }
                    Err(e) => e.to_string().into_bytes(),
                }
            }
            _ => vec![],
        };

        write_response(&mut writer, &result)?;
    }

    // Save snapshot on exit
    let (_, entries) = slider.export_dict();
    if let Err(e) = save_snapshot(&snapshot_path, &entries) {
        eprintln!("GN-SIDECAR-RS: snapshot save failed: {}", e);
    } else {
        eprintln!("GN-SIDECAR-RS: saved {} entries to snapshot", entries.len());
    }

    Ok(())
}

fn save_snapshot(path: &str, entries: &[(Vec<u8>, u64, u64)]) -> Result<(), Box<dyn std::error::Error>> {
    let arr: Vec<serde_json::Value> = entries.iter().map(|(b, f, s)| {
        serde_json::json!({"b": b, "f": f, "s": s})
    }).collect();
    let json = serde_json::json!({"version": 1, "entries": arr});
    std::fs::write(path, serde_json::to_string(&json)?)?;
    Ok(())
}

fn load_snapshot(path: &str) -> Result<Vec<(Vec<u8>, u64, u64)>, Box<dyn std::error::Error>> {
    let data = std::fs::read_to_string(path)?;
    let d: serde_json::Value = serde_json::from_str(&data)?;
    let entries = d["entries"].as_array()
        .ok_or("no entries")?
        .iter()
        .filter_map(|e| {
            let b: Vec<u8> = e["b"].as_array()?.iter()
                .filter_map(|x| x.as_u64().map(|v| v as u8)).collect();
            let f = e["f"].as_u64()?;
            let s = e["s"].as_u64()?;
            Some((b, f, s))
        }).collect();
    Ok(entries)
}
