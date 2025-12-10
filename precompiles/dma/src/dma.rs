use std::sync::Arc;

use fields::PrimeField64;

use pil_std_lib::Std;
use proofman_common::AirInstance;
use proofman_util::{timer_start_trace, timer_stop_and_log_info, timer_stop_and_log_trace};

use super::DmaMemCpyInput;
/*
#[cfg(feature = "packed")]
mod types {
    use zisk_pil::{DmaRowPacked, DmaTracePacked};
    pub type DmaTraceRowType<F> = DmaRowPacked<F>;
    pub type DmaTraceType<F> = DmaTracePacked<F>;
}

#[cfg(not(feature = "packed"))]
mod types {
    use zisk_pil::{DmaTrace, DmaTraceRow};
    pub type DmaTraceRowType<F> = DmaTraceRow<F>;
    pub type DmaTraceType<F> = DmaTrace<F>;
}

use types::*;
*/
#[cfg(feature = "packed")]
pub use zisk_pil::{DmaRowPacked as DmaTraceRow, DmaTracePacked as DmaTrace};

#[cfg(not(feature = "packed"))]
pub use zisk_pil::{DmaTrace, DmaTraceRow};

/// The `DmaSM` struct encapsulates the logic of the Dma State Machine.
pub struct DmaSM<F: PrimeField64> {
    /// Reference to the PIL2 standard library.
    pub std: Arc<Std<F>>,

    /// Number of available dmas in the trace.
    pub num_availables: usize,

    /// Range checks ID's
    range_21_bits_id: usize,
}

impl<F: PrimeField64> DmaSM<F> {
    /// Creates a new Dma State Machine instance.
    ///
    /// # Returns
    /// A new `DmaSM` instance.
    pub fn new(std: Arc<Std<F>>) -> Arc<Self> {
        // Compute some useful values
        let num_availables = DmaTrace::<F>::NUM_ROWS;

        let range_21_bits_id = std.get_range_id(0, (1 << 21) - 1, None);

        Arc::new(Self { std, num_availables, range_21_bits_id })
    }

    /// Processes a slice of operation data, updating the trace.
    ///
    /// # Arguments
    /// * `trace` - A mutable reference to the Dma trace.
    /// * `input` - The operation data to process.
    #[inline(always)]
    pub fn process_slice(
        &self,
        input: &DmaMemCpyInput,
        trace: &mut DmaTraceRow<F>,
        multiplicities: &mut [u32],
    ) {
        unimplemented!();
    }

    /// Computes the witness for a series of inputs and produces an `AirInstance`.
    ///
    /// # Arguments
    /// * `sctx` - The setup context containing the setup data.
    /// * `inputs` - A slice of operations to process.
    ///
    /// # Returns
    /// An `AirInstance` containing the computed witness data.
    pub fn compute_witness(
        &self,
        inputs: &[Vec<DmaMemCpyInput>],
        trace_buffer: Vec<F>,
    ) -> AirInstance<F> {
        let mut trace = DmaTrace::<F>::new_from_vec(trace_buffer);

        let num_rows = trace.num_rows();

        let total_inputs: usize = inputs.iter().map(|c| c.len()).sum();
        assert!(total_inputs <= num_rows);

        tracing::info!(
            "··· Creating Dma instance [{} / {} rows filled {:.2}%]",
            total_inputs,
            num_rows,
            total_inputs as f64 / num_rows as f64 * 100.0
        );

        timer_start_trace!(DMA_TRACE);

        timer_stop_and_log_info!(DMA_TRACE);
        unimplemented!();
    }
}
