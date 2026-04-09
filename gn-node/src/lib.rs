#![deny(clippy::all)]
#![allow(clippy::unused_unit)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use glasik_core::tokenizer::sliding_v2::SlidingTokenizerV2;
use glasik_core::pipeline;
use glasik_core::static_dict;
use std::sync::OnceLock;
use tokio::sync::{mpsc, oneshot};

enum Job {
    CompressL2 { data: Vec<u8>, resp: oneshot::Sender<Vec<u8>> },
    CompressPressurized { target: Vec<u8>, warm: Vec<Vec<u8>>, pk: usize, resp: oneshot::Sender<Vec<u8>> },
    WindowStats { resp: oneshot::Sender<String> },
    SaveSnapshot { path: String, resp: oneshot::Sender<String> },
    LoadSnapshot { path: String, resp: oneshot::Sender<String> },
}

static WORKER: OnceLock<mpsc::Sender<Job>> = OnceLock::new();

fn deflate_buf(tokenized: Vec<u8>) -> Vec<u8> {
    let mut comp = libdeflater::Compressor::new(libdeflater::CompressionLvl::default());
    let max = comp.deflate_compress_bound(tokenized.len());
    let mut out = vec![0u8; max];
    match comp.deflate_compress(&tokenized, &mut out) {
        Ok(n) => { out.truncate(n); if out.len() < tokenized.len() { out } else { tokenized } }
        Err(_) => tokenized
    }
}

fn get_worker() -> &'static mpsc::Sender<Job> {
    WORKER.get_or_init(|| {
        let (tx, mut rx) = mpsc::channel::<Job>(256);
        tokio::spawn(async move {
            let entries = static_dict::load_static_dict();
            let mut slider = SlidingTokenizerV2::new_with_static(entries);
            // Auto-load snapshot
            let snap = format!("{}/.openclaw/gn-window.snapshot",
                std::env::var("HOME").unwrap_or_default());
            if let Ok(data) = std::fs::read_to_string(&snap) {
                if let Ok(d) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(arr) = d["entries"].as_array() {
                        let loaded: Vec<(Vec<u8>, u64, u64)> = arr.iter().filter_map(|e| {
                            let b: Vec<u8> = e["b"].as_array()?.iter()
                                .filter_map(|x| x.as_u64().map(|v| v as u8)).collect();
                            Some((b, e["f"].as_u64()?, e["s"].as_u64()?))
                        }).collect();
                        let n = loaded.len();
                        slider.import_dict(1, loaded);
                        eprintln!("GN-NATIVE: restored {} entries", n);
                    }
                }
            }
            while let Some(job) = rx.recv().await {
                match job {
                    Job::CompressL2 { data, resp } => {
                        let t = slider.encode(&data);
                        let _ = resp.send(deflate_buf(t));
                    }
                    Job::CompressPressurized { target, warm, pk, resp } => {
                        let start = warm.len().saturating_sub(pk);
                        for w in &warm[start..] { slider.encode(w); }
                        let t = slider.encode(&target);
                        let _ = resp.send(deflate_buf(t));
                    }
                    Job::WindowStats { resp } => {
                        let (e, b) = slider.stats();
                        let _ = resp.send(format!(r#"{{"window_entries":{},"batches":{}}}"#, e, b));
                    }
                    Job::SaveSnapshot { path, resp } => {
                        let msg = match save_snap(&slider, &path) {
                            Ok(_) => "ok".to_string(),
                            Err(e) => format!("error: {}", e),
                        };
                        let _ = resp.send(msg);
                    }
                    Job::LoadSnapshot { path, resp } => {
                        let msg = match load_snap(&mut slider, &path) {
                            Ok(n) => format!("loaded {} entries", n),
                            Err(e) => format!("error: {}", e),
                        };
                        let _ = resp.send(msg);
                    }
                }
            }
        });
        tx
    })
}

fn save_snap(slider: &SlidingTokenizerV2, path: &str) -> std::result::Result<(), String> {
    let (_, entries) = slider.export_dict();
    let arr: Vec<serde_json::Value> = entries.iter()
        .map(|(b,f,s)| serde_json::json!({"b":b,"f":f,"s":s})).collect();
    let json = serde_json::json!({"version":1,"entries":arr});
    serde_json::to_string(&json).map_err(|e| e.to_string())
        .and_then(|s| std::fs::write(path, s).map_err(|e| e.to_string()))
}

fn load_snap(slider: &mut SlidingTokenizerV2, path: &str) -> std::result::Result<usize, String> {
    let data = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let d: serde_json::Value = serde_json::from_str(&data).map_err(|e| e.to_string())?;
    let entries: Vec<(Vec<u8>, u64, u64)> = d["entries"].as_array()
        .ok_or_else(|| "no entries".to_string())?
        .iter().filter_map(|e| {
            let b: Vec<u8> = e["b"].as_array()?.iter()
                .filter_map(|x| x.as_u64().map(|v| v as u8)).collect();
            Some((b, e["f"].as_u64()?, e["s"].as_u64()?))
        }).collect();
    let n = entries.len();
    slider.import_dict(1, entries);
    Ok(n)
}

async fn send_job<T>(job: Job, rx: oneshot::Receiver<T>) -> Result<T> {
    get_worker().send(job).await
        .map_err(|_| Error::from_reason("worker closed"))?;
    rx.await.map_err(|_| Error::from_reason("worker dropped"))
}

#[napi]
pub fn gn_compress(data: Buffer) -> Buffer {
    Buffer::from(pipeline::compress(&data))
}

#[napi]
pub fn gn_compress_batch(chunks: Vec<Buffer>) -> Vec<Buffer> {
    use rayon::prelude::*;
    let raw: Vec<Vec<u8>> = chunks.iter().map(|b| b.to_vec()).collect();
    raw.par_iter().map(|d| Buffer::from(pipeline::compress(d))).collect()
}

#[napi]
pub async fn gn_compress_l2(data: Buffer) -> Result<Buffer> {
    let (tx, rx) = oneshot::channel();
    send_job(Job::CompressL2 { data: data.to_vec(), resp: tx }, rx).await
        .map(Buffer::from)
}

#[napi]
pub async fn gn_compress_pressurized(target: Buffer, warm_bufs: Vec<Buffer>, pk: u32) -> Result<Buffer> {
    let (tx, rx) = oneshot::channel();
    let warm: Vec<Vec<u8>> = warm_bufs.into_iter().map(|b| b.to_vec()).collect();
    send_job(Job::CompressPressurized { target: target.to_vec(), warm, pk: pk as usize, resp: tx }, rx).await
        .map(Buffer::from)
}

#[napi]
pub async fn gn_window_stats() -> Result<String> {
    let (tx, rx) = oneshot::channel();
    send_job(Job::WindowStats { resp: tx }, rx).await
}

#[napi]
pub async fn gn_save_snapshot(path: String) -> Result<String> {
    let (tx, rx) = oneshot::channel();
    send_job(Job::SaveSnapshot { path, resp: tx }, rx).await
}

#[napi]
pub async fn gn_load_snapshot(path: String) -> Result<String> {
    let (tx, rx) = oneshot::channel();
    send_job(Job::LoadSnapshot { path, resp: tx }, rx).await
}

#[napi]
pub fn gn_decompress(data: Buffer) -> Result<Buffer> {
    pipeline::decompress(&data)
        .map(Buffer::from)
        .map_err(|e: glasik_core::pipeline::PipelineError| Error::from_reason(e.to_string()))
}
