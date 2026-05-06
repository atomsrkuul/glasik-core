//! glasik-core -- GN compression architecture

pub mod static_dict;
pub mod ans_table;
pub mod level4;
pub mod fractal;
pub mod sliding_v3;
pub mod codec;
pub mod pipeline;
pub mod shards;
pub mod tokenizer;

#[cfg(feature = "python")]
pub mod bindings;
