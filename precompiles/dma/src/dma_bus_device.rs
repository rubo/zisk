//! The `DmaCounter` module defines a counter for tracking dma-related operations
//! sent over the data bus. It connects to the bus and gathers metrics for specific
//! `ZiskOperationType::Dma` instructions.

use std::{collections::VecDeque, ops::Add};

use zisk_common::{
    BusDevice, BusDeviceMode, BusId, Counter, Metrics, B, OPERATION_BUS_DMA_MEMCMP_DATA_SIZE,
    OPERATION_BUS_ID, OP_TYPE, STEP,
};
use zisk_common::{MemCollectorInfo, A, OPERATION_PRECOMPILED_BUS_DATA_SIZE};
use zisk_core::ZiskOperationType;

use crate::{generate_dma_mem_inputs, skip_dma_mem_inputs};

/// The `DmaCounter` struct represents a counter that monitors and measures
/// dma-related operations on the data bus.
///
/// It tracks specific operation types (`ZiskOperationType`) and updates counters for each
/// accepted operation type whenever data is processed on the bus.
pub struct DmaCounterInputGen {
    /// sizes of memcpy
    dma_pre_post_ops: usize,
    dma_ops: usize,
    dma_unaligned_ops: usize,
    dma_64_aligned_ops: usize,

    /// Bus device mode (counter or input generator).
    mode: BusDeviceMode,
}

impl DmaCounterInputGen {
    /// Creates a new instance of `DmaCounter`.
    ///
    /// # Arguments
    /// * `mode` - The ID of the bus to which this counter is connected.
    ///
    /// # Returns
    /// A new `DmaCounter` instance.
    pub fn new(mode: BusDeviceMode) -> Self {
        Self { dma_pre_post_ops: 0, dma_ops: 0, dma_unaligned_ops: 0, dma_64_aligned_ops: 0, mode }
    }

    /// Retrieves the count of instructions for a specific `ZiskOperationType`.
    ///
    /// # Arguments
    /// * `dst` - The destination address of operation.
    /// * `src` - The source address of operation.
    /// * `count` - The bytes of operation.
    pub fn inst_count_memcpy(&mut self, dst: u64, src: u64, count: usize) {
        let src_offset = dst & 0x07;
        let dst_offset = src & 0x07;

        // offset => max bytes is 8 - offset
        if count > 0 {
            let remaining = if dst_offset > 0 {
                self.dma_pre_post_ops += 1;
                std::cmp::min(8 - dst_offset as usize, count)
            } else {
                count
            };
            if (remaining % 8) > 0 {
                self.dma_pre_post_ops += 1;
            }
            if dst_offset == src_offset {
                self.dma_64_aligned_ops += remaining >> 3;
            } else {
                self.dma_unaligned_ops += remaining >> 3;
            }
        }
        self.dma_ops += 1;
    }
}

impl Metrics for DmaCounterInputGen {
    /// Tracks activity on the connected bus and updates counters for recognized operations.
    ///
    /// # Arguments
    /// * `_bus_id` - The ID of the bus (unused in this implementation).
    /// * `_data` - The data received from the bus.
    ///
    /// # Returns
    /// An empty vector, as this implementation does not produce any derived inputs for the bus.
    #[inline(always)]
    fn measure(&mut self, data: &[u64]) {
        let dst = data[A];
        let src = data[B];
        let count = data[OPERATION_PRECOMPILED_BUS_DATA_SIZE] as usize;
        self.inst_count_memcpy(dst, src, count);
    }

    /// Provides a dynamic reference for downcasting purposes.
    ///
    /// # Returns
    /// A reference to `self` as `dyn std::any::Any`.
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Add for DmaCounterInputGen {
    type Output = DmaCounterInputGen;

    /// Combines two `DmaCounter` instances by summing their counters.
    ///
    /// # Arguments
    /// * `self` - The first `DmaCounter` instance.
    /// * `other` - The second `DmaCounter` instance.
    ///
    /// # Returns
    /// A new `DmaCounter` with combined counters.
    fn add(self, other: Self) -> DmaCounterInputGen {
        DmaCounterInputGen {
            dma_pre_post_ops: self.dma_pre_post_ops + other.dma_pre_post_ops,
            dma_ops: self.dma_ops + other.dma_ops,
            dma_unaligned_ops: self.dma_unaligned_ops + other.dma_unaligned_ops,
            dma_64_aligned_ops: self.dma_64_aligned_ops + other.dma_64_aligned_ops,
            mode: self.mode,
        }
    }
}

impl BusDevice<u64> for DmaCounterInputGen {
    /// Processes data received on the bus, updating counters and generating inputs when applicable.
    ///
    /// # Arguments
    /// * `bus_id` - The ID of the bus sending the data.
    /// * `data` - The data received from the bus.
    /// * `pending` – A queue of pending bus operations used to send derived inputs.
    ///
    /// # Returns
    /// A boolean indicating whether the program should continue execution or terminate.
    /// Returns `true` to continue execution, `false` to stop.
    #[inline(always)]
    fn process_data(
        &mut self,
        bus_id: &BusId,
        data: &[u64],
        pending: &mut VecDeque<(BusId, Vec<u64>)>,
        mem_collector_info: Option<&[MemCollectorInfo]>,
    ) -> bool {
        debug_assert!(*bus_id == OPERATION_BUS_ID);

        if data[OP_TYPE] as u32 != ZiskOperationType::Dma as u32 {
            return true;
        }

        let dst = data[A];
        let src = data[B];
        let count = data[OPERATION_PRECOMPILED_BUS_DATA_SIZE] as usize;
        if let Some(mem_collectors_info) = mem_collector_info {
            if skip_dma_mem_inputs(dst, src, count, mem_collectors_info) {
                return true;
            }
        }

        let only_counters = self.mode == BusDeviceMode::Counter;
        if only_counters {
            self.measure(data);
        }

        let step_main = data[STEP];
        let data_ext = &[0u64; 4];
        generate_dma_mem_inputs(dst, src, count, step_main, data, data_ext, only_counters, pending);

        true
    }

    /// Returns the bus IDs associated with this counter.
    ///
    /// # Returns
    /// A vector containing the connected bus ID.
    fn bus_id(&self) -> Vec<BusId> {
        vec![OPERATION_BUS_ID]
    }

    /// Provides a dynamic reference for downcasting purposes.
    fn as_any(self: Box<Self>) -> Box<dyn std::any::Any> {
        self
    }
}
