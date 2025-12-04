//! HintsShmem is responsible for writting precompile processed hints to shared memory.
//!
//! It implements the HintsSink trait to receive processed hints and write them to shared memory
//! using SharedMemoryWriter instances.

use anyhow::Result;
use asm_runner::SharedMemoryWriter;
use std::sync::Mutex;
use tracing::{debug, warn};
use zisk_hints::HintsSink;

/// HintsShmem struct manages the writing of processed precompile hints to shared memory.
pub struct HintsShmem {
    /// Names of the shared memories to write hints to. 0 for control, 1 for data
    shmem_names: Vec<(String, String)>,

    /// Whether to unlock mapped memory after writing.
    unlock_mapped_memory: bool,

    /// Shared memory writers for writing processed hints. 0 for control, 1 for data
    shmem_writers: Mutex<Vec<(SharedMemoryWriter, SharedMemoryWriter)>>,
}

impl HintsShmem {
    const CONTROL_PRECOMPILE_SIZE: u64 = 0x1000; // 4KB
    const MAX_PRECOMPILE_SIZE: u64 = 0x10000000; // 256MB

    /// Create a new HintsShmem with the given shared memory names and unlock option.
    ///
    /// # Arguments
    /// * `shmem_control_names` - A vector of shared memory control names to write hints to.
    /// * `shmem_names` - A vector of shared memory names to write hints to.
    /// * `unlock_mapped_memory` - Whether to unlock mapped memory after writing.
    ///
    /// # Returns
    /// A new `HintsShmem` instance with uninitialized writers.
    pub fn new(
        shmem_control_names: Vec<String>,
        shmem_names: Vec<String>,
        unlock_mapped_memory: bool,
    ) -> Self {
        assert_eq!(
            shmem_control_names.len(),
            shmem_names.len(),
            "Shared memory names and control names must have the same length"
        );

        // Map names to tuples
        let shmem_names: Vec<(String, String)> =
            shmem_control_names.into_iter().zip(shmem_names.into_iter()).collect();

        Self { shmem_names, unlock_mapped_memory, shmem_writers: Mutex::new(Vec::new()) }
    }

    /// Add a shared memory name to the pipeline.
    ///
    /// This method must be called before initialization.
    ///
    /// # Arguments
    /// * `control_name` - The name of the control shared memory to add.
    /// * `name` - The name of the shared memory to add.
    ///
    /// # Returns
    /// * `Ok(())` - If the name was successfully added or already exists
    /// * `Err` - If writers have already been initialized
    pub fn add_shmem_name(&mut self, control_name: String, name: String) -> Result<()> {
        // Check if the writers have already been initialized
        let shmem_writers = self.shmem_writers.lock().unwrap();
        if !shmem_writers.is_empty() {
            return Err(anyhow::anyhow!(
                "Cannot add shared memory name '{}' after initialization",
                name
            ));
        }

        // Check if the name already exists
        if self.shmem_names.contains(&(control_name.clone(), name.clone())) {
            warn!(
                "Shared memory name '{}' already exists in the pipeline. Skipping addition.",
                name
            );
            return Ok(());
        }

        self.shmem_names.push((control_name, name));

        Ok(())
    }

    /// Check if the shared memory writers have been initialized.
    fn is_initialized(&self) -> bool {
        let shmem_writers = self.shmem_writers.lock().unwrap();
        !shmem_writers.is_empty()
    }

    /// Initialize the shared memory writers for the pipeline.
    ///
    /// This method creates SharedMemoryWriter instances for each shared memory name.
    /// If writers are already initialized it logs a warning and does nothing.
    fn initialize(&self) {
        let mut shmem_writer = self.shmem_writers.lock().unwrap();

        if !shmem_writer.is_empty() {
            warn!("SharedMemoryWriters for precompile hints is already initialized. Skipping");
        } else {
            debug!("Initializing SharedMemoryWriter for precompile hints",);

            *shmem_writer = self
                .shmem_names
                .iter()
                .map(|(control_name, name)| {
                    (
                        SharedMemoryWriter::new(
                            &control_name,
                            Self::CONTROL_PRECOMPILE_SIZE as usize,
                            self.unlock_mapped_memory,
                        )
                        .expect("Failed to create SharedMemoryWriter for precompile hints"),
                        SharedMemoryWriter::new(
                            &name,
                            Self::MAX_PRECOMPILE_SIZE as usize,
                            self.unlock_mapped_memory,
                        )
                        .expect("Failed to create SharedMemoryWriter for precompile hints"),
                    )
                })
                .collect();
        }
    }
}

impl HintsSink for HintsShmem {
    /// Writes processed precompile hints to all shared memory writers.
    ///
    /// # Arguments
    /// * `processed` - A vector of processed precompile hints as u64 values.
    ///
    /// # Returns
    /// * `Ok(())` - If hints were successfully written to all shared memories
    /// * `Err` - If writing to any shared memory fails
    fn submit(&self, processed: Vec<u64>) -> anyhow::Result<()> {
        // TODO! Is it necessary????
        if !self.is_initialized() {
            self.initialize();
        }

        // Input size includes length prefix as u64
        let shmem_input_size = processed.len();

        let mut full_input = Vec::with_capacity(shmem_input_size);
        full_input.extend_from_slice(&processed);

        println!("full_input size: {}", full_input.len());

        let shmem_writers = self.shmem_writers.lock().unwrap();
        for shmem_writer in shmem_writers.iter() {
            shmem_writer.1.write_input(&full_input)?;
            shmem_writer.0.write_input(&[processed.len() as u64])?;
        }

        Ok(())
    }
}
