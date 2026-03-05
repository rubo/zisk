use crate::{shmem_control_writer_name, AsmServices, SharedMemoryWriter};

use anyhow::Result;

pub struct ControlShmem {
    writer: SharedMemoryWriter,
}

#[derive(Copy, Clone)]
pub enum ControlShmemOffsets {
    PrecompilesSize = 0,
    ShutdownFlag = 8,
    InputsSize = 16,
}

impl ControlShmem {
    pub const CONTROL_WRITER_SIZE: u64 = 0x1000; // 4KB

    pub fn new(
        base_port: Option<u16>,
        local_rank: i32,
        unlock_mapped_memory: bool,
    ) -> Result<Self> {
        let port = AsmServices::port_base_for(base_port, local_rank);
        Ok(Self {
            writer: SharedMemoryWriter::new(
                &shmem_control_writer_name(port, local_rank),
                Self::CONTROL_WRITER_SIZE as usize,
                unlock_mapped_memory,
            )?,
        })
    }

    pub fn read_u64_at(&self, offset: ControlShmemOffsets) -> u64 {
        self.writer.read_u64_at(offset as usize)
    }

    pub fn write_u64_at(&self, offset: ControlShmemOffsets, size: u64) {
        self.writer.write_u64_at(offset as usize, size);
    }

    pub fn increment_u64_at(&self, offset: ControlShmemOffsets, size: usize) {
        let current_size = self.read_u64_at(offset);
        self.write_u64_at(offset, current_size + size as u64);
    }

    pub fn reset(&self) {
        self.write_u64_at(ControlShmemOffsets::PrecompilesSize, 0);
        self.write_u64_at(ControlShmemOffsets::ShutdownFlag, 0);
        self.write_u64_at(ControlShmemOffsets::InputsSize, 0);
    }
}
