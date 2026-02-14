use precompiles_common::MemBusHelpers;
use precompiles_common::MemProcessor;
use precompiles_helpers::DmaInfo;
use zisk_common::{A, B, DMA_ENCODED, OP, STEP};
use zisk_core::{zisk_ops::ZiskOp, EXTRA_PARAMS_ADDR};

pub fn generate_dma_memcmp_mem_inputs<P: MemProcessor>(
    data: &[u64],
    data_ext: &[u64],
    mem_processors: &mut P,
) {
    // encoding of count was done with effective count, means that if dst and src are equals,
    // effective_count = count while if dst and src are different effective_count = count_eq + 1
    // count_eq is the number of beggining bytes equal between src and dst

    let dst = data[A];
    let src = data[B];
    let encoded = data[DMA_ENCODED];
    let op = data[OP] as u8;
    let load_count_from_mem = op == ZiskOp::DMA_MEMCMP;
    let dst64 = (dst & !0x07) as u32;
    let src64 = (src & !0x07) as u32;
    let step = data[STEP];
    let pre_count = DmaInfo::get_pre_count(encoded) as u64;
    let dst_offset = dst & 0x07;
    let src_offset = src & 0x07;
    let unaligned = dst_offset != src_offset;
    let count = DmaInfo::get_count(encoded) as u64;

    let src_words = ((src_offset + count + 7) >> 3) as usize;
    let dst_words = ((dst_offset + count + 7) >> 3) as usize;
    debug_assert_eq!(
        src_words + dst_words,
        data_ext.len(),
        "[dma_memcmp] data length mismatch, expected {} but got {} DATA:[{}] INFO={}",
        src_words + dst_words,
        data_ext.len(),
        data.iter().map(|v| format!("0x{v:016X}")).collect::<Vec<String>>().join(", "),
        DmaInfo::to_string(encoded)
    );
    let dst_data = &data_ext[0..dst_words];
    let src_data = &data_ext[dst_words..dst_words + src_words];

    if load_count_from_mem {
        #[cfg(feature = "debug_dma_gen_mem_inputs")]
        println!("[dma_memcmp] INPUT PARAM 0x{EXTRA_PARAMS_ADDR:08X} S:{step}");
        MemBusHelpers::mem_aligned_read(EXTRA_PARAMS_ADDR as u32, step, count, mem_processors);
    }

    if pre_count > 0 {
        #[cfg(feature = "debug_dma_gen_mem_inputs")]
        println!("[dma_memcmp] INPUT PRE SRC:0x{src64:08X} DST:0x{dst64:08X} S:{step}");
        MemBusHelpers::mem_aligned_read(src64, step, src_data[0], mem_processors);
        MemBusHelpers::mem_aligned_read(dst64, step, dst_data[0], mem_processors);

        if DmaInfo::is_double_read_pre(encoded) {
            #[cfg(feature = "debug_dma_gen_mem_inputs")]
            println!("[dma_memcmp] INPUT PRE DOUBLE SRC:0x{:08X} S:{step}", src64 + 8);
            MemBusHelpers::mem_aligned_read(src64 + 8, step, src_data[1], mem_processors);
        }
    }

    // this is part of words loop
    let post_count = DmaInfo::get_post_count(encoded) as u64;
    let loop_count = DmaInfo::get_loop_count(encoded);
    let src_data_offset = ((src_offset + pre_count) > 7) as usize;
    let dst_data_offset = (pre_count > 0) as usize;

    if loop_count > 0 {
        let src64_loop = src64 + src_data_offset as u32 * 8;
        let dst64_loop = dst64 + dst_data_offset as u32 * 8;
        #[cfg(feature = "debug_dma_gen_mem_inputs")]
        println!("[dma_memcmp] INPUT LOOP SRC:0x{src64_loop:08X} DST:0x{dst64_loop:08X} C:{loop_count} S:{step}");
        MemBusHelpers::mem_aligned_read_from_slice(
            src64_loop,
            step,
            &src_data[src_data_offset..src_data_offset + loop_count + unaligned as usize],
            mem_processors,
        );
        MemBusHelpers::mem_aligned_read_from_slice(
            dst64_loop,
            step,
            &dst_data[dst_data_offset..dst_data_offset + loop_count],
            mem_processors,
        );
    }

    let dst_data_offset = dst_data_offset + loop_count;

    if post_count > 0 {
        let src64_post = (src64 + pre_count as u32 + loop_count as u32 * 8) & !0x07;
        let src_data_offset = (src64_post - src64) as usize >> 3;
        let dst64_post = dst64 + dst_data_offset as u32 * 8;

        #[cfg(feature = "debug_dma_gen_mem_inputs")]
        println!("[dma_memcmp] INPUT POST SRC:0x{src64_post:08X} DST:0x{dst64_post:08X} S:{step}");
        MemBusHelpers::mem_aligned_read(
            src64_post,
            step,
            src_data[src_data_offset],
            mem_processors,
        );
        MemBusHelpers::mem_aligned_read(
            dst64_post,
            step,
            dst_data[dst_data_offset],
            mem_processors,
        );

        if DmaInfo::is_double_read_post(encoded) {
            #[cfg(feature = "debug_dma_gen_mem_inputs")]
            println!("[dma_memcmp] INPUT DOUBLE-POST SRC:0x{:08X} S:{step}", src64_post + 8);
            MemBusHelpers::mem_aligned_read(
                src64_post + 8,
                step,
                src_data[src_data_offset + 1],
                mem_processors,
            );
        }
    }
}

pub fn skip_dma_memcmp_mem_inputs<P: MemProcessor>(data: &[u64], mem_processors: &mut P) -> bool {
    let dst = data[A];
    let src = data[B];
    let count = DmaInfo::get_count(data[DMA_ENCODED]) as u64;
    let op = data[OP] as u8;
    let load_count_from_mem = op == ZiskOp::DMA_MEMCMP;

    // calculate range for dst and src to verify if any of them are included
    // in the memcollector addresses.

    let dst64_from = dst as u32 & !0x07;
    let dst64_to = (dst + count + 7) as u32 & !0x07;
    #[cfg(feature = "debug_dma_gen_mem_inputs")]
    let (count64, step) = (dst64_to as u64 - dst64_from as u64 + 1, data[STEP]);
    #[cfg(feature = "debug_dma_gen_mem_inputs")]
    println!("[dma_memcmp] SKIP DST:[0x{dst64_from:08X}..=0x{dst64_to:08X}] C:{count} S:{step}");

    if load_count_from_mem {
        #[cfg(feature = "debug_dma_gen_mem_inputs")]
        println!("[dma_memcmp] SKIP PARAM 0x{EXTRA_PARAMS_ADDR:08X} S:{step}");
        if !mem_processors.skip_addr(EXTRA_PARAMS_ADDR as u32) {
            return false;
        }
    }

    if !mem_processors.skip_addr_range(dst64_from, dst64_to) {
        return false;
    }

    let src64_from = src as u32 & !0x07;
    let src64_to = (src + count + 7) as u32 & !0x07;
    #[cfg(feature = "debug_dma_gen_mem_inputs")]
    let (count64, step) = (dst64_to as u64 - dst64_from as u64 + 1, data[STEP]);

    #[cfg(feature = "debug_dma_gen_mem_inputs")]
    println!("[dma_memcmp] SKIP SRC:[0x{src64_from:08X}..=0x{src64_to:08X}] C:{count} S:{step}");
    if !mem_processors.skip_addr_range(src64_from, src64_to) {
        return false;
    }

    // If any mem_collector includes this addresses we could skip this precompiles
    // at mem input data generation.
    true
}
