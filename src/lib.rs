//! glasik-core -- GN compression architecture

pub mod codec;
pub mod tokenizer;
pub mod shards;

#[cfg(feature = "python")]
pub mod bindings;
