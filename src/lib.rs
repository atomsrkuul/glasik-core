//! glasik-core -- GN compression architecture

pub mod static_dict;
pub mod codec;
pub mod pipeline;
pub mod shards;
pub mod tokenizer;

#[cfg(feature = "python")]
pub mod bindings;
