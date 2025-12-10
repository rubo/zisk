use crate::dma_constants::*;
use zisk_common::OperationDmaData;
use zisk_common::{B, OPERATION_PRECOMPILED_BUS_DATA_SIZE, STEP};

#[derive(Debug)]
pub struct MemCpyInput {
    pub step_main: u64,
    pub addr_src: u32,
    pub addr_dst: u32,
    pub a,
    pub a: [u64; 4],
    pub b: [u64; 4],
    pub count: u32,
}

impl MemCpyInput {
    pub fn from(values: &OperationDmaData<u64>) -> Self {
        Self {
            step_main: values[STEP],
            addr_main: values[B] as u32,
            addr_a: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE] as u32,
            addr_b: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 1] as u32,
            addr_c: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE + READ_PARAMS + DIRECT_READ_PARAMS]
                as u32,
            cin: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE + READ_PARAMS],
            a: values[START_READ_PARAMS..START_READ_PARAMS + PARAM_CHUNKS].try_into().unwrap(),
            b: values[START_READ_PARAMS + PARAM_CHUNKS..START_READ_PARAMS + 2 * PARAM_CHUNKS]
                .try_into()
                .unwrap(),
        }
    }
}
