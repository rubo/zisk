//! Precompile Hints Relay

use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use zisk_common::{io::StreamProcessor, PrecompileHint, CTRL_END, CTRL_START, NUM_HINT_TYPES};
use zisk_distributed_common::StreamTypeDto;

type AsyncDispatcher = Arc<
    dyn Fn(u32, StreamTypeDto, Vec<u8>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync,
>;

pub struct PrecompileHintsRelay {
    sequence_number: Arc<AtomicU32>,
    dispatcher: AsyncDispatcher,
    runtime_handle: tokio::runtime::Handle,
}

impl PrecompileHintsRelay {
    pub fn new<F, Fut>(dispatcher: F) -> Self
    where
        F: Fn(u32, StreamTypeDto, Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let dispatcher = Arc::new(
            move |seq: u32,
                  stream_type: StreamTypeDto,
                  payload: Vec<u8>|
                  -> Pin<Box<dyn Future<Output = ()> + Send>> {
                Box::pin(dispatcher(seq, stream_type, payload))
            },
        );

        Self {
            sequence_number: Arc::new(AtomicU32::new(0)),
            dispatcher,
            runtime_handle: tokio::runtime::Handle::current(),
        }
    }

    pub fn process_hints(&self, hints: &[u64], first_batch: bool) -> Result<bool> {
        let mut has_ctrl_start = false;
        let mut has_ctrl_end = false;

        // Parse hints and dispatch to pool
        let mut idx = 0;
        while idx < hints.len() {
            let hint = PrecompileHint::from_u64_slice(hints, idx)?;
            let length = hint.data.len();

            // Validate hint type is in valid range before accessing stats array
            if hint.hint_type >= NUM_HINT_TYPES {
                return Err(anyhow::anyhow!("Invalid hint type: {}", hint.hint_type));
            }

            // CTRL_START must be the first message of the first batch
            if hint.hint_type == CTRL_START {
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
                has_ctrl_start = true;
            }

            if has_ctrl_end {
                return Err(anyhow::anyhow!(
                    "Received hint after CTRL_END: type {} at index {}",
                    hint.hint_type,
                    idx
                ));
            }
            has_ctrl_end = hint.hint_type == CTRL_END;

            idx += length + 1;
        }

        if has_ctrl_start {
            self.send_hints_start();
        }

        // Call async dispatcher - blocks on async work for zero overhead
        self.send_hints_data(hints.to_vec());

        if has_ctrl_end {
            self.send_hints_end();
        }

        Ok(has_ctrl_end)
    }

    fn send_hints_start(&self) {
        let seq_num = self.sequence_number.fetch_add(1, Ordering::SeqCst);
        println!("Sending CTRL_START with sequence number {}", seq_num);

        self.runtime_handle.block_on((self.dispatcher)(seq_num, StreamTypeDto::Start, vec![]));
    }

    fn send_hints_data(&self, hints: Vec<u64>) {
        let seq_num = self.sequence_number.fetch_add(1, Ordering::SeqCst);
        println!("Sending Hints DATA with sequence number {}", seq_num);

        // Convert Vec<u64> to Vec<u8> for wire protocol
        let payload = unsafe {
            let mut hints_vec = hints.to_vec();
            let ptr = hints_vec.as_mut_ptr() as *mut u8;
            let len = hints_vec.len() * std::mem::size_of::<u64>();
            let capacity = hints_vec.capacity() * std::mem::size_of::<u64>();
            std::mem::forget(hints_vec);
            Vec::from_raw_parts(ptr, len, capacity)
        };

        self.runtime_handle.block_on((self.dispatcher)(seq_num, StreamTypeDto::Data, payload));
    }

    fn send_hints_end(&self) {
        let seq_num = self.sequence_number.fetch_add(1, Ordering::SeqCst);
        println!("Sending CTRL_END with sequence number {}", seq_num);

        self.runtime_handle.block_on((self.dispatcher)(seq_num, StreamTypeDto::End, vec![]));
    }
}

impl StreamProcessor for PrecompileHintsRelay {
    fn process(&self, data: &[u64], first_batch: bool) -> Result<bool> {
        self.process_hints(data, first_batch)
    }
}
