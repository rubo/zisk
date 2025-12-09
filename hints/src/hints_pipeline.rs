//! HintsPipeline is responsible for processing precompile hints and submitting them to a sink.
//! It uses a ZiskHintin as the source of hints, and writes the processed hints to a HintsSink.

use crate::{HintsProcessor, HintsSink};
use anyhow::Result;
use std::sync::Mutex;
use tracing::info;
use zisk_common::io::{StreamRead, StreamSource};

/// HintsPipeline struct manages the processing of precompile hints and writing them to shared memory.
pub struct HintsPipeline<HP: HintsProcessor, HS: HintsSink> {
    /// The hints processor used to process hints before writing.
    hints_processor: HP,

    /// The hints sink used to submit processed hints.
    hints_sink: HS,

    /// The ZiskHintin source for reading hints.
    hintin: Mutex<StreamSource>,
}

impl<HP: HintsProcessor, HS: HintsSink> HintsPipeline<HP, HS> {
    /// Create a new HintsPipeline with the given processor, ZiskHintin, and sink.
    ///
    /// # Arguments
    /// * `hints_processor` - The processor used to process hints.
    /// * `hints_sink` - The sink used to submit processed hints.
    /// * `hintin` - The ZiskHintin source for reading hints.
    ///
    /// # Returns
    /// A new `HintsPipeline` instance with uninitialized writers.
    pub fn new(hints_processor: HP, hints_sink: HS, hintin: StreamSource) -> Self {
        Self { hints_processor, hints_sink, hintin: Mutex::new(hintin) }
    }

    /// Set a new ZiskHintin for the pipeline.
    ///
    /// # Arguments
    /// * `hintin` - The new ZiskHintin source for reading hints.
    pub fn set_hintin(&self, hintin: StreamSource) {
        let mut guard = self.hintin.lock().unwrap();
        *guard = hintin;
    }

    /// Process and write precompile hints to all shared memory writers.
    ///
    /// This method:
    /// 1. Reads hints from the ZiskHintin source
    /// 2. Processes them using PrecompileHintsProcessor
    /// 3. Submits the processed hints to the HintsSink
    ///
    /// # Returns
    /// * `Ok(())` - If hints were successfully processed and submitted
    /// * `Err` - If processing or submission fails
    pub fn write_hints(&self) -> Result<()> {
        let mut hintin = self.hintin.lock().unwrap();

        let hints = zisk_common::reinterpret_vec(hintin.next()?)?;

        let processed = self.hints_processor.process_hints(&hints)?;

        // // STore processed hints in a temp file for debugging
        // std::fs::write(
        //     "/data/hints/processed_hints.bin",
        //     &zisk_common::reinterpret_vec::<u64, u8>(processed.clone())?,
        // )?;
        // // // read processed into a /data/hints/precompile_cache.bin
        // // let processed = std::fs::read("/data/hints/precompile_cache.bin")?;
        // // let processed = zisk_common::reinterpret_vec::<u8, u64>(processed)?;

        info!("Precompile hints have generated {} u64 values", processed.len());

        self.hints_sink.submit(processed)
    }
}
