use std::sync::{Arc, Mutex};

use anyhow::Result;
use asm_runner::ControlShmem;
use asm_runner::HintsFile;
use asm_runner::HintsShmem;
use asm_runner::InputsShmemWriter;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use asm_runner::{MOShMemReader, MTShMemReader, RHShMemReader};
use precompiles_hints::HintsProcessor;
use std::sync::atomic::{AtomicBool, Ordering};
use zisk_common::io::ZiskIO;
use zisk_common::io::ZiskStdin;
use zisk_common::io::{StreamSource, ZiskStream};

/// Configuration for assembly resources.
#[derive(Clone)]
pub struct AsmResourcesConfig {
    /// Optional baseline port to communicate with assembly microservices.
    pub base_port: Option<u16>,

    /// Local rank for distributed execution.
    pub local_rank: i32,

    /// Map unlocked flag.
    pub unlock_mapped_memory: bool,
}

impl std::fmt::Debug for AsmResourcesConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsmResources")
            .field("base_port", &self.base_port)
            .field("local_rank", &self.local_rank)
            .field("unlock_mapped_memory", &self.unlock_mapped_memory)
            .finish_non_exhaustive()
    }
}

/// Encapsulates assembly-related resources including shared memory and hints stream.
#[derive(Clone)]
pub struct AsmResources {
    config: AsmResourcesConfig,

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub mt_shmem_reader: Arc<Mutex<MTShMemReader>>,
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub mo_shmem_reader: Arc<Mutex<MOShMemReader>>,
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    pub rh_shmem_reader: Arc<Mutex<Option<RHShMemReader>>>,

    pub inputs_shmem_writer: Arc<InputsShmemWriter>,

    /// Pipeline for handling precompile hints.
    pub hints_stream: Option<Arc<Mutex<ZiskStream>>>,

    hints_stream_initialized: Arc<AtomicBool>,
}

impl std::fmt::Debug for AsmResources {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsmResources")
            .field("config", &self.config)
            .field("hints_stream_initialized", &self.hints_stream_initialized)
            .finish_non_exhaustive()
    }
}

impl AsmResources {
    pub fn new(
        local_rank: i32,
        base_port: Option<u16>,
        unlock_mapped_memory: bool,
        verbose_mode: proofman_common::VerboseMode,
        with_hints: bool,
    ) -> Result<Self> {
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let asm_shmem_mt = MTShMemReader::new(local_rank, base_port, unlock_mapped_memory)?;

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let asm_shmem_mo = MOShMemReader::new(local_rank, base_port, unlock_mapped_memory)?;

        let control_writer =
            Arc::new(ControlShmem::new(base_port, local_rank, unlock_mapped_memory)?);

        let config = AsmResourcesConfig { base_port, local_rank, unlock_mapped_memory };

        let inputs_shmem_writer = Arc::new(InputsShmemWriter::new(
            base_port,
            local_rank,
            unlock_mapped_memory,
            control_writer.clone(),
        )?);

        // Create hints pipeline with null hints stream initially.
        // Debug flag: true = HintsShmem (shared memory), false = HintsFile (file output)
        const USE_SHARED_MEMORY_HINTS: bool = true;

        let hints_stream = if with_hints {
            let hints_processor = if USE_SHARED_MEMORY_HINTS {
                let hints_shmem =
                    HintsShmem::new(base_port, local_rank, unlock_mapped_memory, control_writer)?;

                HintsProcessor::builder2(hints_shmem, Some(inputs_shmem_writer.clone()))
                    .enable_stats(verbose_mode != proofman_common::VerboseMode::Info)
                    .build()?
            } else {
                let hints_file = HintsFile::new(format!("hints_results_{}.bin", local_rank))?;

                HintsProcessor::builder2(hints_file, Some(inputs_shmem_writer.clone()))
                    .enable_stats(verbose_mode != proofman_common::VerboseMode::Info)
                    .build()?
            };

            Some(Arc::new(Mutex::new(ZiskStream::new(hints_processor))))
        } else {
            None
        };

        Ok(Self {
            config,
            hints_stream,
            hints_stream_initialized: Arc::new(AtomicBool::new(false)),
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            mt_shmem_reader: Arc::new(Mutex::new(asm_shmem_mt)),
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            mo_shmem_reader: Arc::new(Mutex::new(asm_shmem_mo)),
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            rh_shmem_reader: Arc::new(Mutex::new(None)),
            #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
            inputs_shmem_writer,
        })
    }

    pub fn start_stream(&self) -> Result<()> {
        if let Some(hints_stream) = &self.hints_stream {
            hints_stream.lock().unwrap().start_stream()
        } else {
            Ok(())
        }
    }

    pub fn set_hints_stream_src(&self, stream: StreamSource) -> Result<()> {
        if let Some(hints_stream) = &self.hints_stream {
            hints_stream.lock().unwrap().set_hints_stream_src(stream)?;
        } else {
            return Err(anyhow::anyhow!("Hints stream not initialized"));
        }
        self.hints_stream_initialized.store(true, Ordering::SeqCst);
        Ok(())
    }

    pub fn is_hints_stream_initialized(&self) -> bool {
        self.hints_stream_initialized.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        if let Some(hints_stream) = &self.hints_stream {
            hints_stream.lock().unwrap().reset();
            self.hints_stream_initialized.store(false, Ordering::SeqCst);
        }
    }

    pub fn config(&self) -> &AsmResourcesConfig {
        &self.config
    }

    pub fn write_input(&self, stdin: &ZiskStdin) -> Result<()> {
        let inputs = stdin.read_bytes();

        self.inputs_shmem_writer.write_input(&inputs)
    }
}
