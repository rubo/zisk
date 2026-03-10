use std::sync::{Arc, Mutex};

use anyhow::Result;
use asm_runner::ControlShmem;
use asm_runner::HintsFile;
use asm_runner::HintsShmem;
use asm_runner::InputsShmemWriter;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
use asm_runner::{MOShMemReader, MTShMemReader, RHShMemReader};
use precompiles_hints::{HintsProcessor, MpiBroadcastFn};
use std::sync::atomic::{AtomicBool, Ordering};
use zisk_common::io::StreamSink;
use zisk_common::io::ZiskIO;
use zisk_common::io::ZiskStdin;
use zisk_common::io::{StreamProcessor, StreamSource, ZiskStream};

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

    pub hints_sink: Option<Arc<dyn StreamSink>>,

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
        mpi_broadcast_fn: Option<MpiBroadcastFn>,
        init_rom: bool,
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

        let (hints_stream, hints_sink) = if with_hints {
            let (hints_processor, hints_sink): (HintsProcessor, Arc<dyn StreamSink>) =
                if USE_SHARED_MEMORY_HINTS {
                    let hints_shmem = Arc::new(HintsShmem::new(
                        base_port,
                        local_rank,
                        unlock_mapped_memory,
                        control_writer,
                    )?);

                    let mut builder = HintsProcessor::builder(
                        hints_shmem.clone(),
                        Some(inputs_shmem_writer.clone()),
                    )
                    .enable_stats(verbose_mode != proofman_common::VerboseMode::Info);

                    if let Some(broadcast_fn) = mpi_broadcast_fn.clone() {
                        builder = builder.with_mpi_broadcast(move |data| broadcast_fn(data));
                    }

                    (builder.build().expect("Failed to build HintsProcessor"), hints_shmem)
                } else {
                    let hints_file =
                        Arc::new(HintsFile::new(format!("hints_results_{}.bin", local_rank))?);

                    let mut builder = HintsProcessor::builder(
                        hints_file.clone(),
                        Some(inputs_shmem_writer.clone()),
                    )
                    .enable_stats(verbose_mode != proofman_common::VerboseMode::Info);

                    if let Some(broadcast_fn) = mpi_broadcast_fn.clone() {
                        builder = builder.with_mpi_broadcast(move |data| broadcast_fn(data));
                    }

                    (builder.build().expect("Failed to build HintsProcessor"), hints_file)
                };

            if init_rom {
                hints_processor.set_has_rom_sm(true);
            }

            (Some(Arc::new(Mutex::new(ZiskStream::new(hints_processor)))), Some(hints_sink))
        } else {
            (None, None)
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
            hints_sink,
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

    pub fn get_hints_processor(&self) -> Option<Arc<dyn StreamProcessor>> {
        self.hints_stream.as_ref().map(|stream| stream.lock().unwrap().get_processor())
    }

    pub fn reset(&self) {
        if let Some(hints_stream) = &self.hints_stream {
            hints_stream.lock().unwrap().reset();
            self.hints_stream_initialized.store(false, Ordering::SeqCst);
        }
        self.inputs_shmem_writer.reset();
    }

    pub fn config(&self) -> &AsmResourcesConfig {
        &self.config
    }

    pub fn write_input(&self, stdin: &ZiskStdin) -> Result<()> {
        let inputs = stdin.read_bytes();

        self.inputs_shmem_writer.write_input(&inputs)
    }
}
