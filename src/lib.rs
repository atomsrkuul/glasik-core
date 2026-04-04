//! glasik-core -- GN compression architecture

pub mod codec;
pub mod tokenizer;
pub mod shards;
pub mod pipeline;

#[cfg(feature = "python")]
pub mod bindings;
