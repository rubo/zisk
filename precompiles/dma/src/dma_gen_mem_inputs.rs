use crate::DmaHelpers;
use std::collections::VecDeque;
use zisk_common::BusId;
use zisk_common::MemCollectorInfo;

#[derive(Debug)]
pub struct DmaMemInputConfig {
    pub indirect_params: usize,
    pub rewrite_params: bool,
    pub read_params: usize,
    pub write_params: usize,
    pub chunks_per_param: usize,
}

// all DMA memory operation are aligned

// minimal trace
// reads + writes
const MASK_ALIGNED_ADDR: u64 = !0x07;
/*
pub fn calculate(dst: u64, src: u64, count: u64) -> (usize, usize, usize) {
    let from_src = src & MASK_ALIGNED_ADDR;
    let to_src = (src + count - 1) & MASK_ALIGNED_ADDR;
    let count_src = (to_src - from_src) >> 3;

    let first_dst_addr = dst & MASK_ALIGNED_ADDR;
    let read_first_dst_addr = dst & 0x07 != 0;

    let last_dst_addr = (dst + count - 1) & MASK_ALIGNED_ADDR;
    let read_last_dst_addr = (dst + count) & 0x07 != 0 && last_dst_addr > first_dst_addr;
}
*/

pub fn generate_dma_mem_inputs(
    dst: u64,
    src: u64,
    count: usize,
    _step_main: u64,
    _data: &[u64],
    _data_ext: &[u64],
    _only_counters: bool,
    _pending: &mut VecDeque<(BusId, Vec<u64>)>,
) {
    let from_src = src & !0x07;
    let to_src = (src + count as u64 - 1) & MASK_ALIGNED_ADDR;
    let _count_src = (to_src - from_src) >> 3;

    let first_dst_addr = dst & MASK_ALIGNED_ADDR;
    let _read_first_dst_addr = dst & 0x07 != 0;

    let last_dst_addr = (dst + count as u64 - 1) & MASK_ALIGNED_ADDR;
    let _read_last_dst_addr = (dst + count as u64) & 0x07 != 0 && last_dst_addr > first_dst_addr;

    let _dma = DmaHelpers::precalculate_dma_values(dst, src, count as usize);

    unimplemented!();
    /*
        if dma.pre_count > 0 {
            MemBusHelpers::mem_aligned_load(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
            MemBusHelpers::mem_aligned_load(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );

            MemBusHelpers::mem_aligned_load(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
            MemBusHelpers::mem_aligned_write(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
        }

        for i in 0..dma.memcpy_count / 8 {
            MemBusHelpers::mem_aligned_load(
                src_aligned + i as u32 * 8,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + i as usize],
                pending,
            );
            MemBusHelpers::mem_aligned_write(
                dst_aligned + i as u32 * 8,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + i as usize],
                pending,
            );
        }

        if dma.post_count > 0 {
            MemBusHelpers::mem_aligned_load(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
            MemBusHelpers::mem_aligned_load(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
            MemBusHelpers::mem_aligned_write(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
        }
    */
    /*
    // Information collected during memory trace generation includes the alignated reads:
    // - previous read dst & ~0x07 if dst % 8 > 0
    // - previous read (dst + count - 1) & ~0x07 if use post
    // - src reads from: src & ~0x07 to (src + count - 1) & ~0x07


    let write_addr = dst & ~0x07;
    let write_count = (((dst + count - 1) - write_addr - 1) / 8) + 1;



    // full aligned src % 8 == 0 && dst % 8 == 0
    // parcialy aligned src % 8 == dst % 8 == 0


    // block
    let mut read_index = 0;
    let mut align_write_addr = 0;

    if offset == 0 {

    } else {
        for i in 0..count {
            MemBusHelpers::mem_aligned_store(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
        }
    }

    let src_offset = dst & 0x07;
    let dst_offset = src & 0x07;

    if count <= dst_offset {
        if count > 0 {
            let dst_aligned = dst & 0xFFFF_FFFF_FFFF_FFF8;

            MemBusHelpers::mem_aligned_load(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );

            MemBusHelpers::mem_aligned_store(
                dst_aligned,
                step_main,
                data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 0],
                pending,
            );
        }
    } else {
        if dst_offset > 0 {
            self.dma_pre_pos_ops += 1
        }
        if dst_offset == src_offset {
            self.dma_64_aligned_ops += (count - dst_offset) >> 3
        } else {
            self.dma_unaligned_ops += (count - dst_offset) >> 3
        }

        if (count - dst_offset) % 8 > 0 {
            self.dma_pre_pos_ops += 1
        }
    }
    self.dma_ops += 1;

    let dst = data[A];
    let src = data[B];

    //
    // precompiled_mem_load( sel: enabled, main_step: main_step, addr: src_addr * 8,     value: [values[0], values[1]]);
    // precompiled_mem_load( sel: enabled_second_read, main_step: main_step, addr: src_addr * 8 + 8, value: [values[2], values[3]]);
    // precompiled_mem_load( sel: enabled, main_step: main_step, addr: dst_addr * 8,     value: [values[4], values[5]]);
    // precompiled_mem_store(sel: enabled, main_step: main_step, addr: dst_addr * 8,     value: write_value);

    let src_aligned = src & 0xFFFF_FFFF_FFFF_FFF8;
    MemBusHelpers::mem_aligned_load(
        src_aligned + iparam as u32 * 8,
        step_main,
        data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + iparam],
        pending,
    );

    // ALIGNED, UNALIGNED
    for iparam in 0..PARAMS {
        MemBusHelpers::mem_aligned_load(
            src + iparam as u32 * 8,
            step_main,
            data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + iparam],
            pending,
        );
    }

    // generate load params
    for iparam in 0..READ_PARAMS {
        let param_addr = data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + iparam] as u32;
        for ichunk in 0..PARAM_CHUNKS {
            MemBusHelpers::mem_aligned_load(
                param_addr + ichunk as u32 * 8,
                step_main,
                data[START_READ_PARAMS + iparam * PARAM_CHUNKS + ichunk],
                pending,
            );
        }
    }

    let mut write_data = [0u64; PARAM_CHUNKS];
    if !only_counters {
        let a: [u64; 4] =
            data[START_READ_PARAMS..START_READ_PARAMS + PARAM_CHUNKS].try_into().unwrap();
        let b: [u64; 4] = data
            [START_READ_PARAMS + PARAM_CHUNKS..START_READ_PARAMS + 2 * PARAM_CHUNKS]
            .try_into()
            .unwrap();
        dma(&a, &b, data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + READ_PARAMS], &mut write_data);
    }

    // verify write param
    let write_addr = data[OPERATION_PRECOMPILED_BUS_DATA_SIZE + WRITE_ADDR_PARAM] as u32;
    for (ichunk, write_data) in write_data.iter().enumerate().take(PARAM_CHUNKS) {
        let param_addr = write_addr + ichunk as u32 * 8;
        MemBusHelpers::mem_aligned_write(param_addr, step_main, *write_data, pending);
    }*/
}

// op_a = step
// op_b = addr_main
// mem_trace: @a, @b, cin, @c, a[0..3], b[0..3], cout, [ c[0..3] ]

pub fn skip_dma_mem_inputs(
    dst: u64,
    src: u64,
    count: usize,
    mem_collectors_info: &[MemCollectorInfo],
) -> bool {
    let to_dst = (dst + count as u64 - 1) as u32 & !0x07;
    let to_src = (src + count as u64 - 1) as u32 & !0x07;
    let mut dst = (dst & !0x07) as u32;
    let mut src = (src & !0x07) as u32;
    // TODO:
    // for mem_collector in mem_collectors_info {
    //     if !mem_collector.skip_addr_range(from_dst, to_dst) ||
    //        !mem_collector.skip_addr_range(from_src, to_src) {
    //         return false;
    //     }
    // }

    while dst <= to_dst {
        for mem_collector in mem_collectors_info {
            if !mem_collector.skip_addr(dst) {
                return false;
            }
        }
        dst += 8;
    }
    while src <= to_src {
        for mem_collector in mem_collectors_info {
            if !mem_collector.skip_addr(src) {
                return false;
            }
        }
        src += 8;
    }
    true
}
