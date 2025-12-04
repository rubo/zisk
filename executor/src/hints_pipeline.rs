//! HintsPipeline is responsible for processing precompile hints and writing them to shared memory.
//!
//! It uses a ZiskHintin as the source of hints, and writes the processed hints to shared memories
//! specified by their names.
//! The pipeline ensures that the shared memory writers are initialized before writing hints.

use anyhow::Result;
use asm_runner::SharedMemoryWriter;
use precompiles_common::PrecompileHintsProcessor;
use std::sync::Mutex;
use tracing::{debug, info, warn};
use zisk_common::io::{ZiskHintin, ZiskIO};

/// HintsPipeline struct manages the processing of precompile hints and writing them to shared memory.
pub struct HintsPipeline {
    /// The ZiskHintin source for reading hints.
    hintin: Mutex<ZiskHintin>,

    /// Names of the shared memories to write hints to.
    shmem_names: Vec<String>,

    /// Whether to unlock mapped memory after writing.
    unlock_mapped_memory: bool,

    /// Shared memory writers for writing processed hints.
    shmem_writers: Mutex<Vec<SharedMemoryWriter>>,
}

impl HintsPipeline {
    const MAX_PRECOMPILE_SIZE: u64 = 0x10000000; // 256MB

    /// Create a new HintsPipeline with the given ZiskHintin, shared memory names, and unlock option.
    ///
    /// # Arguments
    /// * `hintin` - The ZiskHintin source for reading hints.
    /// * `shmem_names` - A vector of shared memory names to write hints to.
    /// * `unlock_mapped_memory` - Whether to unlock mapped memory after writing.
    ///
    /// # Returns
    /// A new `HintsPipeline` instance with uninitialized writers.
    pub fn new(hintin: ZiskHintin, shmem_names: Vec<String>, unlock_mapped_memory: bool) -> Self {
        Self {
            hintin: Mutex::new(hintin),
            shmem_names,
            unlock_mapped_memory,
            shmem_writers: Mutex::new(Vec::new()),
        }
    }

    /// Add a shared memory name to the pipeline.
    ///
    /// This method must be called before initialization.
    ///
    /// # Arguments
    /// * `name` - The name of the shared memory to add.
    ///
    /// # Returns
    /// * `Ok(())` - If the name was successfully added or already exists
    /// * `Err` - If writers have already been initialized
    pub fn add_shmem_name(&mut self, name: String) -> Result<()> {
        // Check if the writers have already been initialized
        let shmem_writers = self.shmem_writers.lock().unwrap();
        if !shmem_writers.is_empty() {
            return Err(anyhow::anyhow!(
                "Cannot add shared memory name '{}' after initialization",
                name
            ));
        }

        // Check if the name already exists
        if self.shmem_names.contains(&name) {
            warn!(
                "Shared memory name '{}' already exists in the pipeline. Skipping addition.",
                name
            );
            return Ok(());
        }

        self.shmem_names.push(name);
        Ok(())
    }

    /// Set a new ZiskHintin for the pipeline.
    ///
    /// # Arguments
    /// * `hintin` - The new ZiskHintin source for reading hints.
    pub fn set_hintin(&self, hintin: ZiskHintin) {
        let mut guard = self.hintin.lock().unwrap();
        *guard = hintin;
    }

    /// Initialize the shared memory writers for the pipeline.
    ///
    /// This method creates SharedMemoryWriter instances for each shared memory name.
    /// If writers are already initialized it logs a warning and does nothing.
    pub fn initialize(&self) {
        let mut shmem_writer = self.shmem_writers.lock().unwrap();

        if !shmem_writer.is_empty() {
            warn!(
                "SharedMemoryWriters for precompile hints is already initialized at '{}'. Skipping",
                self.shmem_names.join(", ")
            );
        } else {
            debug!(
                "Initializing SharedMemoryWriter for precompile hints at '{}'",
                self.shmem_names.join(", ")
            );

            *shmem_writer = self
                .shmem_names
                .iter()
                .map(|name| {
                    SharedMemoryWriter::new(
                        &name,
                        Self::MAX_PRECOMPILE_SIZE as usize,
                        self.unlock_mapped_memory,
                    )
                    .expect("Failed to create SharedMemoryWriter for precompile hints")
                })
                .collect();
        }
    }

    /// Process and write precompile hints to all shared memory writers.
    ///
    /// This method:
    /// 1. Reads hints from the ZiskHintin source
    /// 2. Processes them using PrecompileHintsProcessor
    /// 3. Prepares the data with a length prefix (u64) followed by the processed hints
    /// 4. Writes the data to all configured shared memory writers
    ///
    /// The shared memory writers will be automatically initialized if needed.
    ///
    /// # Returns
    /// * `Ok(())` - If hints were successfully processed and written
    /// * `Err` - If processing or writing fails
    pub fn write_hints(&self) -> Result<()> {
        // Check if initialization is needed without holding the lock
        let needs_init = {
            let shmem_writers = self.shmem_writers.lock().unwrap();
            shmem_writers.is_empty()
        };

        if needs_init {
            self.initialize();
        }

        let mut hintin = self.hintin.lock().unwrap();

        let hints = zisk_common::reinterpret_vec(hintin.read())?;

        let processor = PrecompileHintsProcessor::new()?;
        let processed = processor.process_hints(&hints)?;

        info!("Precompile hints have generated {} u64 values", processed.len());

        // Input size includes length prefix as u64
        let shmem_input_size = processed.len() + 1;

        let mut full_input = Vec::with_capacity(shmem_input_size);
        // Prefix with length as u64
        full_input.extend_from_slice(&[processed.len() as u64]);
        // Append processed hints
        full_input.extend_from_slice(&processed);

        println!("full_input size: {}", full_input.len());

        let shmem_writers = self.shmem_writers.lock().unwrap();
        for shmem_writer in shmem_writers.iter() {
            shmem_writer.write_input(&full_input)?;
        }

        Ok(())
    }
}
