//! The `DmaInstance` module defines an instance to perform the witness computation
//! for the Dma State Machine.
//!
//! It manages collected inputs and interacts with the `DmaSM` to compute witnesses for
//! execution plans.

use crate::{DmaMemCpyInput, DmaSM};
use fields::PrimeField64;
use proofman_common::{AirInstance, ProofCtx, SetupCtx};
use std::collections::VecDeque;
use std::{any::Any, collections::HashMap, sync::Arc};
use zisk_common::ChunkId;
use zisk_common::{
    BusDevice, BusId, CheckPoint, CollectSkipper, ExtOperationData, Instance, InstanceCtx,
    InstanceType, MemCollectorInfo, PayloadType, OPERATION_BUS_ID, OP_TYPE,
};
use zisk_core::ZiskOperationType;
use zisk_pil::DmaTrace;

/// The `DmaInstance` struct represents an instance for the Dma State Machine.
///
/// It encapsulates the `DmaSM` and its associated context, and it processes input data
/// to compute witnesses for the Dma State Machine.
pub struct DmaInstance<F: PrimeField64> {
    /// Dma state machine.
    dma_sm: Arc<DmaSM<F>>,

    /// Instance context.
    ictx: InstanceCtx,
}

impl<F: PrimeField64> DmaInstance<F> {
    /// Creates a new `DmaInstance`.
    ///
    /// # Arguments
    /// * `dma_sm` - An `Arc`-wrapped reference to the Dma State Machine.
    /// * `ictx` - The `InstanceCtx` associated with this instance, containing the execution plan.
    /// * `bus_id` - The bus ID associated with this instance.
    ///
    /// # Returns
    /// A new `DmaInstance` instance initialized with the provided state machine and
    /// context.
    pub fn new(dma_sm: Arc<DmaSM<F>>, ictx: InstanceCtx) -> Self {
        Self { dma_sm, ictx }
    }

    pub fn build_dma_collector(&self, chunk_id: ChunkId) -> DmaCollector {
        assert_eq!(
            self.ictx.plan.air_id,
            DmaTrace::<F>::AIR_ID,
            "DmaInstance: Unsupported air_id: {:?}",
            self.ictx.plan.air_id
        );

        let meta = self.ictx.plan.meta.as_ref().unwrap();
        let collect_info = meta.downcast_ref::<HashMap<ChunkId, (u64, CollectSkipper)>>().unwrap();
        let (num_ops, collect_skipper) = collect_info[&chunk_id];
        DmaCollector::new(num_ops, collect_skipper)
    }
}

impl<F: PrimeField64> Instance<F> for DmaInstance<F> {
    /// Computes the witness for the Dma execution plan.
    ///
    /// This method leverages the `DmaSM` to generate an `AirInstance` using the collected
    /// inputs.
    ///
    /// # Arguments
    /// * `_pctx` - The proof context, unused in this implementation.
    ///
    /// # Returns
    /// An `Option` containing the computed `AirInstance`.
    fn compute_witness(
        &self,
        _pctx: &ProofCtx<F>,
        _sctx: &SetupCtx<F>,
        collectors: Vec<(usize, Box<dyn BusDevice<PayloadType>>)>,
        trace_buffer: Vec<F>,
    ) -> Option<AirInstance<F>> {
        let inputs: Vec<_> = collectors
            .into_iter()
            .map(|(_, collector)| collector.as_any().downcast::<DmaCollector>().unwrap().inputs)
            .collect();

        Some(self.dma_sm.compute_witness(&inputs, trace_buffer))
    }

    /// Retrieves the checkpoint associated with this instance.
    ///
    /// # Returns
    /// A `CheckPoint` object representing the checkpoint of the execution plan.
    fn check_point(&self) -> &CheckPoint {
        &self.ictx.plan.check_point
    }

    /// Retrieves the type of this instance.
    ///
    /// # Returns
    /// An `InstanceType` representing the type of this instance (`InstanceType::Instance`).
    fn instance_type(&self) -> InstanceType {
        InstanceType::Instance
    }

    fn build_inputs_collector(&self, chunk_id: ChunkId) -> Option<Box<dyn BusDevice<PayloadType>>> {
        assert_eq!(
            self.ictx.plan.air_id,
            DmaTrace::<F>::AIR_ID,
            "DmaInstance: Unsupported air_id: {:?}",
            self.ictx.plan.air_id
        );

        let meta = self.ictx.plan.meta.as_ref().unwrap();
        let collect_info = meta.downcast_ref::<HashMap<ChunkId, (u64, CollectSkipper)>>().unwrap();
        let (num_ops, collect_skipper) = collect_info[&chunk_id];
        Some(Box::new(DmaCollector::new(num_ops, collect_skipper)))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub struct DmaCollector {
    /// Collected inputs for witness computation.
    inputs: Vec<DmaMemCpyInput>,

    /// The number of operations to collect.
    num_operations: u64,

    /// Helper to skip instructions based on the plan's configuration.
    collect_skipper: CollectSkipper,
}

impl DmaCollector {
    /// Creates a new `DmaCollector`.
    ///
    /// # Arguments
    ///
    /// * `bus_id` - The connected bus ID.
    /// * `num_operations` - The number of operations to collect.
    /// * `collect_skipper` - The helper to skip instructions based on the plan's configuration.
    ///
    /// # Returns
    /// A new `ArithInstanceCollector` instance initialized with the provided parameters.
    pub fn new(num_operations: u64, collect_skipper: CollectSkipper) -> Self {
        Self {
            inputs: Vec::with_capacity(num_operations as usize),
            num_operations,
            collect_skipper,
        }
    }
}

impl BusDevice<PayloadType> for DmaCollector {
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
    fn process_data(
        &mut self,
        bus_id: &BusId,
        data: &[PayloadType],
        _pending: &mut VecDeque<(BusId, Vec<PayloadType>)>,
        _mem_collector_info: Option<&[MemCollectorInfo]>,
    ) -> bool {
        debug_assert!(*bus_id == OPERATION_BUS_ID);

        if self.inputs.len() == self.num_operations as usize {
            return false;
        }

        if data[OP_TYPE] != ZiskOperationType::Dma as u64 {
            return true;
        }

        if self.collect_skipper.should_skip() {
            return true;
        }

        let data: ExtOperationData<u64> =
            data.try_into().expect("Regular Metrics: Failed to convert data");
        if let ExtOperationData::OperationDmaMemCpyData(data) = data {
            self.inputs.push(DmaMemCpyInput::from(&data));
        } else {
            panic!("Expected ExtOperationData::OperationDmaData");
        }

        self.inputs.len() < self.num_operations as usize
    }

    /// Returns the bus IDs associated with this instance.
    ///
    /// # Returns
    /// A vector containing the connected bus ID.
    fn bus_id(&self) -> Vec<BusId> {
        vec![OPERATION_BUS_ID]
    }

    fn as_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}
