// use static_assertions::const_assert;
// const_assert!(CHUNK_MEM_STEP_BITS <= 24);

pub struct DmaHelpers {}

pub struct DmaValues {
    pub dst64: u64,
    pub src64: u64,
    pub src_offset: u64,
    pub dst_offset: u64,
    pub pre_count: u64,
    pub post_count: u64,
    pub memcpy_count: u64,
    pub src64_inc_by_pre: u64,
    pub src_offset_after_pre: u64,
}
impl DmaHelpers {
    #[inline(always)]
    pub fn precalculate_dma_values(dst: u64, src: u64, count: usize) -> DmaValues {
        let dst64 = dst & !0x07;
        let src64 = src & !0x07;
        let dst_offset = dst & 0x07;
        let src_offset = src & 0x07;

        let use_pre = dst_offset > 0;
        let pre_count = if use_pre { std::cmp::min(8 - dst_offset, count as u64) } else { 0 };
        let post_count = (count as u64 - pre_count) % 8;
        let memcpy_count = count as u64 - pre_count - post_count;
        let src64_inc_by_pre = if use_pre && (src_offset + pre_count) >= 8 { 1 } else { 0 };
        let src_offset_after_pre = (src_offset + pre_count) % 8;

        DmaValues {
            dst64,
            src64,
            src_offset,
            dst_offset,
            pre_count,
            post_count,
            memcpy_count,
            src64_inc_by_pre,
            src_offset_after_pre,
        }
    }
}
