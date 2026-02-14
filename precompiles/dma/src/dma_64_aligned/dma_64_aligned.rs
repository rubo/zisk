use std::sync::Arc;

use fields::PrimeField64;

use pil_std_lib::Std;
use proofman_common::{AirInstance, FromTrace, ProofmanResult};
use proofman_util::{timer_start_trace, timer_stop_and_log_trace};
use zisk_common::SegmentId;
use zisk_core::zisk_ops::ZiskOp;
use zisk_pil::Dma64AlignedAirValues;

#[cfg(feature = "packed")]
pub use zisk_pil::{
    Dma64AlignedTracePacked as Dma64AlignedTrace,
    Dma64AlignedTraceRowPacked as Dma64AlignedTraceRow,
};

#[cfg(not(feature = "packed"))]
pub use zisk_pil::{Dma64AlignedTrace, Dma64AlignedTraceRow};

use crate::{
    Dma64AlignedInput, Dma64AlignedModule, DMA_64_ALIGNED_OPS_BY_ROW, F_SEL_INPUTCPY, F_SEL_MEMCMP,
    F_SEL_MEMCPY, F_SEL_MEMSET,
};
use precompiles_helpers::DmaInfo;

/// The `Dma64AlignedSM` struct encapsulates the logic of the Dma64Aligned State Machine.
pub struct Dma64AlignedSM<F: PrimeField64> {
    /// Reference to the PIL2 standard library.
    pub std: Arc<Std<F>>,

    /// Range checks ID's
    range_16_bits_id: usize,
    op_x_rows: usize,
}

impl<F: PrimeField64> Dma64AlignedSM<F> {
    /// Creates a new Dma State Machine instance.
    ///
    /// # Returns
    /// A new `Dma64AlignedSM` instance.
    pub fn new(std: Arc<Std<F>>) -> Arc<Self> {
        Arc::new(Self {
            std: std.clone(),
            range_16_bits_id: std
                .get_range_id(0, 0xFFFF, None)
                .expect("Failed to get 16b table ID"),
            op_x_rows: DMA_64_ALIGNED_OPS_BY_ROW,
        })
    }

    /// Processes a slice of operation data, updating the trace.
    ///
    /// # Arguments
    /// * `trace` - A mutable reference to the Dma trace.
    /// * `input` - The operation data to process.
    #[inline(always)]
    pub fn process_input(
        &self,
        input: &Dma64AlignedInput,
        trace: &mut [Dma64AlignedTraceRow<F>],
        _local_16_bits_table: &mut [u32], // for input_cpy
        air_values: &mut Dma64AlignedAirValues<F>,
    ) -> usize {
        let rows = input.rows as usize;
        let skip_count = input.skip_rows as usize * self.op_x_rows;
        let initial_count = DmaInfo::get_loop_count(input.encoded) - skip_count;
        let mut count64 = initial_count;
        // println!(
        //     "DMA_64_ALIGNED INPUT {input:?} count:{count64} rows:{rows} dma_info:{}",
        //     DmaInfo::to_string(input.encoded)
        // );
        let mut src_values_index = 0;
        let mut dst64 = ((input.dst + 7) >> 3) + skip_count as u32;
        let mut src64 = ((input.src + 7) >> 3) + skip_count as u32;
        let mut seq_end = false;
        let addr_incr_by_row = self.op_x_rows as u32;

        let is_memcpy = input.op == ZiskOp::DMA_XMEMCPY || input.op == ZiskOp::DMA_MEMCPY;
        let is_memeq = input.op == ZiskOp::DMA_MEMCMP || input.op == ZiskOp::DMA_XMEMCMP;
        let is_memset = input.op == ZiskOp::DMA_XMEMSET;
        let is_inputcpy = input.op == ZiskOp::DMA_INPUTCPY;
        let fill_byte = if is_memset { (input.encoded & 0xFF) as u8 } else { 0 };
        for (irow, row) in trace.iter_mut().enumerate().take(rows) {
            row.set_main_step(input.step);

            row.set_sel_memcpy(is_memcpy);
            row.set_sel_memeq(is_memeq);
            row.set_sel_memset(is_memset);
            row.set_sel_inputcpy(is_inputcpy);
            if irow == 0 && input.skip_rows == 0 {
                row.set_sel_memcpy_count_load(input.op == ZiskOp::DMA_MEMCPY);
            }
            row.set_fill_byte(fill_byte);
            row.set_previous_seq_end(irow == 0 && input.skip_rows == 0);

            // calculate the first aligned address
            // if dst is aligned is same address if not it's addr + 8
            row.set_dst64(dst64);
            row.set_src64(src64);
            dst64 += addr_incr_by_row;
            src64 += addr_incr_by_row;

            row.set_count64(count64 as u32);
            let use_count = if count64 <= self.op_x_rows {
                seq_end = true;
                for index in count64..self.op_x_rows {
                    if index > 0 {
                        row.set_sel_op_from_1(index - 1, false);
                    }
                    row.set_h_value_chunks(index, 0, 0);
                    row.set_h_value_chunks(index, 1, 0);
                    row.set_l_value_chunks(index, 0, 0);
                    row.set_l_value_chunks(index, 1, 0);
                }
                count64
            } else {
                count64 -= self.op_x_rows;
                self.op_x_rows
            };
            row.set_seq_end(seq_end);
            for index in 0..use_count {
                if index > 0 {
                    row.set_sel_op_from_1(index - 1, true);
                }
                let value = input.src_values[src_values_index];
                src_values_index += 1;
                row.set_h_value_chunks(index, 0, (value >> 8) as u32);
                row.set_h_value_chunks(index, 1, (value >> 40) as u32);
                row.set_l_value_chunks(index, 0, value as u8);
                row.set_l_value_chunks(index, 1, (value >> 32) as u8);
            }
        }

        if input.is_last_instance_input {
            if seq_end {
                air_values.segment_last_seq_end = F::ONE;
                air_values.segment_last_src64 = F::ZERO;
                air_values.segment_last_dst64 = F::ZERO;
                air_values.segment_last_main_step = F::ZERO;
                air_values.segment_last_count64 = F::ZERO;
                air_values.last_count_chunk[0] = F::ZERO;
                air_values.last_count_chunk[1] = F::ZERO;
                air_values.segment_last_flags = F::ZERO;
            } else {
                air_values.segment_last_seq_end = F::ZERO;
                air_values.segment_last_src64 = F::from_u32(src64 - addr_incr_by_row);
                air_values.segment_last_dst64 = F::from_u32(dst64 - addr_incr_by_row);
                air_values.segment_last_main_step = F::from_u64(input.step);
                let last_count = initial_count - (rows - 1) * self.op_x_rows;
                air_values.segment_last_count64 = F::from_u32(last_count as u32);
                air_values.last_count_chunk[0] = F::from_u16(last_count as u16);
                air_values.last_count_chunk[1] = F::from_u16((last_count >> 16) as u16);
                air_values.segment_last_flags = F::from_u16(match input.op {
                    ZiskOp::DMA_MEMCPY | ZiskOp::DMA_XMEMCPY => F_SEL_MEMCPY,
                    ZiskOp::DMA_MEMCMP | ZiskOp::DMA_XMEMCMP => F_SEL_MEMCMP,
                    ZiskOp::DMA_INPUTCPY => F_SEL_INPUTCPY,
                    ZiskOp::DMA_XMEMSET => F_SEL_MEMSET,
                    _ => panic!("Invalid operation 0x{:02X}", input.op),
                } as u16);
            }
        }
        rows
    }

    /// Processes a slice of operation data, updating the trace.
    ///
    /// # Arguments
    /// * `trace` - A mutable reference to the Dma trace.
    /// * `input` - The operation data to process.
    #[inline(always)]
    pub fn process_empty_slice(&self, trace: &mut Dma64AlignedTraceRow<F>) {
        trace.set_seq_end(true);
        trace.set_previous_seq_end(true);
    }
}
impl<F: PrimeField64> Dma64AlignedModule<F> for Dma64AlignedSM<F> {
    fn get_name(&self) -> &'static str {
        "dma_64_aligned"
    }

    /// Computes the witness for a series of inputs and produces an `AirInstance`.
    ///
    /// # Arguments
    /// * `sctx` - The setup context containing the setup data.
    /// * `inputs` - A slice of operations to process.
    ///
    /// # Returns
    /// An `AirInstance` containing the computed witness data.
    fn compute_witness(
        &self,
        inputs: &[Vec<Dma64AlignedInput>],
        segment_id: SegmentId,
        is_last_segment: bool,
        trace_buffer: Vec<F>,
    ) -> ProofmanResult<AirInstance<F>> {
        let mut trace = Dma64AlignedTrace::<F>::new_from_vec(trace_buffer)?;
        let num_rows = trace.num_rows();

        let total_inputs: usize = inputs
            .iter()
            .map(|inputs| inputs.iter().map(|input| input.rows as usize).sum::<usize>())
            .sum();

        assert!(total_inputs > 0);
        // println!("LAST INPUT: {:?}", inputs.last().unwrap());
        // println!("DMA_64_ALIGNED TOTALS total_inputs:{total_inputs} num_rows:{num_rows}");
        assert!(
            total_inputs <= num_rows,
            "Too many inputs, total_inputs:{total_inputs} num_rows:{num_rows}"
        );

        tracing::debug!(
            "··· Creating Dma64Aligned instance [{total_inputs} / {num_rows} rows filled {:.2}%]",
            total_inputs as f64 / num_rows as f64 * 100.0
        );

        timer_start_trace!(DMA_64_ALIGNED_TRACE);

        // Split the dma_trace.buffer into slices matching each inner vector’s length.
        let flat_inputs: Vec<_> = inputs.iter().flatten().collect();
        let trace_rows = trace.buffer.as_mut_slice();

        let mut local_16_bits_table = vec![0u32; 1 << 16];
        let mut air_values = Dma64AlignedAirValues::<F>::new();

        // TODO: inputs between instances
        let mut row_offset = 0;
        for input in flat_inputs.iter() {
            let rows_used = self.process_input(
                input,
                &mut trace_rows[row_offset..],
                &mut local_16_bits_table,
                &mut air_values,
            );
            row_offset += rows_used;
        }

        // padding
        air_values.padding_size = F::from_u32((num_rows - row_offset) as u32);
        for padding_row in trace_rows.iter_mut().take(num_rows).skip(row_offset) {
            self.process_empty_slice(padding_row);
        }
        if row_offset < num_rows {
            air_values.segment_last_seq_end = F::ONE;
            air_values.segment_last_src64 = F::ZERO;
            air_values.segment_last_dst64 = F::ZERO;
            air_values.segment_last_main_step = F::ZERO;
            air_values.segment_last_count64 = F::ZERO;
            air_values.last_count_chunk[0] = F::ZERO;
            air_values.last_count_chunk[1] = F::ZERO;
            air_values.segment_last_flags = F::ZERO;
        }

        // add range check of count to check that it's a positive 32-bits number
        let last_count = air_values.segment_last_count64.as_canonical_u64();
        local_16_bits_table[(last_count & 0xFFFF) as usize] += 1;
        local_16_bits_table[((last_count >> 16) & 0xFFFF) as usize] += 1;

        self.std.range_checks(self.range_16_bits_id, local_16_bits_table);

        let segment_id = segment_id.into();
        air_values.segment_id = F::from_usize(segment_id);
        air_values.is_last_segment = F::from_bool(is_last_segment);

        let first_input = flat_inputs.first().unwrap();
        if first_input.skip_rows == 0 {
            air_values.segment_previous_seq_end = F::ONE;
            air_values.segment_previous_dst64 = F::ZERO;
            air_values.segment_previous_src64 = F::ZERO;
            air_values.segment_previous_main_step = F::ZERO;
            air_values.segment_previous_count64 = F::ZERO;
            air_values.segment_previous_flags = F::ZERO;
        } else {
            assert!(segment_id > 0);
            air_values.segment_previous_seq_end = F::ZERO;
            air_values.segment_previous_dst64 =
                F::from_u32(trace_rows[0].get_dst64() - self.op_x_rows as u32);
            air_values.segment_previous_src64 =
                F::from_u32(trace_rows[0].get_src64() - self.op_x_rows as u32);
            air_values.segment_previous_main_step = F::from_u64(trace_rows[0].get_main_step());
            air_values.segment_previous_count64 =
                F::from_u32(trace_rows[0].get_count64() + self.op_x_rows as u32);
            air_values.segment_previous_flags = F::from_u16(match first_input.op {
                ZiskOp::DMA_MEMCPY | ZiskOp::DMA_XMEMCPY => F_SEL_MEMCPY,
                ZiskOp::DMA_MEMCMP | ZiskOp::DMA_XMEMCMP => F_SEL_MEMCMP,
                ZiskOp::DMA_INPUTCPY => F_SEL_INPUTCPY,
                ZiskOp::DMA_XMEMSET => F_SEL_MEMSET,
                _ => panic!("Invalid operation 0x{:02X}", first_input.op),
            } as u16);
        }

        #[cfg(feature = "debug_dma")]
        {
            println!("TRACE Dma64AlignedSM @{segment_id} [0] {:?}", trace[0]);
            println!(
                "TRACE Dma64AlignedSM @{segment_id} [{}] {:?}",
                num_rows - 1,
                trace[num_rows - 1]
            );
            println!("TRACE Dma64AlignedSM AIR_VALUES {:?}", air_values);
        }
        timer_stop_and_log_trace!(DMA_64_ALIGNED_TRACE);
        let from_trace = FromTrace::new(&mut trace).with_air_values(&mut air_values);
        Ok(AirInstance::new_from_trace(from_trace))
    }
}
