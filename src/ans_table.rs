//! ans_table.rs -- Bundled pretrained o1 ANS table
//! Trained on LLM conversation corpus (conversations.json).
//! Embedded at compile time; used by pipeline.rs as the entropy coder.

pub const ANS_O1_TABLE: &[u8] = include_bytes!("../scripts/gn_ans_o1_table.bin");
