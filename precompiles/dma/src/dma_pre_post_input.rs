use crate::dma_constants::*;
use zisk_common::OperationDmaData;
use zisk_common::{B, OPERATION_PRECOMPILED_BUS_DATA_SIZE, STEP};

#[derive(Debug)]
pub struct DmaPrePostInput {
    pub src: u32,
    pub dst: u32,
    pub step: u64,
    pub count: u8,
    pub src_values: [u64; 2],
    pub dst_pre_value: u64,
}

impl DmaPrePostInput {
    pub fn from(data: &OperationDmaData<u64>, data_ext: &[u64]) -> Self {
        Self {
            dst: data[A] as u32,
            src: data[B] as u32,
            step: data[STEP],
            src_values: [data_ext[2], data_ext[3]],
            dst_pre_value: data_ext[0],
        }
    }
}
