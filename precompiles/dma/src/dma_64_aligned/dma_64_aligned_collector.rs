//! The `Dma64AlignedInstance` module defines an instance to perform the witness computation
//! for the Dma State Machine.
//!
//! It manages collected inputs and interacts with the `DmaSM` to compute witnesses for
//! execution plans.

use crate::{Dma64AlignedInput, DmaCollectCounters, DmaCollectorRoutingLog};
use precompiles_helpers::DmaInfo;
use std::any::Any;
use zisk_common::{BusDevice, BusId, ChunkId, DMA_ENCODED, OP, OPERATION_BUS_ID, OP_TYPE};
use zisk_core::{zisk_ops::ZiskOp, ZiskOperationType};
#[derive(Debug)]
pub struct Dma64AlignedCollector {
    /// Collected inputs for witness computation.
    pub inputs: Vec<Dma64AlignedInput>,

    pub chunk_id: ChunkId,

    pub rlog: DmaCollectorRoutingLog,

    /// The number of inputs to collect.
    pub num_inputs: u64,

    /// Helper to skip instructions based on the plan's configuration.
    pub collect_counters: DmaCollectCounters,

    pub trace_offset: usize,
    pub ops_by_row: usize,
    pub last_segment_collector: bool,
}

impl Dma64AlignedCollector {
    /// Creates a new `Dma64AlignedCollector`.
    ///
    /// # Arguments
    ///
    /// * `bus_id` - The connected bus ID.
    /// * `num_inputs` - The number of inputs to collect.
    /// * `collect_counter` - The helper to skip instructions based on the plan's configuration.
    ///
    /// # Returns
    /// A new `Dma64AlignedCollector` instance initialized with the provided parameters.
    pub fn new(
        chunk_id: ChunkId,
        num_inputs: u64,
        collect_counters: DmaCollectCounters,
        ops_by_row: usize,
        last_segment_collector: bool,
    ) -> Self {
        Self {
            inputs: Vec::with_capacity(num_inputs as usize),
            num_inputs,
            collect_counters,
            trace_offset: 0,
            ops_by_row,
            last_segment_collector,
            rlog: DmaCollectorRoutingLog::new(chunk_id),
            chunk_id,
        }
    }

    /// Processes data received on the bus, collecting the inputs necessary for witness computation.
    ///
    /// # Arguments
    /// * `_bus_id` - The ID of the bus (unused in this implementation).
    /// * `data` - The data received from the bus.
    /// * `pending` – A queue of pending bus operations used to send derived inputs.
    ///
    /// # Returns
    /// A tuple where:
    /// A boolean indicating whether the program should continue execution or terminate.
    /// Returns `true` to continue execution, `false` to stop.
    #[inline(always)]
    pub fn process_data(&mut self, bus_id: &BusId, data: &[u64], data_ext: &[u64]) -> bool {
        debug_assert!(*bus_id == OPERATION_BUS_ID);

        if data[OP_TYPE] != ZiskOperationType::Dma as u64 {
            return true;
        }

        if self.inputs.len() == self.num_inputs as usize {
            debug_assert!(self.collect_counters.is_final_skip());
            return self.rlog.log_discard_cond(false, 1, data, false);
        }

        let op = data[OP] as u8;
        let has_src = op == ZiskOp::DMA_MEMCPY
            || op == ZiskOp::DMA_XMEMCPY
            || op == ZiskOp::DMA_MEMCMP
            || op == ZiskOp::DMA_XMEMCMP;
        let encoded = data[DMA_ENCODED];

        if has_src && !DmaInfo::dst_is_aligned_with_src(encoded) {
            self.rlog.log_discard(2, data);
            return true;
        }

        let rows = DmaInfo::get_loop_count(encoded).div_ceil(self.ops_by_row);
        if rows == 0 {
            self.rlog.log_discard(3, data);
            return true;
        }
        // self.collect_counters.memcpy.should_process(rows)
        if let Some((skip, max_count, is_final_skip)) =
            self.collect_counters.should_collect(rows as u64, op)
        {
            self.rlog.log_collect(rows, data);
            self.inputs.push(if op == ZiskOp::DMA_XMEMSET {
                Dma64AlignedInput::from_memset(
                    data,
                    self.trace_offset,
                    skip as usize,
                    self.ops_by_row,
                    max_count as usize,
                    self.last_segment_collector && is_final_skip,
                )
            } else {
                Dma64AlignedInput::from(
                    data,
                    data_ext,
                    self.trace_offset,
                    skip as usize,
                    self.ops_by_row,
                    max_count as usize,
                    self.last_segment_collector && is_final_skip,
                )
            });
            self.trace_offset += max_count as usize;
        } else {
            self.rlog.log_discard(10, data);
        }
        if self.inputs.len() >= self.num_inputs as usize {
            debug_assert!(self.collect_counters.is_final_skip());
            return self.rlog.log_discard_cond(true, 11, data, false);
        }
        true
    }
    pub fn get_debug_info(&self) -> String {
        #[cfg(feature = "save_dma_collectors")]
        return format!(
            "CC|{}|{}|{}\n",
            self.chunk_id,
            self.inputs.len(),
            self.collect_counters.get_debug_info(),
        ) + &self.rlog.get_debug_info();
        #[cfg(not(feature = "save_dma_collectors"))]
        String::new()
    }
}

impl BusDevice<u64> for Dma64AlignedCollector {
    fn as_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}
