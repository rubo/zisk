use zisk_common::{OperationDmaMemCpyData, A, B, OPERATION_PRECOMPILED_BUS_DATA_SIZE, STEP};

#[derive(Debug)]
pub struct DmaMemCpyInput {
    pub src: u64,
    pub dst: u64,
    pub count: usize,
    pub main_step: u64,
}

#[cfg(feature = "dma_memcmp")]
#[derive(Debug)]
pub struct DmaMemCmpInput {
    pub addr1: u64,
    pub addr2: u64,
    // number of bytes to compare
    pub count: usize,
    pub main_step: u64,
    // number of bytes from beginning that are equal
    pub count_eq_bytes: usize,
    // results of comparation (p1 - p2)
    // p1 == p2 ==> result == 0
    // p1 > p2 ==> result === 1..255
    // p1 < p2 ==> result === 0xFFFF_FFFF_FFFF_FFFF .. 0xFFFF_FFFF_FFFF_FF00
    pub result: u64,
}

impl DmaMemCpyInput {
    pub fn from(values: &OperationDmaMemCpyData<u64>) -> Self {
        Self {
            dst: values[A],
            src: values[B],
            main_step: values[STEP],
            count: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE] as usize,
        }
    }
}

#[cfg(feature = "dma_memcmp")]
impl DmaMemCmpInput {
    pub fn from(values: &OperationDmaMemCmpData<u64>) -> Self {
        Self {
            addr1: values[A],
            addr2: values[B],
            main_step: values[STEP],
            count: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE] as usize,
            count_eq_bytes: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 1] as usize,
            result: values[OPERATION_PRECOMPILED_BUS_DATA_SIZE + 2],
        }
    }
}
