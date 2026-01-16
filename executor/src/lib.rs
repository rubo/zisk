mod dummy_counter;
mod emu_asm;
mod emu_rust;
mod executor;
mod sm_static_bundle;
mod static_data_bus;
mod static_data_bus_collect;

pub use dummy_counter::*;
pub use executor::*;
pub use sm_static_bundle::*;
pub use static_data_bus::*;
pub use static_data_bus_collect::*;

use crate::emu_asm::{DeviceMetricsList, NestedDeviceMetricsList};
use asm_runner::{AsmRunnerMO, MinimalTraces};
use fields::PrimeField64;
use proofman_common::ProofCtx;
use std::{sync::Mutex, thread::JoinHandle};
use zisk_common::{io::ZiskStdin, ExecutorStatsHandle, ZiskExecutionResult};

/// Trait for unified execution across different emulator backends
pub trait Emulator<F: PrimeField64>: Send + Sync {
    /// Execute the emulator
    fn execute(
        &self,
        stdin: &Mutex<ZiskStdin>,
        pctx: &ProofCtx<F>,
        sm_bundle: &StaticSMBundle<F>,
        stats: &ExecutorStatsHandle,
        caller_stats_id: u64,
    ) -> (
        MinimalTraces,
        DeviceMetricsList,
        NestedDeviceMetricsList,
        Option<JoinHandle<AsmRunnerMO>>,
        ZiskExecutionResult,
    );

    fn is_asm_emulator(&self) -> bool;
}
