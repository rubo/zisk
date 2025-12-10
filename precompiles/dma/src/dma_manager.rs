use std::sync::Arc;

use fields::PrimeField64;
use pil_std_lib::Std;
use zisk_common::{
    BusDevice, BusDeviceMetrics, BusDeviceMode, ComponentBuilder, Instance, InstanceCtx,
    InstanceInfo, PayloadType, Planner,
};
use zisk_core::ZiskOperationType;
use zisk_pil::DmaTrace;

use crate::{DmaCounterInputGen, DmaInstance, DmaPlanner, DmaSM};

/// The `DmaManager` struct represents the Dma manager,
/// which is responsible for managing the Dma state machine and its table state machine.
#[allow(dead_code)]
pub struct DmaManager<F: PrimeField64> {
    /// Dma state machine
    dma_sm: Arc<DmaSM<F>>,
}

impl<F: PrimeField64> DmaManager<F> {
    /// Creates a new instance of `DmaManager`.
    ///
    /// # Returns
    /// An `Arc`-wrapped instance of `DmaManager`.
    pub fn new(std: Arc<Std<F>>) -> Arc<Self> {
        let dma_sm = DmaSM::new(std);

        Arc::new(Self { dma_sm })
    }

    pub fn build_dma_counter(&self) -> DmaCounterInputGen {
        DmaCounterInputGen::new(BusDeviceMode::Counter)
    }

    pub fn build_dma_input_generator(&self) -> DmaCounterInputGen {
        DmaCounterInputGen::new(BusDeviceMode::InputGenerator)
    }
}

impl<F: PrimeField64> ComponentBuilder<F> for DmaManager<F> {
    /// Builds and returns a new counter for monitoring Dma operations.
    ///
    /// # Returns
    /// A boxed implementation of `RegularCounters` configured for Dma operations.
    fn build_counter(&self) -> Option<Box<dyn BusDeviceMetrics>> {
        Some(Box::new(DmaCounterInputGen::new(BusDeviceMode::Counter)))
    }

    /// Builds a planner to plan Dma-related instances.
    ///
    /// # Returns
    /// A boxed implementation of `RegularPlanner`.
    fn build_planner(&self) -> Box<dyn Planner> {
        // Get the number of Dmas that a single Dma instance can handle
        let num_availables = self.dma_sm.num_availables;

        Box::new(DmaPlanner::new().add_instance(InstanceInfo::new(
            DmaTrace::<usize>::AIRGROUP_ID,
            DmaTrace::<usize>::AIR_ID,
            num_availables,
            ZiskOperationType::BigInt,
        )))
    }

    /// Builds an inputs data collector for Dma operations.
    ///
    /// # Arguments
    /// * `ictx` - The context of the instance, containing the plan and its associated
    ///   configurations.
    ///
    /// # Returns
    /// A boxed implementation of `BusDeviceInstance` specific to the requested `air_id` instance.
    ///
    /// # Panics
    /// Panics if the provided `air_id` is not supported.
    fn build_instance(&self, ictx: InstanceCtx) -> Box<dyn Instance<F>> {
        match ictx.plan.air_id {
            id if id == DmaTrace::<usize>::AIR_ID => {
                Box::new(DmaInstance::new(self.dma_sm.clone(), ictx))
            }
            _ => {
                panic!("DmaBuilder::get_instance() Unsupported air_id: {:?}", ictx.plan.air_id)
            }
        }
    }

    fn build_inputs_generator(&self) -> Option<Box<dyn BusDevice<PayloadType>>> {
        Some(Box::new(DmaCounterInputGen::new(BusDeviceMode::InputGenerator)))
    }
}
