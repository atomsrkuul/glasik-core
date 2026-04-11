use napi_derive::napi;
use napi::bindgen_prelude::*;
use sha2::{Sha256, Digest};

#[napi]
pub async fn gn_compress_fractal_with_vtc(
    data: Buffer,
    shard_type: String,
    session_id: String,
) -> Result<String> {

    let frame = crate::gn_compress_fractal(
        data.clone(),
        shard_type.clone(),
        session_id.clone()
    ).await?;

    let mut hasher = Sha256::new();

    // domain separation
    hasher.update(shard_type.as_bytes());

    if frame.len() > 5 {
        let pairs_len = u16::from_le_bytes([frame[1], frame[2]]) as usize;
        let l3_len = u16::from_le_bytes([frame[3], frame[4]]) as usize;

        let start = 5 + l3_len;
        let end = start + pairs_len;

        if end <= frame.len() {
            hasher.update(&frame[start..end]);
        } else {
            hasher.update(&frame);
        }
    } else {
        hasher.update(&frame);
    }

    let hash = hasher.finalize();
    let vtc = format!("VTC-v1-{}", hex::encode(&hash[..16]));

    Ok(vtc)
}
