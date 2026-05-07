//! glasik-core -- GN compression architecture

pub mod static_dict;
pub mod level4;
pub mod fractal;
pub mod sliding_v2_l4;
pub mod codec;
pub mod pipeline;
pub mod shards;
pub mod tokenizer;

#[cfg(feature = "python")]
pub mod bindings;
