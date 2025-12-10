//! Precompile Hints Processor
//!
//! This module provides functionality for parsing and processing precompile hints
//! that are received as a stream of `u64` values. Hints are used to provide preprocessed
//! data to precompile operations in the ZisK zkVM.
//!
//! # Hint Format
//!
//! Each hint consists of:
//! - A **header** (`u64`): Contains the hint type (upper 32 bits) and data length (lower 32 bits)
//! - **Data** (`[u64; length]`): The hint payload, where `length` is specified in the header
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                         Header (u64)                           │
//! ├────────────────────────────────┬───────────────────────────────┤
//! │      Hint Code (32 bits)       │       Length (32 bits)        │
//! ├────────────────────────────────┴───────────────────────────────┤
//! │                      Data[0] (u64)                             │
//! ├────────────────────────────────────────────────────────────────┤
//! │                      Data[1] (u64)                             │
//! ├────────────────────────────────────────────────────────────────┤
//! │                         ...                                    │
//! ├────────────────────────────────────────────────────────────────┤
//! │                      Data[length-1] (u64)                      │
//! └────────────────────────────────────────────────────────────────┘
//!
//! - Hint Code — Control code or Data Hint Type
//! - Length — Number of following u64 data words
//!
//! ## Hint Type Layout
//!
//! ### Control codes
//!
//! The following control codes are defined:
//! - `0x00` (START): Reset processor state and global sequence.
//! - `0x01` (END): Wait until completion of all pending hints.
//! - `0x02` (CANCEL): Cancel current stream and stop processing further hints.
//! - `0x03` (ERROR): Indicate an error has occurred; stop processing further hints.
//!
//! Control codes are for control only and do not have any associated data (Length should be zero).
//!
//! ### Data Hint Types:
//! - `0x04` (`HINTS_TYPE_RESULT`): Pass-through data
//! - `0x05` (`HINTS_TYPE_ECRECOVER`): ECRECOVER inputs (currently returns empty)
//! ```

use anyhow::Result;
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use tracing::debug;
use ziskos::syscalls::SyscallPoint256;

use crate::{secp256k1_ecdsa_verify, HintsProcessor, HintsSink};

// TODO! COnvert Control Code to an enum and HINT TYPE to an enum as well

/// Control code: Reset processor state and global sequence.
const CTRL_START: u32 = 0x00;

/// Control code: Wait until completion of all pending hints.
const CTRL_END: u32 = 0x01;

/// Control code: Cancel current stream and stop processing.
const CTRL_CANCEL: u32 = 0x02;

/// Control code: Signal error and stop processing.
const CTRL_ERROR: u32 = 0x03;

/// Hint type indicating that the data is already the precomputed result.
///
/// When a hint has this type, the processor simply passes through the data
/// without any additional computation.
pub const HINTS_TYPE_RESULT: u32 = 0x04;

/// Hint type indicating that the data contains inputs for the ecrecover precompile.
pub const HINTS_TYPE_ECRECOVER: u32 = 0x05;

/// Number if hint types defined.
pub const NUM_HINT_TYPES: u32 = 6;

/// Represents a single precompile hint parsed from a `u64` slice.
///
/// A hint consists of a type identifier and associated data. The hint type
/// determines how the data should be processed by the [`PrecompileHintsProcessor`].
pub struct PrecompileHint {
    /// The type of hint, determining how the data should be processed.
    hint_type: u32,
    /// The hint payload data.
    data: Vec<u64>,
}

impl std::fmt::Debug for PrecompileHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileHint")
            .field("hint_type", &self.hint_type)
            .field("data", &self.data)
            .finish()
    }
}

impl PrecompileHint {
    /// Parses a [`PrecompileHint`] from a slice of `u64` values at the given index.
    ///
    /// # Arguments
    ///
    /// * `slice` - The source slice containing concatenated hints
    /// * `idx` - The index where the hint header starts
    ///
    /// # Returns
    ///
    /// * `Ok(PrecompileHint)` - Successfully parsed hint
    /// * `Err` - If the slice is too short or the index is out of bounds
    #[inline(always)]
    fn from_u64_slice(slice: &[u64], idx: usize) -> Result<Self> {
        if slice.is_empty() || idx >= slice.len() {
            return Err(anyhow::anyhow!("Slice too short or index out of bounds"));
        }

        let header = slice[idx];
        let hint_type = (header >> 32) as u32;
        let length = (header & 0xFFFFFFFF) as u32;

        if slice.len() < idx + length as usize + 1 {
            return Err(anyhow::anyhow!(
                "Slice too short for hint data: expected {}, got {}",
                length,
                slice.len() - idx - 1
            ));
        }

        // TODO! This creates a new Vec to own the data. Sine performance is critical,
        // TODO! consider using a slice reference instead.
        let data = slice[idx + 1..idx + length as usize + 1].to_vec();

        Ok(PrecompileHint { hint_type, data })
    }
}

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
    /// Notifies when queue becomes empty or error occurs
    drain_signal: Condvar,
    /// Next sequence ID to assign to incoming hints
    next_seq: AtomicUsize,
    /// Signals processing should stop
    error_flag: AtomicBool,
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
            generation: AtomicUsize::new(0),
        }
    }
}

/// Processor for precompile hints that supports parallel execution.
///
/// This struct provides methods to parse and process a stream of concatenated
/// hints, using a dedicated Rayon thread pool for parallel processing while
/// preserving the original order of results.
pub struct PrecompileHintsProcessor<HS: HintsSink + Send + Sync> {
    /// The thread pool used for parallel hint processing.
    pool: ThreadPool,

    /// Shared state for parallel hint processing
    state: Arc<HintProcessorState>,

    /// Optional statistics collected during hint processing.
    stats: [AtomicUsize; NUM_HINT_TYPES as usize],

    /// The hints sink used to submit processed hints.
    hints_sink: Arc<HS>,
}

impl<HS: HintsSink + Send + Sync> PrecompileHintsProcessor<HS> {
    const DEFAULT_NUM_THREADS: usize = 32;

    /// Creates a new processor with the default number of threads.
    ///
    /// The default is 32 threads.
    ///
    /// # Returns
    ///
    /// * `Ok(PrecompileHintsProcessor)` - The configured processor
    /// * `Err` - If the thread pool fails to initialize
    pub fn new(hints_sink: HS) -> Result<Self> {
        Self::with_num_threads(Self::DEFAULT_NUM_THREADS, hints_sink)
    }

    /// Creates a new processor with the specified number of threads.
    ///
    /// # Arguments
    ///
    /// * `num_threads` - The number of worker threads in the pool
    /// * `hints_sink` - The sink used to submit processed hints
    ///
    /// # Returns
    ///
    /// * `Ok(PrecompileHintsProcessor)` - The configured processor
    /// * `Err` - If the thread pool fails to initialize
    pub fn with_num_threads(num_threads: usize, hints_sink: HS) -> Result<Self> {
        let pool = ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to create thread pool: {}", e))?;

        Ok(Self {
            pool,
            state: Arc::new(HintProcessorState::new()),
            stats: Default::default(),
            hints_sink: Arc::new(hints_sink),
        })
    }

    /// Processes hints in parallel with non-blocking, ordered output.
    ///
    /// This method dispatches each hint to the thread pool for parallel processing.
    /// Results are collected in a reorder buffer and drained (printed!!!!!!!!!!!!!!!!!!!!!) in the original
    /// order as soon as consecutive results become available.
    ///
    /// # Key characteristics:
    /// - **Non-blocking**: Returns immediately after dispatching work to the pool
    /// - **Global sequence**: Sequence IDs are maintained across multiple calls
    /// - **Ordered output**: Results are printed!!!!!!!!!!!!!!!!!!!! in the order hints were received
    /// - **Error handling**: Stops processing on first error
    ///
    /// # Arguments
    ///
    /// * `hints` - A slice of `u64` values containing concatenated hints
    ///
    /// # Returns
    ///
    /// * `Ok((Vec<u64>, bool))` - Tuple of (processed data, has_ctrl_end) where has_ctrl_end is true if CTRL_END was encountered
    /// * `Err` - If a previous error occurred or hints are malformed
    pub fn process_hints(&self, hints: &[u64], first_batch: bool) -> Result<bool> {
        let mut processed = Vec::new();
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

            // Validate hint type is in valid range before accessing stats array
            if hint.hint_type >= NUM_HINT_TYPES {
                return Err(anyhow::anyhow!("Invalid hint type: {}", hint.hint_type));
            }

            self.stats[hint.hint_type as usize].fetch_add(1, Ordering::Relaxed);

            // Check if this is a control code or data hint type
            match hint.hint_type {
                CTRL_START => {
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
                CTRL_END => {
                    // Control hint only; wait for completion then set flag
                    self.wait_for_completion()?;
                    has_ctrl_end = true;
                    idx += length + 1;

                    // CTRL_END should be the last message - verify and break
                    if idx < hints.len() {
                        return Err(anyhow::anyhow!(
                            "CTRL_END must be the last hint, but {} bytes remain",
                            hints.len() - idx
                        ));
                    }
                    break;
                }
                CTRL_CANCEL => {
                    // Cancel current stream: set error and notify
                    self.state.error_flag.store(true, Ordering::Release);
                    self.state.drain_signal.notify_all();
                    return Err(anyhow::anyhow!("Stream cancelled"));
                }
                CTRL_ERROR => {
                    // External error signal
                    self.state.error_flag.store(true, Ordering::Release);
                    self.state.drain_signal.notify_all();
                    return Err(anyhow::anyhow!("Stream error signalled"));
                }
                _ => {
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
            if hint.hint_type == HINTS_TYPE_RESULT {
                processed.extend_from_slice(&hint.data);

                // Immediately mark this slot as complete and drain
                let mut queue = self.state.queue.lock().unwrap();
                let offset = seq_id - queue.next_drain_seq;
                queue.buffer[offset] = Some(Ok(hint.data.clone()));

                // Drain consecutive ready results from the front
                while let Some(Some(res)) = queue.buffer.front() {
                    match res {
                        Ok(_data) => {
                            queue.buffer.pop_front();
                            queue.next_drain_seq += 1;
                        }
                        Err(_) => {
                            self.state.error_flag.store(true, Ordering::Release);
                            if let Some(Some(Err(e))) = queue.buffer.pop_front() {
                                eprintln!("[seq={}] Error: {}", queue.next_drain_seq, e);
                            }
                            queue.next_drain_seq += 1;
                            self.state.drain_signal.notify_all();
                            break;
                        }
                    }
                }

                // Notify if buffer is now empty
                if queue.buffer.is_empty() {
                    self.state.drain_signal.notify_all();
                }
            } else {
                // Spawn processing task
                let state = Arc::clone(&self.state);
                self.pool.spawn(move || {
                    // TODO! Is it necessary? TO increase performance maybe is enough to check error_flag only when storing result
                    // Check if we should stop due to error
                    if state.error_flag.load(Ordering::Acquire) {
                        return;
                    }

                    // Process the hint
                    let result = Self::process_hint(&hint);

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

                    // Drain consecutive ready results from the front
                    while let Some(Some(res)) = queue.buffer.front() {
                        match res {
                            Ok(_data) => {
                                // Print the result (will be replaced with send to another process)
                                // println!("[seq={}] Result: {:?}", queue.next_drain_seq, data);
                                queue.buffer.pop_front();
                                queue.next_drain_seq += 1;
                            }
                            Err(_) => {
                                // Error found - signal to stop and break
                                state.error_flag.store(true, Ordering::Release);
                                // Print error and stop draining
                                if let Some(Some(Err(e))) = queue.buffer.pop_front() {
                                    eprintln!("[seq={}] Error: {}", queue.next_drain_seq, e);
                                }
                                queue.next_drain_seq += 1;
                                state.drain_signal.notify_all();
                                break;
                            }
                        }
                    }

                    // Notify if buffer is now empty
                    if queue.buffer.is_empty() {
                        state.drain_signal.notify_all();
                    }
                });
            }

            idx += length + 1;
        }

        debug!("Processed hints stats:");
        for (i, count) in self.stats.iter().enumerate() {
            debug!("Hint type {}: {}", i, count.load(Ordering::Relaxed));
        }

        if !processed.is_empty() {
            self.hints_sink.submit(processed)?;
        }

        Ok(has_ctrl_end)
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
    /// * `hint` - The parsed hint to process
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u64>)` - The processed result for this hint
    /// * `Err` - If the hint type is unknown
    fn process_hint(hint: &PrecompileHint) -> Result<Vec<u64>> {
        let result = match hint.hint_type {
            HINTS_TYPE_RESULT => Self::process_hint_result(hint)?,
            HINTS_TYPE_ECRECOVER => Self::process_hint_ecrecover(hint)?,
            _ => {
                return Err(anyhow::anyhow!("Unknown hint type: {}", hint.hint_type));
            }
        };

        Ok(result)
    }

    /// Processes a [`HINTS_TYPE_RESULT`] hint.
    ///
    /// This is a pass-through handler that simply returns the hint data as-is.
    /// Used when the hint already contains the precomputed result.
    fn process_hint_result(hint: &PrecompileHint) -> Result<Vec<u64>> {
        Ok(hint.data.to_vec())
    }

    /// Processes a [`HINTS_TYPE_ECRECOVER`] hint.
    fn process_hint_ecrecover(hint: &PrecompileHint) -> Result<Vec<u64>> {
        const EXPECTED_LEN: usize = 8 + 4 + 4 + 4; // pk(8) + z(4) + r(4) + s(4)

        if hint.data.len() != EXPECTED_LEN {
            return Err(anyhow::anyhow!(
                "Invalid ECRECOVER hint length: expected {}, got {}",
                EXPECTED_LEN,
                hint.data.len()
            ));
        }

        let mut processed_hints = Vec::new();

        // Safety: We've validated the length above
        unsafe {
            let ptr = hint.data.as_ptr();
            let pk = &*(ptr as *const SyscallPoint256);
            let z = &*(ptr.add(8) as *const [u64; 4]);
            let r = &*(ptr.add(12) as *const [u64; 4]);
            let s = &*(ptr.add(16) as *const [u64; 4]);

            secp256k1_ecdsa_verify(pk, z, r, s, &mut processed_hints);
        }

        Ok(processed_hints)
    }
}

impl<HS: HintsSink + Send + Sync> HintsProcessor for PrecompileHintsProcessor<HS> {
    fn process_hints(&self, hints: &[u64], first_batch: bool) -> Result<bool> {
        self.process_hints(hints, first_batch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NullHints;

    impl HintsSink for NullHints {
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

    fn processor() -> PrecompileHintsProcessor<NullHints> {
        PrecompileHintsProcessor::with_num_threads(2, NullHints).unwrap()
    }

    // Positive tests
    #[test]
    fn test_single_result_hint_non_blocking() {
        let p = processor();
        let data = vec![make_header(HINTS_TYPE_RESULT, 2), 0x111, 0x222];

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
            make_header(HINTS_TYPE_RESULT, 1),
            0x111,
            make_header(HINTS_TYPE_RESULT, 1),
            0x222,
            make_header(HINTS_TYPE_RESULT, 1),
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
        let data1 = vec![make_header(HINTS_TYPE_RESULT, 1), 0xAAA];
        let data2 = vec![make_header(HINTS_TYPE_RESULT, 1), 0xBBB];

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
        assert!(result.unwrap_err().to_string().contains("Invalid hint type"));
    }

    #[test]
    fn test_error_stops_wait() {
        let p = processor();
        // First valid, then invalid type
        let data = vec![make_header(HINTS_TYPE_RESULT, 1), 0x111, make_header(999, 0)];

        // Should error immediately when encountering invalid hint type
        let result = p.process_hints(&data, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid hint type"));
    }

    #[test]
    fn test_reset_clears_error() {
        let p = processor();
        let bad = vec![make_header(999, 0)];
        let result = p.process_hints(&bad, false);

        // Should get synchronous error for invalid hint type
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid hint type"));

        // Reset should clear any error state
        p.reset();
        assert!(!p.state.error_flag.load(Ordering::Acquire));

        // Should be able to process new hints after reset
        let good = vec![make_header(HINTS_TYPE_RESULT, 1), 0x42];
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
        let batch1 = vec![make_header(HINTS_TYPE_RESULT, 1), 0x01];
        p.process_hints(&batch1, false).unwrap();
        p.wait_for_completion().unwrap();

        // Sequence should be at 1
        {
            let queue = p.state.queue.lock().unwrap();
            assert_eq!(queue.next_drain_seq, 1);
        }

        // Send START control - should reset sequence
        let start = vec![make_ctrl_header(CTRL_START, 0)];
        p.process_hints(&start, true).unwrap();

        // Sequence should be reset to 0
        {
            let queue = p.state.queue.lock().unwrap();
            assert_eq!(queue.next_drain_seq, 0);
            assert!(queue.buffer.is_empty());
        }

        // Process new batch
        let batch2 = vec![make_header(HINTS_TYPE_RESULT, 1), 0x02];
        p.process_hints(&batch2, false).unwrap();

        let end = vec![make_ctrl_header(CTRL_END, 0)];
        p.process_hints(&end, false).unwrap();

        // Should have processed 1 hint (starting from 0 again)
        let queue = p.state.queue.lock().unwrap();
        assert_eq!(queue.next_drain_seq, 1);
    }

    #[test]
    fn test_stream_end_waits_until_completion() {
        let p = processor();

        // Dispatch hints
        let data =
            vec![make_header(HINTS_TYPE_RESULT, 1), 0x10, make_header(HINTS_TYPE_RESULT, 1), 0x20];
        p.process_hints(&data, false).unwrap();

        // END should wait internally
        let end = vec![make_ctrl_header(CTRL_END, 0)];
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
        let cancel = vec![make_ctrl_header(CTRL_CANCEL, 0)];

        let result = p.process_hints(&cancel, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));

        // Error flag should be set
        assert!(p.state.error_flag.load(Ordering::Acquire));
    }

    #[test]
    fn test_stream_error_signal_returns_error() {
        let p = processor();
        let signal_err = vec![make_ctrl_header(CTRL_ERROR, 0)];

        let result = p.process_hints(&signal_err, false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("error"));

        // Error flag should be set
        assert!(p.state.error_flag.load(Ordering::Acquire));
    }

    // Stress test
    #[test]
    fn test_stress_throughput() {
        use std::time::Instant;

        let p = PrecompileHintsProcessor::with_num_threads(32, NullHints).unwrap();

        // Generate a large batch of hints
        const NUM_HINTS: usize = 100_000;
        let mut data = Vec::with_capacity(NUM_HINTS * 2);

        for i in 0..NUM_HINTS {
            data.push(make_header(HINTS_TYPE_RESULT, 1));
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

        let p = PrecompileHintsProcessor::with_num_threads(32, NullHints).unwrap();

        const NUM_BATCHES: usize = 1_000;
        const HINTS_PER_BATCH: usize = 100;

        let start = Instant::now();

        // Call process_hints multiple times with small batches
        for batch_id in 0..NUM_BATCHES {
            let mut data = Vec::with_capacity(HINTS_PER_BATCH * 2);
            for i in 0..HINTS_PER_BATCH {
                data.push(make_header(HINTS_TYPE_RESULT, 1));
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

        let p = PrecompileHintsProcessor::with_num_threads(32, NullHints).unwrap();

        const ITERATIONS: usize = 100;
        const HINTS_PER_ITER: usize = 1_000;

        let start = Instant::now();

        for _iter in 0..ITERATIONS {
            // Reset at start of each iteration
            let reset = vec![make_ctrl_header(CTRL_START, 0)];
            p.process_hints(&reset, true).unwrap();

            // Process batch
            let mut data = Vec::with_capacity(HINTS_PER_ITER * 2);
            for i in 0..HINTS_PER_ITER {
                data.push(make_header(HINTS_TYPE_RESULT, 1));
                data.push(i as u64);
            }
            p.process_hints(&data, false).unwrap();

            // End stream
            let end = vec![make_ctrl_header(CTRL_END, 0)];
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
