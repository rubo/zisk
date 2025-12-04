pub mod hints;
pub mod hints_definitions;
mod hints_pipeline;
mod hints_processor;

pub mod secp256k1;

pub use hints::*;
pub use hints_definitions::*;
pub use hints_pipeline::HintsPipeline;
pub use hints_processor::{
    PrecompileHint, PrecompileHintsProcessor, HINTS_TYPE_ECRECOVER, HINTS_TYPE_RESULT,
};
pub use secp256k1::*;

pub trait HintsProcessor {
    fn process_hints(&self, hints: &[u64]) -> anyhow::Result<Vec<u64>>;
}

pub trait HintsSink {
    fn submit(&self, processed: Vec<u64>) -> anyhow::Result<()>;
}
