pub mod hints;
pub mod hints_definitions;
mod hints_processor;
mod hints_stream;

pub mod secp256k1;

pub use hints::*;
pub use hints_definitions::*;
pub use hints_processor::{
    PrecompileHint, PrecompileHintsProcessor, HINTS_TYPE_ECRECOVER, HINTS_TYPE_RESULT,
};
pub use hints_stream::HintsStream;
pub use secp256k1::*;

pub trait HintsProcessor {
    /// Process hints and return the processed data along with a flag indicating if CTRL_END was encountered.
    ///
    /// # Returns
    /// A tuple of (processed_hints, has_ctrl_end) where:
    /// - processed_hints: Vec<u64> - The processed hint data
    /// - has_ctrl_end: bool - True if CTRL_END was found (signals end of batch)
    fn process_hints(&self, hints: &[u64], first_batch: bool) -> anyhow::Result<(Vec<u64>, bool)>;
}

pub trait HintsSink {
    fn submit(&self, processed: Vec<u64>) -> anyhow::Result<()>;
}
