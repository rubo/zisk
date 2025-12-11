//! HintsShmem is responsible for writting precompile processed hints to shared memory.
//!
//! It implements the HintsSink trait to receive processed hints and write them to shared memory
//! using SharedMemoryWriter instances.

use anyhow::Result;
use asm_runner::{AsmMTHeader, AsmService, AsmServices, AsmSharedMemory, SharedMemoryWriter};
use named_sem::NamedSemaphore;
use std::sync::Mutex;
use tracing::debug;
use zisk_hints::HintsSink;

enum NameType {
    Control,
    Data,
    SemAvail,
    SemRead,
}

/// HintsShmem struct manages the writing of processed precompile hints to shared memory.
pub struct HintsShmem {
    /// Names of the shared memories to write hints to. 0 for control, 1 for data
    shmem_names: Vec<(String, String)>,

    /// Names of the semaphores for synchronization. 0 for available, 1 for read
    sem_names: Vec<(String, String)>,

    /// Whether to unlock mapped memory after writing.
    unlock_mapped_memory: bool,

    /// Shared memory writers for writing processed hints. 0 for control, 1 for data
    shmem_writers: Mutex<Vec<(SharedMemoryWriter, SharedMemoryWriter)>>,

    /// Control semaphores for synchronization. 0 for available, 1 for read
    sem_control: Mutex<Vec<(NamedSemaphore, NamedSemaphore)>>,
}

unsafe impl Send for HintsShmem {}
unsafe impl Sync for HintsShmem {}

impl HintsShmem {
    const CONTROL_PRECOMPILE_SIZE: u64 = 0x1000; // 4KB
    const MAX_PRECOMPILE_SIZE: u64 = 0x10000000; // 256MB

    /// Create a new HintsShmem with the given shared memory names and unlock option.
    ///
    /// # Arguments
    /// * `base_port` - Optional base port for generating shared memory names.
    /// * `local_rank` - Local rank for generating shared memory names.
    /// * `unlock_mapped_memory` - Whether to unlock mapped memory after writing.
    ///
    /// # Returns
    /// A new `HintsShmem` instance with uninitialized writers.
    pub fn new(base_port: Option<u16>, local_rank: i32, unlock_mapped_memory: bool) -> Self {
        // Generate shared memory names for hints pipeline.
        let shmem_names = AsmServices::SERVICES
            .iter()
            .map(|service| {
                let port = if let Some(base_port) = base_port {
                    AsmServices::port_for(service, base_port, local_rank)
                } else {
                    AsmServices::default_port(service, local_rank)
                };
                let control_name =
                    Self::resource_name(service, port, local_rank, NameType::Control);
                let data = Self::resource_name(service, port, local_rank, NameType::Data);
                (control_name, data)
            })
            .collect::<Vec<_>>();

        // Generate semaphore names for hints pipeline.
        let sem_names = AsmServices::SERVICES
            .iter()
            .map(|service| {
                let port = if let Some(base_port) = base_port {
                    AsmServices::port_for(service, base_port, local_rank)
                } else {
                    AsmServices::default_port(service, local_rank)
                };
                let sem_avail = Self::resource_name(service, port, local_rank, NameType::SemAvail);
                let sem_read = Self::resource_name(service, port, local_rank, NameType::SemRead);
                (sem_avail, sem_read)
            })
            .collect::<Vec<_>>();

        Self {
            shmem_names,
            sem_names,
            unlock_mapped_memory,
            shmem_writers: Mutex::new(Vec::new()),
            sem_control: Mutex::new(Vec::new()),
        }
    }

    fn resource_name(
        service: &AsmService,
        port: u16,
        local_rank: i32,
        name_type: NameType,
    ) -> String {
        match name_type {
            NameType::Control => {
                AsmSharedMemory::<AsmMTHeader>::shmem_control_name(port, *service, local_rank)
            }
            NameType::Data => {
                AsmSharedMemory::<AsmMTHeader>::shmem_precompile_name(port, *service, local_rank)
            }
            NameType::SemAvail => AsmSharedMemory::<AsmMTHeader>::shmem_semaphore_available_name(
                port, *service, local_rank,
            ),
            NameType::SemRead => AsmSharedMemory::<AsmMTHeader>::shmem_semaphore_read_name(
                port, *service, local_rank,
            ),
        }
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
    fn initialize(&self) -> Result<()> {
        let mut shmem_writer = self.shmem_writers.lock().unwrap();
        let mut sem_control = self.sem_control.lock().unwrap();

        // Initialize shared memory writers
        if !shmem_writer.is_empty() {
            return Err(anyhow::anyhow!(
                "SharedMemoryWriters for precompile hints already initialized."
            ));
        }

        debug!("Initializing SharedMemoryWriter for precompile hints",);
        *shmem_writer = self
            .shmem_names
            .iter()
            .map(|(control_name, name)| {
                (
                    Self::create_writer(
                        control_name,
                        Self::CONTROL_PRECOMPILE_SIZE as usize,
                        self.unlock_mapped_memory,
                    ),
                    Self::create_writer(
                        name,
                        Self::MAX_PRECOMPILE_SIZE as usize,
                        self.unlock_mapped_memory,
                    ),
                )
            })
            .collect();

        // Initialize semaphores
        if !sem_control.is_empty() {
            return Err(anyhow::anyhow!(
                "Control semaphores for precompile hints already initialized."
            ));
        }

        debug!("Initializing control semaphores for precompile hints",);
        *sem_control = self
            .sem_names
            .iter()
            .map(|(sem_avail_name, sem_read_name)| {
                (Self::create_semaphore(sem_avail_name), Self::create_semaphore(sem_read_name))
            })
            .collect();

        Ok(())
    }

    /// Create a SharedMemoryWriter with error handling.
    fn create_writer(name: &str, size: usize, unlock_mapped_memory: bool) -> SharedMemoryWriter {
        SharedMemoryWriter::new(name, size, unlock_mapped_memory)
            .expect("Failed to create SharedMemoryWriter for precompile hints")
    }

    /// Create a NamedSemaphore with error handling.
    fn create_semaphore(name: &str) -> NamedSemaphore {
        NamedSemaphore::create(name.to_string(), 0)
            .expect("Failed to create semaphore for precompile hints")
    }

    fn write_size(&self, writer: &SharedMemoryWriter) -> u64 {
        writer.read_u64_at(0)
    }

    fn read_size(&self, writer: &SharedMemoryWriter) -> u64 {
        writer.read_u64_at(8)
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
            self.initialize()?;
        }

        // Input size includes length prefix as u64
        let shmem_input_size = processed.len();

        let mut full_input = Vec::with_capacity(shmem_input_size);
        full_input.extend_from_slice(&processed);

        let shmem_writers = self.shmem_writers.lock().unwrap();
        for shmem_writer in shmem_writers.iter() {
            shmem_writer.1.write_input(&full_input)?;
            shmem_writer.0.write_input(&[processed.len() as u64])?;
        }

        Ok(())
    }
}
