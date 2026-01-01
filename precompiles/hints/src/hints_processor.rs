//! Precompile Hints Processor
//!
//! This module provides functionality for processing precompile hints
//! that are received as a stream of `u64` values. Hints are used to provide preprocessed
//! data to precompile operations in the ZisK zkVM.

use anyhow::Result;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::collections::{HashMap, VecDeque};
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tracing::debug;
use zisk_common::io::{StreamProcessor, StreamSink};
use zisk_common::{HintCode, PrecompileHint};

/// Ordered result buffer with drain state.
///
/// This structure maintains a VecDeque that holds processed results in order,
/// allowing out-of-order completion while ensuring in-order output.
struct ResultQueue {
    /// The result buffer: None = pending, Some(Ok(...)) = ready, Some(Err(...)) = error
    buffer: VecDeque<Option<Result<Vec<u64>>>>,
    /// Sequence ID of the next result to drain from buffer[0]
    next_drain_seq: usize,
}

/// Thread-safe shared state for parallel hint processing.
struct HintProcessorState {
    /// Ordered results ready for draining
    queue: Mutex<ResultQueue>,
    /// Notifies drainer thread when a hint completes
    drain_signal: Condvar,
    /// Next sequence ID to assign to incoming hints
    next_seq: AtomicUsize,
    /// Signals processing should stop
    error_flag: AtomicBool,
    /// Signals drainer thread to shut down
    shutdown: AtomicBool,
    /// Invalidates stale workers after reset
    generation: AtomicUsize,
}

impl HintProcessorState {
    fn new() -> Self {
        Self {
            queue: Mutex::new(ResultQueue { buffer: VecDeque::new(), next_drain_seq: 0 }),
            drain_signal: Condvar::new(),
            next_seq: AtomicUsize::new(0),
            error_flag: AtomicBool::new(false),
            shutdown: AtomicBool::new(false),
            generation: AtomicUsize::new(0),
        }
    }
}

/// Builder for configuring and constructing a [`HintsProcessor`].
pub struct HintsProcessorBuilder<HS: StreamSink + Send + Sync + 'static> {
    hints_sink: HS,
    num_threads: usize,
    enable_stats: bool,
}

impl<HS: StreamSink + Send + Sync + 'static> HintsProcessorBuilder<HS> {
    /// Sets the number of worker threads in the thread pool.
    pub fn num_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = num_threads;
        self
    }

    /// Enables or disables statistics collection.
    pub fn enable_stats(mut self, enable: bool) -> Self {
        self.enable_stats = enable;
        self
    }

    /// Builds the [`HintsProcessor`] with the configured settings.
    ///
    /// # Returns
    ///
    /// * `Ok(HintsProcessor)` - Successfully constructed processor
    /// * `Err` - If the thread pool fails to initialize
    pub fn build(self) -> Result<HintsProcessor<HS>> {
        let pool = ThreadPoolBuilder::new()
            .num_threads(self.num_threads)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create thread pool: {}", e))?;

        let state = Arc::new(HintProcessorState::new());
        let hints_sink = Arc::new(self.hints_sink);

        // Spawn drainer thread
        let drainer_state = Arc::clone(&state);
        let drainer_sink = Arc::clone(&hints_sink);
        let drainer_thread = std::thread::spawn(move || {
            HintsProcessor::drainer_thread(drainer_state, drainer_sink);
        });

        Ok(HintsProcessor {
            pool,
            state,
            stats: if self.enable_stats { Some(Mutex::new(HashMap::new())) } else { None },
            hints_sink,
            drainer_thread: ManuallyDrop::new(drainer_thread),
        })
    }
}

/// Processor for precompile hints that supports parallel execution.
///
/// This struct provides methods to parse and process a stream of concatenated
/// hints, using a dedicated Rayon thread pool for parallel processing while
/// preserving the original order of results.
pub struct HintsProcessor<HS: StreamSink + Send + Sync + 'static> {
    /// The thread pool used for parallel hint processing.
    pool: ThreadPool,

    /// Shared state for parallel hint processing
    state: Arc<HintProcessorState>,

    /// Optional statistics collected during hint processing (for debugging).
    stats: Option<Mutex<HashMap<HintCode, usize>>>,

    /// The hints sink used to submit processed hints (kept for ownership).
    #[allow(dead_code)]
    hints_sink: Arc<HS>,

    /// Handle to the drainer thread (wrapped in ManuallyDrop to join in Drop)
    drainer_thread: ManuallyDrop<std::thread::JoinHandle<()>>,
}

impl<HS: StreamSink + Send + Sync + 'static> HintsProcessor<HS> {
    const DEFAULT_NUM_THREADS: usize = 32;

    /// Creates a builder for configuring a [`HintsProcessor`].
    ///
    /// # Arguments
    ///
    /// * `hints_sink` - The sink used to submit processed hints
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let processor = HintsProcessor::builder(my_sink)
    ///     .num_threads(16)
    ///     .enable_stats(false)
    ///     .build()?;
    /// ```
    pub fn builder(hints_sink: HS) -> HintsProcessorBuilder<HS> {
        HintsProcessorBuilder {
            hints_sink,
            num_threads: Self::DEFAULT_NUM_THREADS,
            enable_stats: false,
        }
    }

    /// Processes hints in parallel with non-blocking, ordered output.
    ///
    /// This method dispatches each hint to the thread pool for parallel processing.
    /// Results are collected in a reorder buffer and submitted to the sink in the original
    /// order as soon as consecutive results become available.
    ///
    /// # Key characteristics:
    /// - **Non-blocking**: Returns immediately after enqueuing hints
    /// - **Global sequence**: Sequence IDs maintained across multiple batch calls
    /// - **Ordered submission**: Results submitted to sink in order hints were received
    /// - **Error handling**: Stops processing on first error
    ///
    /// # Arguments
    ///
    /// * `hints` - A slice of `u64` values containing concatenated hints
    /// * `first_batch` - Whether this is the first batch (for CTRL_START validation)
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - CTRL_END was encountered
    /// * `Ok(false)` - Batch processed successfully, no CTRL_END
    /// * `Err` - If a previous error occurred or hints are malformed
    pub fn process_hints(&self, hints: &[u64], first_batch: bool) -> Result<bool> {
        let mut has_ctrl_end = false;

        // Parse hints and dispatch to pool
        let mut idx = 0;
        while idx < hints.len() {
            // Check for error before processing each hint
            if self.state.error_flag.load(Ordering::Acquire) {
                return Err(anyhow::anyhow!("Processing stopped due to previous error"));
            }

            let hint = PrecompileHint::from_u64_slice(hints, idx)?;
            let length = hint.data.len();

            if let Some(stats) = &self.stats {
                stats.lock().unwrap().entry(hint.hint_code).and_modify(|c| *c += 1).or_insert(1);
            }

            // Check if this is a control code or data hint type
            match HintCode::try_from(hint.hint_code)? {
                HintCode::CtrlStart => {
                    // CTRL_START must be the first message of the first batch
                    if !first_batch {
                        return Err(anyhow::anyhow!(
                            "CTRL_START can only be sent as the first message in the stream"
                        ));
                    }
                    if idx != 0 {
                        return Err(anyhow::anyhow!(
                            "CTRL_START must be the first hint in the batch, but found at index {}",
                            idx
                        ));
                    }
                    // Reset global sequence and buffer at stream start
                    self.reset();
                    // Control hint only; skip processing
                    idx += length + 1;
                    continue;
                }
                HintCode::CtrlEnd => {
                    // Control hint only; wait for completion then set flag
                    self.wait_for_completion()?;
                    has_ctrl_end = true;
                    idx += length + 1;

                    debug!("CTRL_END received, all hints processed");

                    // CTRL_END should be the last message - verify and break
                    if idx < hints.len() {
                        return Err(anyhow::anyhow!(
                            "CTRL_END must be the last hint, but {} bytes remain",
                            hints.len() - idx
                        ));
                    }
                    break;
                }
                HintCode::CtrlCancel => {
                    // Cancel current stream: set error and notify
                    self.state.error_flag.store(true, Ordering::Release);
                    self.state.drain_signal.notify_all();
                    return Err(anyhow::anyhow!("Stream cancelled"));
                }
                HintCode::CtrlError => {
                    // External error signal
                    self.state.error_flag.store(true, Ordering::Release);
                    self.state.drain_signal.notify_all();
                    return Err(anyhow::anyhow!("Stream error signalled"));
                }
                HintCode::HintsTypeResult | HintCode::HintsTypeEcrecover => {
                    // Data hint type - process normally
                }
            }

            // Atomically reserve slot and capture generation inside mutex
            // This prevents orphaned slots if reset happens between generation load and push_back
            let (generation, seq_id) = {
                let mut queue = self.state.queue.lock().unwrap();
                let gen = self.state.generation.load(Ordering::SeqCst);
                let seq = self.state.next_seq.fetch_add(1, Ordering::SeqCst);
                queue.buffer.push_back(None);
                (gen, seq)
            };

            // Handle HINTS_TYPE_RESULT synchronously - it doesn't need async processing
            if hint.hint_code == HintCode::HintsTypeResult {
                // Immediately mark this slot as complete
                {
                    let mut queue = self.state.queue.lock().unwrap();
                    let offset = seq_id - queue.next_drain_seq;
                    queue.buffer[offset] = Some(Ok(hint.data));
                }

                // Notify drainer thread
                self.state.drain_signal.notify_one();
            } else {
                // Spawn processing task
                let state = Arc::clone(&self.state);
                self.pool.spawn(move || {
                    // Check if we should stop due to error
                    if state.error_flag.load(Ordering::Acquire) {
                        return;
                    }

                    // Process the hint
                    let result = Self::dispatch_hint(hint);

                    // Store result and try to drain
                    let mut queue = state.queue.lock().unwrap();

                    // Check generation first to detect stale workers from previous sessions
                    let current_gen = state.generation.load(Ordering::SeqCst);
                    if generation != current_gen {
                        // Worker belongs to old generation; ignore result
                        return;
                    }

                    // Calculate offset in buffer; handle drained slots
                    if seq_id < queue.next_drain_seq {
                        // This result belongs to a previous stream/session; ignore
                        return;
                    }
                    let offset = seq_id - queue.next_drain_seq;

                    // Check error flag again before storing to avoid processing after error
                    if state.error_flag.load(Ordering::Acquire) {
                        return;
                    }

                    queue.buffer[offset] = Some(result);

                    // Release lock before notifying
                    drop(queue);

                    // Notify drainer thread
                    state.drain_signal.notify_one();
                });
            }

            idx += length + 1;
        }

        if has_ctrl_end {
            if let Some(stats) = &self.stats {
                debug!("Processed hints stats:");
                let stats = stats.lock().unwrap();
                let mut sorted_stats: Vec<_> = stats.iter().collect();
                sorted_stats.sort_by_key(|(hint_code, _)| **hint_code as u32);
                for (hint_code, count) in sorted_stats {
                    debug!("Hint type {}: {}", hint_code, count);
                }
            }
        }

        Ok(has_ctrl_end)
    }

    /// Drainer thread that waits for hints to complete and drains ready results from queue.
    fn drainer_thread(state: Arc<HintProcessorState>, hints_sink: Arc<HS>) {
        loop {
            let mut queue = state.queue.lock().unwrap();

            // Check for shutdown
            if state.shutdown.load(Ordering::Acquire) {
                break;
            }

            // Drain all consecutive ready results from the front
            while let Some(Some(res)) = queue.buffer.front() {
                match res {
                    Ok(data) => {
                        // Clone data before dropping lock
                        let data_to_submit = data.clone();
                        queue.buffer.pop_front();
                        queue.next_drain_seq += 1;

                        // Drop lock before submitting to avoid blocking workers
                        drop(queue);

                        // Submit to sink
                        if let Err(e) = hints_sink.submit(data_to_submit) {
                            eprintln!("Error submitting to sink: {}", e);
                            state.error_flag.store(true, Ordering::Release);
                            state.drain_signal.notify_all();
                            return;
                        }

                        // Re-acquire lock for next iteration
                        queue = state.queue.lock().unwrap();
                    }
                    Err(e) => {
                        // Error found - signal to stop
                        state.error_flag.store(true, Ordering::Release);
                        eprintln!("[seq={}] Error: {}", queue.next_drain_seq, e);
                        queue.buffer.pop_front();
                        queue.next_drain_seq += 1;
                        state.drain_signal.notify_all();
                        return;
                    }
                }
            }

            // Notify waiters if buffer is now empty
            if queue.buffer.is_empty() {
                state.drain_signal.notify_all();
            }

            // Wait for notification that a hint completed
            #[allow(unused_assignments)]
            {
                queue = state.drain_signal.wait(queue).unwrap();
            }
        }
    }

    /// Waits for all pending hints to be processed and drained.
    ///
    /// This method blocks until the reorder buffer is empty, meaning all
    /// dispatched hints have been processed and their results printed.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - All hints processed successfully
    /// * `Err` - If an error occurred during processing
    fn wait_for_completion(&self) -> Result<()> {
        let mut queue = self.state.queue.lock().unwrap();

        while !queue.buffer.is_empty() {
            if self.state.error_flag.load(Ordering::Acquire) {
                return Err(anyhow::anyhow!("Processing stopped due to error"));
            }
            // Wait for notification that buffer state changed
            queue = self.state.drain_signal.wait(queue).unwrap();
        }

        if self.state.error_flag.load(Ordering::Acquire) {
            return Err(anyhow::anyhow!("Processing stopped due to error"));
        }

        Ok(())
    }

    /// Resets the processor state, clearing any errors and the reorder buffer.
    ///
    /// This should be called to start a fresh processing session after an error
    /// or when you want to reset the global sequence counter.
    ///
    /// Increments the generation counter to invalidate any in-flight workers
    /// from the previous session, preventing them from corrupting the new state.
    fn reset(&self) {
        self.state.error_flag.store(false, Ordering::Release);
        self.state.next_seq.store(0, Ordering::Release);
        // Increment generation to invalidate stale workers
        self.state.generation.fetch_add(1, Ordering::SeqCst);
        let mut queue = self.state.queue.lock().unwrap();
        queue.buffer.clear();
        queue.next_drain_seq = 0;
    }

    /// Dispatches a single hint to its appropriate handler based on hint type.
    ///
    /// # Arguments
    ///
    /// * `hint` - The parsed hint to dispatch
    ///
    /// # Returns
    ///
    /// The result produced by the selected hint handler.
    ///
    /// # Errors
    ///
    /// Returns an error if the hint type is unknown or if the handler fails.
    #[inline]
    fn dispatch_hint(hint: PrecompileHint) -> Result<Vec<u64>> {
        match hint.hint_code {
            // Control codes should not reach here
            HintCode::CtrlStart
            | HintCode::CtrlEnd
            | HintCode::CtrlCancel
            | HintCode::CtrlError => {
                Err(anyhow::anyhow!("Control code {:?} should not be dispatched", hint.hint_code))
            }

            // When hint type is HINTS_TYPE_RESULT, return the data as-is.
            HintCode::HintsTypeResult => Ok(hint.data),

            // Dispatch to the ECRECOVER handler.
            HintCode::HintsTypeEcrecover => Self::process_hint_ecrecover(&hint),
        }
    }

    /// Processes a [`HINTS_TYPE_ECRECOVER`] hint.
    #[inline]
    fn process_hint_ecrecover(hint: &PrecompileHint) -> Result<Vec<u64>> {
        ziskos_hints::hints::process_ecrecover_hint(&hint.data).map_err(|e| anyhow::anyhow!(e))
    }
}

impl<HS: StreamSink + Send + Sync + 'static> Drop for HintsProcessor<HS> {
    fn drop(&mut self) {
        // Signal drainer thread to shut down
        self.state.shutdown.store(true, Ordering::Release);
        self.state.drain_signal.notify_all();

        // Join the drainer thread to ensure clean shutdown
        // Safety: We only take the value once in drop
        unsafe {
            let handle = ManuallyDrop::take(&mut self.drainer_thread);
            let _ = handle.join();
        }
    }
}

impl<HS: StreamSink + Send + Sync + 'static> StreamProcessor for HintsProcessor<HS> {
    fn process(&self, data: &[u64], first_batch: bool) -> Result<bool> {
        self.process_hints(data, first_batch)
    }
}

#[cfg(test)]
mod tests {
    use zisk_common::HintCode;

    use super::*;

    struct NullHints;

    impl StreamSink for NullHints {
        fn submit(&self, _processed: Vec<u64>) -> Result<()> {
            Ok(())
        }
    }

    fn make_header(hint_type: u32, length: u32) -> u64 {
        ((hint_type as u64) << 32) | (length as u64)
    }

    fn make_ctrl_header(ctrl: u32, length: u32) -> u64 {
        make_header(ctrl, length)
    }

    fn processor() -> HintsProcessor<NullHints> {
        HintsProcessor::builder(NullHints).num_threads(2).build().unwrap()
    }

    // Positive tests
    #[test]
    fn test_single_result_hint_non_blocking() {
        let p = processor();
        let data = vec![make_header(HintCode::HintsTypeResult as u32, 2), 0x111, 0x222];

        // Dispatch should succeed and be non-blocking
        assert!(p.process_hints(&data, false).is_ok());
        // Wait for completion should succeed
        assert!(p.wait_for_completion().is_ok());

        // Buffer should be empty after completion
        let queue = p.state.queue.lock().unwrap();
        assert!(queue.buffer.is_empty());
        assert_eq!(queue.next_drain_seq, 1);
    }

    #[test]
    fn test_multiple_hints_ordered_output() {
        let p = processor();
        let data = vec![
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x111,
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x222,
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x333,
        ];
        assert!(p.process_hints(&data, false).is_ok());
        assert!(p.wait_for_completion().is_ok());

        // Verify all hints were processed (buffer empty, next_drain_seq advanced)
        let queue = p.state.queue.lock().unwrap();
        assert!(queue.buffer.is_empty());
        assert_eq!(queue.next_drain_seq, 3);
    }

    #[test]
    fn test_multiple_calls_global_sequence() {
        let p = processor();
        let data1 = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0xAAA];
        let data2 = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0xBBB];

        assert!(p.process_hints(&data1, false).is_ok());
        assert!(p.process_hints(&data2, false).is_ok());
        assert!(p.wait_for_completion().is_ok());

        // Verify sequence continued across calls
        let queue = p.state.queue.lock().unwrap();
        assert_eq!(queue.next_drain_seq, 2);
        assert!(queue.buffer.is_empty());
    }

    #[test]
    fn test_empty_input_ok() {
        let p = processor();
        let data: Vec<u64> = vec![];
        assert!(p.process_hints(&data, false).is_ok());
        assert!(p.wait_for_completion().is_ok());

        // No hints processed
        let queue = p.state.queue.lock().unwrap();
        assert_eq!(queue.next_drain_seq, 0);
    }

    // Negative tests
    #[test]
    fn test_unknown_hint_type_returns_error() {
        let p = processor();
        let data = vec![make_header(999, 1), 0x1234];

        // Should return error immediately during validation
        let result = p.process_hints(&data, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid hint code"));
    }

    #[test]
    fn test_error_stops_wait() {
        let p = processor();
        // First valid, then invalid type
        let data =
            vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x111, make_header(999, 0)];

        // Should error immediately when encountering invalid hint type
        let result = p.process_hints(&data, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid hint code"));
    }

    #[test]
    fn test_reset_clears_error() {
        let p = processor();
        let bad = vec![make_header(999, 0)];
        let result = p.process_hints(&bad, false);

        // Should get synchronous error for invalid hint type
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid hint code"));

        // Reset should clear any error state
        p.reset();
        assert!(!p.state.error_flag.load(Ordering::Acquire));

        // Should be able to process new hints after reset
        let good = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x42];
        assert!(p.process_hints(&good, false).is_ok());
        assert!(p.wait_for_completion().is_ok());

        let queue = p.state.queue.lock().unwrap();
        assert_eq!(queue.next_drain_seq, 1);
    }

    // Stream control tests
    #[test]
    fn test_stream_start_resets_state() {
        let p = processor();

        // First batch increments sequence
        let batch1 = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x01];
        p.process_hints(&batch1, false).unwrap();
        p.wait_for_completion().unwrap();

        // Sequence should be at 1
        {
            let queue = p.state.queue.lock().unwrap();
            assert_eq!(queue.next_drain_seq, 1);
        }

        // Send START control - should reset sequence
        let start = vec![make_ctrl_header(HintCode::CtrlStart as u32, 0)];
        p.process_hints(&start, true).unwrap();

        // Sequence should be reset to 0
        {
            let queue = p.state.queue.lock().unwrap();
            assert_eq!(queue.next_drain_seq, 0);
            assert!(queue.buffer.is_empty());
        }

        // Process new batch
        let batch2 = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x02];
        p.process_hints(&batch2, false).unwrap();

        let end = vec![make_ctrl_header(HintCode::CtrlEnd as u32, 0)];
        p.process_hints(&end, false).unwrap();

        // Should have processed 1 hint (starting from 0 again)
        let queue = p.state.queue.lock().unwrap();
        assert_eq!(queue.next_drain_seq, 1);
    }

    #[test]
    fn test_stream_end_waits_until_completion() {
        let p = processor();

        // Dispatch hints
        let data = vec![
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x10,
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x20,
        ];
        p.process_hints(&data, false).unwrap();

        // END should wait internally
        let end = vec![make_ctrl_header(HintCode::CtrlEnd as u32, 0)];
        p.process_hints(&end, false).unwrap();

        // Buffer should already be empty
        {
            let queue = p.state.queue.lock().unwrap();
            assert!(queue.buffer.is_empty());
            assert_eq!(queue.next_drain_seq, 2);
        }

        // Explicit wait should be instant
        assert!(p.wait_for_completion().is_ok());
    }

    #[test]
    fn test_stream_cancel_returns_error() {
        let p = processor();
        let cancel = vec![make_ctrl_header(HintCode::CtrlCancel as u32, 0)];

        let result = p.process_hints(&cancel, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));

        // Error flag should be set
        assert!(p.state.error_flag.load(Ordering::Acquire));
    }

    #[test]
    fn test_stream_error_signal_returns_error() {
        let p = processor();
        let signal_err = vec![make_ctrl_header(HintCode::CtrlError as u32, 0)];

        let result = p.process_hints(&signal_err, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("error"));

        // Error flag should be set
        assert!(p.state.error_flag.load(Ordering::Acquire));
    }

    // Builder tests
    #[test]
    fn test_builder_default() {
        let p = HintsProcessor::builder(NullHints).build().unwrap();

        // Should have stats disabled by default
        assert!(p.stats.is_none());

        // Should process hints normally
        let data = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x42];
        assert!(p.process_hints(&data, false).is_ok());
        assert!(p.wait_for_completion().is_ok());
    }

    #[test]
    fn test_builder_custom_threads() {
        let p = HintsProcessor::builder(NullHints).num_threads(4).build().unwrap();

        // Should process hints normally
        let data = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x42];
        assert!(p.process_hints(&data, false).is_ok());
        assert!(p.wait_for_completion().is_ok());
    }

    #[test]
    fn test_builder_stats_disabled() {
        let p = HintsProcessor::builder(NullHints).enable_stats(false).build().unwrap();

        // Stats should be None
        assert!(p.stats.is_none());

        // Should still process hints normally
        let data = vec![
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x111,
            make_header(HintCode::HintsTypeResult as u32, 1),
            0x222,
        ];
        assert!(p.process_hints(&data, false).is_ok());
        assert!(p.wait_for_completion().is_ok());
    }

    #[test]
    fn test_builder_chaining() {
        let p =
            HintsProcessor::builder(NullHints).num_threads(8).enable_stats(true).build().unwrap();

        assert!(p.stats.is_some());

        let data = vec![make_header(HintCode::HintsTypeResult as u32, 1), 0x42];
        assert!(p.process_hints(&data, false).is_ok());
        assert!(p.wait_for_completion().is_ok());
    }

    // Stress test
    #[test]
    fn test_stress_throughput() {
        use std::time::Instant;

        let p = HintsProcessor::builder(NullHints).num_threads(32).build().unwrap();

        // Generate a large batch of hints
        const NUM_HINTS: usize = 100_000;
        let mut data = Vec::with_capacity(NUM_HINTS * 2);

        for i in 0..NUM_HINTS {
            data.push(make_header(HintCode::HintsTypeResult as u32, 1));
            data.push(i as u64);
        }

        let start = Instant::now();
        p.process_hints(&data, false).unwrap();
        p.wait_for_completion().unwrap();
        let duration = start.elapsed();

        let ops_per_sec = NUM_HINTS as f64 / duration.as_secs_f64();
        println!("\n========================================");
        println!("Stress Test Results:");
        println!("  Total hints: {}", NUM_HINTS);
        println!("  Duration: {:.3}s", duration.as_secs_f64());
        println!("  Throughput: {:.0} ops/sec", ops_per_sec);
        println!("  Avg latency: {:.2}µs per hint", duration.as_micros() as f64 / NUM_HINTS as f64);
        println!("========================================\n");

        // Sanity check: should be able to process at least 10k ops/sec
        assert!(ops_per_sec > 10_000.0, "Throughput too low: {:.0} ops/sec", ops_per_sec);
    }

    #[test]
    fn test_stress_concurrent_batches() {
        use std::time::Instant;

        let p = HintsProcessor::builder(NullHints).num_threads(32).build().unwrap();

        const NUM_BATCHES: usize = 1_000;
        const HINTS_PER_BATCH: usize = 100;

        let start = Instant::now();

        // Call process_hints multiple times with small batches
        for batch_id in 0..NUM_BATCHES {
            let mut data = Vec::with_capacity(HINTS_PER_BATCH * 2);
            for i in 0..HINTS_PER_BATCH {
                data.push(make_header(HintCode::HintsTypeResult as u32, 1));
                data.push((batch_id * HINTS_PER_BATCH + i) as u64);
            }
            p.process_hints(&data, false).unwrap();
        }

        p.wait_for_completion().unwrap();
        let duration = start.elapsed();

        let total_hints = NUM_BATCHES * HINTS_PER_BATCH;
        let ops_per_sec = total_hints as f64 / duration.as_secs_f64();

        println!("\n========================================");
        println!("Multiple Batches Stress Test:");
        println!("  Number of batches: {}", NUM_BATCHES);
        println!("  Hints per batch: {}", HINTS_PER_BATCH);
        println!("  Total hints: {}", total_hints);
        println!("  Duration: {:.3}s", duration.as_secs_f64());
        println!("  Throughput: {:.0} ops/sec", ops_per_sec);
        println!("========================================\n");

        assert!(ops_per_sec > 10_000.0, "Throughput too low: {:.0} ops/sec", ops_per_sec);
    }

    #[test]
    fn test_stress_with_resets() {
        use std::time::Instant;

        let p = HintsProcessor::builder(NullHints).num_threads(32).build().unwrap();

        const ITERATIONS: usize = 100;
        const HINTS_PER_ITER: usize = 1_000;

        let start = Instant::now();

        for _iter in 0..ITERATIONS {
            // Reset at start of each iteration
            let reset = vec![make_ctrl_header(HintCode::CtrlStart as u32, 0)];
            p.process_hints(&reset, true).unwrap();

            // Process batch
            let mut data = Vec::with_capacity(HINTS_PER_ITER * 2);
            for i in 0..HINTS_PER_ITER {
                data.push(make_header(HintCode::HintsTypeResult as u32, 1));
                data.push(i as u64);
            }
            p.process_hints(&data, false).unwrap();

            // End stream
            let end = vec![make_ctrl_header(HintCode::CtrlEnd as u32, 0)];
            p.process_hints(&end, false).unwrap();
        }

        let duration = start.elapsed();
        let total_hints = ITERATIONS * HINTS_PER_ITER;
        let ops_per_sec = total_hints as f64 / duration.as_secs_f64();

        println!("\n========================================");
        println!("Reset Stress Test:");
        println!("  Iterations: {}", ITERATIONS);
        println!("  Hints per iteration: {}", HINTS_PER_ITER);
        println!("  Total hints: {}", total_hints);
        println!("  Duration: {:.3}s", duration.as_secs_f64());
        println!("  Throughput: {:.0} ops/sec", ops_per_sec);
        println!("========================================\n");

        assert!(
            ops_per_sec > 5_000.0,
            "Throughput too low with resets: {:.0} ops/sec",
            ops_per_sec
        );
    }
}
