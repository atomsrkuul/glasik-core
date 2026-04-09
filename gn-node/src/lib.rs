#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi_derive::napi;
use glasik_core::tokenizer::sliding_v2::SlidingTokenizerV2;
use glasik_core::pipeline;
use glasik_core::static_dict;
use std::cell::RefCell;

// Thread-local slider -- no mutex needed for single-threaded Node.js
thread_local! {
    static SLIDER: RefCell<Option<SlidingTokenizerV2>> = RefCell::new(None);
}

fn with_slider<F, R>(f: F) -> R
where F: FnOnce(&mut SlidingTokenizerV2) -> R {
    SLIDER.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            let entries = static_dict::load_static_dict();
            *opt = Some(SlidingTokenizerV2::new_with_static(entries));
        }
        f(opt.as_mut().unwrap())
    })
}

fn deflate_tokenized(tokenized: Vec<u8>) -> Vec<u8> {
    let mut comp = libdeflater::Compressor::new(libdeflater::CompressionLvl::default());
    let max = comp.deflate_compress_bound(tokenized.len());
    let mut deflated = vec![0u8; max];
    match comp.deflate_compress(&tokenized, &mut deflated) {
        Ok(n) => {
            deflated.truncate(n);
            if deflated.len() < tokenized.len() { deflated } else { tokenized }
        }
        Err(_) => tokenized
    }
}

#[napi]
pub fn gn_compress(data: Buffer) -> Buffer {
    Buffer::from(pipeline::compress(&data))
}

#[napi]
pub fn gn_compress_l2(data: Buffer) -> Buffer {
    with_slider(|slider| {
        let tokenized = slider.encode(&data);
        Buffer::from(deflate_tokenized(tokenized))
    })
}

#[napi]
pub fn gn_compress_batch(chunks: Vec<Buffer>) -> Vec<Buffer> {
    use rayon::prelude::*;
    let raw: Vec<Vec<u8>> = chunks.iter().map(|b| b.to_vec()).collect();
    raw.par_iter()
        .map(|data| Buffer::from(pipeline::compress(data.as_slice())))
        .collect()
}

#[napi]
pub fn gn_window_stats() -> String {
    with_slider(|slider| {
        let (entries, batches) = slider.stats();
        format!(r#"{{"window_entries":{},"batches":{}}}"#, entries, batches)
    })
}

#[napi]
pub fn gn_decompress(data: Buffer) -> napi::Result<Buffer> {
    pipeline::decompress(&data)
        .map(Buffer::from)
        .map_err(|e: glasik_core::pipeline::PipelineError| napi::Error::from_reason(e.to_string()))
}
