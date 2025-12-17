//! HintsStream is responsible for reading precompile hints from a stream source and sent to a hints processor.

use crate::HintsProcessor;
use anyhow::Result;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tracing::debug;
use zisk_common::io::{StreamRead, StreamSource};

enum ThreadCommand {
    Process,
    Shutdown,
}

/// HintsStream struct manages the processing of precompile hints and writing them to shared memory.
pub struct HintsStream<HP: HintsProcessor + Send + Sync + 'static> {
    /// The hints processor used to process hints before writing.
    hints_processor: Arc<HP>,

    /// Channel sender to communicate with the background thread.
    tx: Option<Sender<ThreadCommand>>,

    /// Join handle for the background thread.
    thread_handle: Option<JoinHandle<()>>,
}

impl<HP: HintsProcessor + Send + Sync + 'static> HintsStream<HP> {
    /// Create a new HintsStream with the given processor.
    ///
    /// # Arguments
    /// * `hints_processor` - The processor used to process hints.
    ///
    /// # Returns
    /// A new `HintsStream` instance without a running thread.
    pub fn new(hints_processor: HP) -> Self {
        Self { hints_processor: Arc::new(hints_processor), tx: None, thread_handle: None }
    }

    /// Stop the current background thread if running.
    fn stop_thread(&mut self) {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(ThreadCommand::Shutdown);
        }
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }

    /// Set a new StreamSource for the pipeline and spawn a background thread to process hints.
    ///
    /// This will stop any existing background thread and start a new one with the new stream.
    ///
    /// # Arguments
    /// * `stream` - The new StreamSource source for reading hints.
    pub fn set_hints_stream(&mut self, mut stream: StreamSource) -> Result<()> {
        if !stream.is_active() {
            // Stop the existing thread if running
            self.stop_thread();
            stream.open()?;
        }

        // Create a new channel for communication with the thread
        let (tx, rx) = std::sync::mpsc::channel();
        self.tx = Some(tx);

        // Clone Arc references for the thread
        let hints_processor = Arc::clone(&self.hints_processor);

        // Spawn the background thread
        let thread_handle = thread::spawn(move || {
            Self::background_thread(stream, hints_processor, rx);
        });

        self.thread_handle = Some(thread_handle);

        Ok(())
    }

    /// Background thread function that processes hints when requested.
    fn background_thread(
        mut stream: StreamSource,
        hints_processor: Arc<HP>,
        rx: Receiver<ThreadCommand>,
    ) {
        loop {
            match rx.recv() {
                Ok(ThreadCommand::Process) => {
                    if let Err(e) = Self::process_stream(&mut stream, &hints_processor) {
                        tracing::error!("Error processing hints in background thread: {:?}", e);
                    }
                }
                Ok(ThreadCommand::Shutdown) | Err(_) => {
                    // Channel closed or shutdown requested
                    break;
                }
            }
        }
    }

    /// Process all hints from the stream.
    ///
    /// Processes hints in batches until CTRL_END is encountered or the stream ends.
    fn process_stream(stream: &mut StreamSource, hints_processor: &HP) -> Result<()> {
        let mut first_batch = true;

        while let Some(hints) = stream.next()? {
            let hints = zisk_common::reinterpret_vec(hints)?;
            let has_ctrl_end = hints_processor.process_hints(&hints, first_batch)?;

            first_batch = false;

            // Break if CTRL_END was encountered
            if has_ctrl_end {
                debug!("CTRL_END encountered, stopping hint processing");
                break;
            }
        }

        Ok(())
    }

    /// Trigger the background thread to process hints asynchronously.
    ///
    /// This method:
    /// 1. Sends a command to the background thread to process hints
    /// 2. Returns immediately without waiting for processing to complete
    ///
    /// # Returns
    /// * `Ok(())` - If the command was successfully sent
    /// * `Err` - If there's no active thread or the channel is closed
    pub fn start_stream(&mut self) -> Result<()> {
        if let Some(tx) = &self.tx {
            tx.send(ThreadCommand::Process).map_err(|e| {
                anyhow::anyhow!("Failed to send process command to background thread: {}", e)
            })?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("No background thread running. Call set_hints_stream first."))
        }
    }
}

impl<HP: HintsProcessor + Send + Sync> Drop for HintsStream<HP> {
    fn drop(&mut self) {
        self.stop_thread();
    }
}
