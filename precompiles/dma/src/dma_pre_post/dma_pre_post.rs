use std::sync::Arc;

use fields::PrimeField64;

use pil_std_lib::Std;
use proofman_common::{AirInstance, FromTrace, ProofmanResult};
use proofman_util::{timer_start_trace, timer_stop_and_log_trace};
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::{ParallelSlice, ParallelSliceMut},
};
use zisk_core::zisk_ops::ZiskOp;
use zisk_pil::{
    DMA_BYTE_CMP_TABLE_ID, DMA_PRE_POST_TABLE_ID, DMA_PRE_POST_TABLE_SIZE, DUAL_RANGE_BYTE_ID,
};

#[cfg(feature = "packed")]
pub use zisk_pil::{
    DmaPrePostTracePacked as DmaPrePostTrace, DmaPrePostTraceRowPacked as DmaPrePostTraceRow,
};

#[cfg(not(feature = "packed"))]
pub use zisk_pil::{DmaPrePostTrace, DmaPrePostTraceRow};

use crate::{DmaPrePostInput, DmaPrePostModule, DmaPrePostRom};
use precompiles_helpers::DmaInfo;

// Type aliases to simplify complex types
type MultTable = Vec<Vec<u64>>;
type PrePostAndByteCmpTables = (MultTable, MultTable);
type GlobalMultiplicities = (PrePostAndByteCmpTables, MultTable);

/// The `DmaPrePostSM` struct encapsulates the logic of the DmaPrePost State Machine.
pub struct DmaPrePostSM<F: PrimeField64> {
    /// Reference to the PIL2 standard library.
    pub std: Arc<Std<F>>,

    /// Range checks ID's
    pre_post_table_id: usize,

    /// Table to verify byte comparison
    byte_cmp_table_id: usize,

    /// Dual Byte Range checks
    dual_range_byte_id: usize,
}

impl<F: PrimeField64> DmaPrePostSM<F> {
    /// Creates a new Dma State Machine instance.
    ///
    /// # Returns
    /// A new `DmaPrePostSM` instance.
    pub fn new(std: Arc<Std<F>>) -> Arc<Self> {
        Arc::new(Self {
            std: std.clone(),
            dual_range_byte_id: std
                .get_virtual_table_id(DUAL_RANGE_BYTE_ID)
                .expect("Failed to get table DUAL_RANGE_BYTE indentifer"),
            byte_cmp_table_id: std
                .get_virtual_table_id(DMA_BYTE_CMP_TABLE_ID)
                .expect("Failed to get table DMA_BYTE_CMP_TABLE indentifier"),
            pre_post_table_id: std
                .get_virtual_table_id(DMA_PRE_POST_TABLE_ID)
                .expect("Failed to get table DMA_PRE_POST_TABLE identifier"),
        })
    }

    /// Processes a slice of operation data, updating the trace.
    ///
    /// # Arguments
    /// * `trace` - A mutable reference to the Dma trace.
    /// * `input` - The operation data to process.
    #[inline(always)]
    pub fn process_slice(
        &self,
        input: &DmaPrePostInput,
        trace: &mut DmaPrePostTraceRow<F>,
        pre_post_table_mul: &mut [u64],
        byte_cmp_table_mul: &mut [u64],
        local_dual_range_byte_mul: &mut [u64],
    ) {
        let is_memcmp = input.op == ZiskOp::DMA_MEMCMP || input.op == ZiskOp::DMA_XMEMCMP;
        let is_memcpy = input.op == ZiskOp::DMA_MEMCPY || input.op == ZiskOp::DMA_XMEMCPY;
        let is_memset = input.op == ZiskOp::DMA_XMEMSET;
        let is_inputcpy = input.op == ZiskOp::DMA_INPUTCPY;
        let load_src = is_memcpy || is_memcmp;

        let dst_offset = input.dst & 0x07;
        let src_offset = if load_src { input.src & 0x07 } else { 0 };
        let is_pre = dst_offset > 0;
        let step = input.step;

        let dst64 = input.dst >> 3;
        let src64 = input.src >> 3;

        trace.set_main_step(input.step);
        trace.set_dst64(dst64);
        trace.set_src64(src64);
        trace.set_dst_offset(dst_offset as u8);
        trace.set_src_offset(src_offset as u8);
        trace.set_is_post(!is_pre);

        let count = if is_pre {
            DmaInfo::get_pre_count(input.encoded)
        } else {
            DmaInfo::get_post_count(input.encoded)
        };

        trace.set_count(count as u8);

        trace.set_sel_memcpy(is_memcpy);
        trace.set_sel_memset(is_memset);
        trace.set_sel_inputcpy(is_inputcpy);
        trace.set_sel_memcmp(is_memcmp);

        let fill_byte = DmaInfo::get_fill_byte(input.encoded);
        if is_memset {
            trace.set_fill_byte(fill_byte);
        }
        let second_read = (src_offset as usize + count) > 8;
        //println!("SECOND_READ: {second_read}");
        trace.set_enabled_second_read(second_read);

        let mut value = input.src_values[0];
        let mut rb = [0u8; 16];
        let mut pb = [0u8; 8];

        if is_memset {
            for rb in rb.iter_mut() {
                *rb = fill_byte;
            }
        } else {
            rb[0] = value as u8;
            rb[1] = (value >> 8) as u8;
            rb[2] = (value >> 16) as u8;
            rb[3] = (value >> 24) as u8;
            rb[4] = (value >> 32) as u8;
            rb[5] = (value >> 40) as u8;
            rb[6] = (value >> 48) as u8;
            rb[7] = (value >> 56) as u8;

            local_dual_range_byte_mul[(value & 0xFFFF) as usize] += 1;
            local_dual_range_byte_mul[((value >> 16) & 0xFFFF) as usize] += 1;
            local_dual_range_byte_mul[((value >> 32) & 0xFFFF) as usize] += 1;
            local_dual_range_byte_mul[((value >> 48) & 0xFFFF) as usize] += 1;

            if second_read {
                value = input.src_values[1];
                rb[8] = value as u8;
                rb[9] = (value >> 8) as u8;
                rb[10] = (value >> 16) as u8;
                rb[11] = (value >> 24) as u8;
                rb[12] = (value >> 32) as u8;
                rb[13] = (value >> 40) as u8;
                rb[14] = (value >> 48) as u8;
                rb[15] = (value >> 56) as u8;
                local_dual_range_byte_mul[(value & 0xFFFF) as usize] += 1;
                local_dual_range_byte_mul[((value >> 16) & 0xFFFF) as usize] += 1;
                local_dual_range_byte_mul[((value >> 32) & 0xFFFF) as usize] += 1;
                local_dual_range_byte_mul[((value >> 48) & 0xFFFF) as usize] += 1;
            } else {
                local_dual_range_byte_mul[0] += 4;
            }
        }

        value = input.dst_pre_value;
        pb[0] = value as u8;
        pb[1] = (value >> 8) as u8;
        pb[2] = (value >> 16) as u8;
        pb[3] = (value >> 24) as u8;
        pb[4] = (value >> 32) as u8;
        pb[5] = (value >> 40) as u8;
        pb[6] = (value >> 48) as u8;
        pb[7] = (value >> 56) as u8;

        local_dual_range_byte_mul[(value & 0xFFFF) as usize] += 1;
        local_dual_range_byte_mul[((value >> 16) & 0xFFFF) as usize] += 1;
        local_dual_range_byte_mul[((value >> 32) & 0xFFFF) as usize] += 1;
        local_dual_range_byte_mul[((value >> 48) & 0xFFFF) as usize] += 1;

        let selr_value = if dst_offset > src_offset {
            trace.set_dst_offset_gt_src_offset(true);
            dst_offset - src_offset
        } else {
            trace.set_dst_offset_gt_src_offset(false);
            src_offset - dst_offset
        };

        let read_value_23 =
            if selr_value > 0 { input.src_values[0] << (selr_value * 8) } else { 0 };
        let read_value_01 = (input.src_values[0] >> (selr_value * 8))
            | if selr_value > 0 { input.src_values[1] << (64 - selr_value * 8) } else { 0 };

        let _mask = 0xFFFF_FFFF_FFFF_FFFFu64 << (dst_offset * 8);
        let mask = _mask ^ (_mask << (count * 8));

        let write_value_01 = (read_value_01 & mask) | (input.dst_pre_value & !mask);
        let write_value_23 = (read_value_23 & mask) | (input.dst_pre_value & !mask);

        trace.set_write_value(0, write_value_01 as u32);
        trace.set_write_value(1, (write_value_01 >> 32) as u32);
        trace.set_write_value(2, write_value_23 as u32);
        trace.set_write_value(3, (write_value_23 >> 32) as u32);

        trace.set_sb(0, (mask & 0x0000_0000_0000_00FF) != 0);
        trace.set_sb(1, (mask & 0x0000_0000_0000_FF00) != 0);
        trace.set_sb(2, (mask & 0x0000_0000_00FF_0000) != 0);
        trace.set_sb(3, (mask & 0x0000_0000_FF00_0000) != 0);
        trace.set_sb(4, (mask & 0x0000_00FF_0000_0000) != 0);
        trace.set_sb(5, (mask & 0x0000_FF00_0000_0000) != 0);
        trace.set_sb(6, (mask & 0x00FF_0000_0000_0000) != 0);
        trace.set_sb(7, (mask & 0xFF00_0000_0000_0000) != 0);

        for (index, byte) in rb.iter().enumerate() {
            // println!("PRE-POST bytes[{index}]: 0x{byte:02X}");
            trace.set_rb(index, *byte);
        }
        for (index, byte) in pb.iter().enumerate() {
            // println!("PRE-POST bytes[{index}]: 0x{byte:02X}");
            trace.set_pb(index, *byte);
        }

        trace.set_selr(0, selr_value == 0);
        trace.set_selr(1, selr_value == 1);
        trace.set_selr(2, selr_value == 2);
        trace.set_selr(3, selr_value == 3);
        trace.set_selr(4, selr_value == 4);
        trace.set_selr(5, selr_value == 5);
        trace.set_selr(6, selr_value == 6);

        let table_row = if is_memcmp {
            let result = DmaInfo::get_memcmp_res_as_u64(input.encoded);
            let is_negative = DmaInfo::is_memcmp_negative(input.encoded);
            let is_nz = result != 0;
            trace.set_memcmp_result_is_negative(is_negative);
            trace.set_memcmp_result_nz(is_nz);
            let abs_diff_dst_src = if is_negative { (!result).wrapping_add(1) } else { result };
            assert!(abs_diff_dst_src <= 0xFF);
            let abs_diff_dst_src = abs_diff_dst_src as u8;
            trace.set_abs_diff_dst_src(abs_diff_dst_src);

            // the index of different byte determines the factor
            let dst_index = dst_offset as usize + count - 1;
            if is_negative {
                // implies that count > 0
                if count < 5 {
                    trace.set_diff_factor(0, F::ORDER_U64 - (1 << (8 * dst_index)));
                    trace.set_diff_factor(1, 0);
                } else {
                    trace.set_diff_factor(0, 0);
                    trace.set_diff_factor(1, F::ORDER_U64 - (1 << (8 * (dst_index - 4))));
                }
            } else if is_nz {
                if count < 5 {
                    trace.set_diff_factor(0, 1 << (8 * dst_index));
                    trace.set_diff_factor(1, 0);
                } else {
                    trace.set_diff_factor(0, 0);
                    trace.set_diff_factor(1, 1 << (8 * (dst_index - 4)));
                }
            }

            // calculate the contribution to byte_cmp_table multiplicity
            if is_nz {
                let last_dst_byte = pb[dst_index];
                let row_byte_cmp_table = if is_negative {
                    assert!(
                        abs_diff_dst_src >= last_dst_byte,
                        "abs_diff_dst_src: {abs_diff_dst_src} last_dst_byte: 0x{last_dst_byte:02X} result: 0x{result:016X} S:{step}",
                    );
                    last_dst_byte as usize * 255 + (abs_diff_dst_src + last_dst_byte) as usize - 1
                } else {
                    assert!(
                        abs_diff_dst_src <= last_dst_byte,
                        "abs_diff_dst_src: {abs_diff_dst_src} last_dst_byte: 0x{last_dst_byte:02X} result: 0x{result:016X} S:{step}",
                    );
                    last_dst_byte as usize * 255 + (last_dst_byte - abs_diff_dst_src) as usize
                };
                // println!("\x1B[1;41mBYTE_CMP_TABLE[{row_byte_cmp_table}] abs_diff_dst_src: {abs_diff_dst_src} last_dst_byte: 0x{last_dst_byte:02X} is_negative:{is_negative} result: 0x{result:016X} S:{step}\x1B[0m");
                byte_cmp_table_mul[row_byte_cmp_table] += 1;
            }
            DmaPrePostRom::get_row(
                dst_offset as usize,
                src_offset as usize,
                count,
                is_nz,
                is_negative,
                true,
            )
        } else {
            DmaPrePostRom::get_row(
                dst_offset as usize,
                src_offset as usize,
                count,
                false,
                false,
                load_src,
            )
        };

        pre_post_table_mul[table_row] += 1;
    }
}

impl<F: PrimeField64> DmaPrePostModule<F> for DmaPrePostSM<F> {
    fn get_name(&self) -> &'static str {
        "dma_pre_post"
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
        inputs: &[Vec<DmaPrePostInput>],
        trace_buffer: Vec<F>,
    ) -> ProofmanResult<AirInstance<F>> {
        let mut trace = DmaPrePostTrace::<F>::new_from_vec_zeroes(trace_buffer)?;
        let num_rows = trace.num_rows();

        let total_inputs: usize = inputs.iter().map(|inputs| inputs.len()).sum();

        assert!(total_inputs <= num_rows);
        assert!(total_inputs > 0);

        tracing::debug!(
            "··· Creating DmaPrePost instance [{total_inputs} / {num_rows} rows filled {:.2}%]",
            total_inputs as f64 / num_rows as f64 * 100.0
        );

        timer_start_trace!(DMA_PRE_POST_TRACE);

        // Split the dma_trace.buffer into slices matching each inner vector’s length.
        let flat_inputs: Vec<_> = inputs.iter().flatten().collect();
        let trace_rows = trace.buffer.as_mut_slice();

        // Calculate optimal chunk size
        let num_threads = rayon::current_num_threads();
        let chunk_size = std::cmp::max(1, flat_inputs.len() / num_threads);

        // Process in chunks to allow per-chunk local multiplicities arrays
        let ((global_pre_post_table_mul, global_byte_cmp_table_mul), global_dual_range_byte_mul): GlobalMultiplicities =
            flat_inputs
            .par_chunks(chunk_size)
            .zip(trace_rows.par_chunks_mut(chunk_size))
            .map(|(input_chunk, trace_chunk)| {
                // Local array shared by this chunk
                let mut local_pre_post_table_mul = vec![0u64; DMA_PRE_POST_TABLE_SIZE];
                let mut local_dual_range_byte_mul = vec![0u64; 1 << 16];
                let mut local_byte_cmp_table_mul = vec![0u64; 256 * 255];

                // Sum all local arrays into a global one
                for (input, trace_row) in input_chunk.iter().zip(trace_chunk.iter_mut()) {
                    self.process_slice(
                        input,
                        trace_row,
                        &mut local_pre_post_table_mul,
                        &mut local_byte_cmp_table_mul,
                        &mut local_dual_range_byte_mul,
                    )
                }

                // Return nested tuple for unzip
                ((local_pre_post_table_mul, local_byte_cmp_table_mul), local_dual_range_byte_mul)
            })
            .unzip();
        for pre_post_table_mul in global_pre_post_table_mul.iter() {
            // println!("PRE_POST_TABLE_MUL {:?}", pre_post_table_mul);
            self.std.inc_virtual_rows_ranged(self.pre_post_table_id, pre_post_table_mul);
        }

        for byte_cmp_table_mul in global_byte_cmp_table_mul.iter() {
            // println!("PRE_POST_TABLE_MUL {:?}", pre_post_table_mul);
            self.std.inc_virtual_rows_ranged(self.byte_cmp_table_id, byte_cmp_table_mul);
        }

        for dual_range_byte_mul in global_dual_range_byte_mul.iter() {
            self.std.inc_virtual_rows_ranged(self.dual_range_byte_id, dual_range_byte_mul);
        }
        let from_trace = FromTrace::new(&mut trace);
        timer_stop_and_log_trace!(DMA_PRE_POST_TRACE);
        Ok(AirInstance::new_from_trace(from_trace))
    }
}
