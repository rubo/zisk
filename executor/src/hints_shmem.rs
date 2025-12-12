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

/// Names for a service's shared memory and semaphore resources
struct ServiceResourceNames {
    control_name: String,
    data_name: String,
    sem_available_name: String,
    sem_read_name: String,
}

impl ServiceResourceNames {
    fn new(service: &AsmService, port: u16, local_rank: i32) -> Self {
        Self {
            control_name: AsmSharedMemory::<AsmMTHeader>::shmem_control_name(
                port, *service, local_rank,
            ),
            data_name: AsmSharedMemory::<AsmMTHeader>::shmem_precompile_name(
                port, *service, local_rank,
            ),
            sem_available_name: AsmSharedMemory::<AsmMTHeader>::semaphore_available_name(
                port, *service, local_rank,
            ),
            sem_read_name: AsmSharedMemory::<AsmMTHeader>::semaphore_read_name(
                port, *service, local_rank,
            ),
        }
    }
}

/// Represents a service's shared memory and synchronization resources
struct ServiceResources {
    /// Control shared memory writer
    control_writer: SharedMemoryWriter,
    /// Data shared memory writer
    data_writer: SharedMemoryWriter,
    /// Semaphore to signal data availability
    sem_available: NamedSemaphore,
    /// Semaphore to wait for data consumption
    sem_read: NamedSemaphore,
}

/// HintsShmem struct manages the writing of processed precompile hints to shared memory.
pub struct HintsShmem {
    /// Service resources combining shared memory writers and semaphores
    resources: Mutex<Vec<ServiceResources>>,
}

unsafe impl Send for HintsShmem {}
unsafe impl Sync for HintsShmem {}

impl HintsShmem {
    const CONTROL_PRECOMPILE_SIZE: u64 = 0x2000; // 8KB
    const MAX_PRECOMPILE_SIZE: u64 = 0x10000000; // 256MB
                                                 // const MAX_PRECOMPILE_SIZE: u64 = 0x100000; // 1MB
    const BUFFER_THRESHOLD: u64 = 1000; // 1000 bytes - threshold for signaling reader

    /// Create a new HintsShmem with the given shared memory names and unlock option.
    ///
    /// # Arguments
    /// * `base_port` - Optional base port for generating shared memory names.
    /// * `local_rank` - Local rank for generating shared memory names.
    /// * `unlock_mapped_memory` - Whether to unlock mapped memory after writing.
    ///
    /// # Returns
    /// A new `HintsShmem` instance with uninitialized writers.
    pub fn new(
        base_port: Option<u16>,
        local_rank: i32,
        unlock_mapped_memory: bool,
    ) -> Result<Self> {
        let resources_names = AsmServices::SERVICES
            .iter()
            .map(|service| {
                let port = if let Some(base_port) = base_port {
                    AsmServices::port_for(service, base_port, local_rank)
                } else {
                    AsmServices::default_port(service, local_rank)
                };
                ServiceResourceNames::new(service, port, local_rank)
            })
            .collect();

        let resources = Mutex::new(Self::create_resources(resources_names, unlock_mapped_memory)?);

        Ok(Self { resources })
    }

    /// Initialize the shared memory writers for the pipeline.
    ///
    /// This method creates SharedMemoryWriter instances for each shared memory name.
    /// If writers are already initialized it logs a warning and does nothing.
    fn create_resources(
        resources_names: Vec<ServiceResourceNames>,
        unlock_mapped_memory: bool,
    ) -> Result<Vec<ServiceResources>> {
        debug!("Initializing resources for precompile hints");

        Ok(resources_names
            .iter()
            .map(|names: &ServiceResourceNames| ServiceResources {
                control_writer: Self::create_writer(
                    &names.control_name,
                    Self::CONTROL_PRECOMPILE_SIZE as usize,
                    unlock_mapped_memory,
                ),
                data_writer: Self::create_writer(
                    &names.data_name,
                    Self::MAX_PRECOMPILE_SIZE as usize,
                    unlock_mapped_memory,
                ),
                sem_available: Self::create_semaphore(&names.sem_available_name),
                sem_read: Self::create_semaphore(&names.sem_read_name),
            })
            .collect())
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

    #[inline]
    fn get_write_size(&self, writer: &SharedMemoryWriter) -> Result<u64> {
        writer.read_u64_at(0).map_err(|e| anyhow::anyhow!(e))
    }

    #[inline]
    fn set_write_size(&self, writer: &SharedMemoryWriter, size: u64) -> anyhow::Result<()> {
        writer.write_u64_at(0, size).map_err(|e| anyhow::anyhow!(e))
    }

    #[inline]
    fn get_read_size(&self, writer: &SharedMemoryWriter) -> Result<u64> {
        writer.read_u64_at(4096).map_err(|e| anyhow::anyhow!(e))
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
        let data_size = processed.len() as u64;

        let mut resources = self.resources.lock().unwrap();

        // for resource in resources.iter_mut() {
        let resource = &mut resources[0];

        // Read current positions
        let write_pos = self.get_write_size(&resource.control_writer)?;
        let read_pos = self.get_read_size(&resource.control_writer)?;

        // Calculate occupied space in ring buffer (positions are absolute values)
        let occupied_space = write_pos - read_pos;
        let available_space = (Self::MAX_PRECOMPILE_SIZE >> 3) - occupied_space;

        debug_assert!(
            available_space <= (Self::MAX_PRECOMPILE_SIZE >> 3),
            "Available space calculation error"
        );
        // TODO! Check for overflow of write_pos and read_pos and handle it

        // Flow control based on buffer occupancy
        if available_space < data_size {
            // Not enough space - signal reader and wait for consumption
            // resource.sem_available.post()?;
            if write_pos > 131000 {
                println!("Waiting on sem_read for precompile hints write_pos={} occupied={} available={} needed={}",
                    write_pos, occupied_space, available_space, data_size);
            }
            resource.sem_read.wait()?;
        } else if available_space < Self::BUFFER_THRESHOLD {
            // Buffer getting full - signal reader but don't wait
            // resource.sem_available.post()?;
        }

        // Write data to shared memory with automatic wraparound
        resource.data_writer.write_ring_buffer(&processed)?;

        // Update write position in control memory with wraparound
        self.set_write_size(&resource.control_writer, write_pos + data_size)?;

        resource.sem_available.post()?;

        // if write_pos > 131000 {
        //     println!("Posted available semaphore for precompile hints {}", write_pos);
        // }
        // }

        Ok(())
    }
}
