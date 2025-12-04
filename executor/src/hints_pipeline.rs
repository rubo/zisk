use anyhow::Result;
use asm_runner::SharedMemoryWriter;
use precompiles_common::PrecompileHintsProcessor;
use std::sync::Mutex;
use tracing::{debug, info, warn};
use zisk_common::io::{ZiskHintin, ZiskIO};

pub struct HintsPipeline {
    hintin: Mutex<ZiskHintin>,
    shmem_names: Vec<String>,
    unlock_mapped_memory: bool,
    shmem_writers: Mutex<Vec<SharedMemoryWriter>>,
}

impl HintsPipeline {
    const MAX_PRECOMPILE_SIZE: u64 = 0x10000000; // 256MB

    pub fn new(hintin: ZiskHintin, shmem_names: Vec<String>, unlock_mapped_memory: bool) -> Self {
        Self {
            hintin: Mutex::new(hintin),
            shmem_names,
            unlock_mapped_memory,
            shmem_writers: Mutex::new(Vec::new()),
        }
    }

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

    pub fn set_hintin(&self, hintin: ZiskHintin) {
        let mut guard = self.hintin.lock().unwrap();
        *guard = hintin;
    }

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

        let hints = Self::reinterpret_vec(hintin.read());

        let processor = PrecompileHintsProcessor::new()?;
        let processed = processor.process_hints(&hints)?;

        info!("Precompile hints have generated {} u64 values", processed.len());

        // Input size includes length prefix as u64
        let shmem_input_size = processed.len() + 1;

        let mut full_input = Vec::with_capacity(shmem_input_size * 8);
        full_input.extend_from_slice(&processed.len().to_le_bytes());
        full_input.extend_from_slice(&Self::reinterpret_vec(processed));

        let shmem_writers = self.shmem_writers.lock().unwrap();
        for shmem_writer in shmem_writers.iter() {
            shmem_writer.write_input(&full_input)?
        }

        Ok(())
    }

    fn reinterpret_vec<T, U>(v: Vec<T>) -> Vec<U> {
        let size_t = std::mem::size_of::<T>();
        let size_u = std::mem::size_of::<U>();

        assert_eq!(
            v.as_ptr() as usize % std::mem::align_of::<U>(),
            0,
            "Vec is not properly aligned"
        );

        let len = (v.len() * size_t) / size_u;
        let cap = (v.capacity() * size_t) / size_u;
        let ptr = v.as_ptr() as *mut U;

        std::mem::forget(v);
        unsafe { Vec::from_raw_parts(ptr, len, cap) }
    }
}
