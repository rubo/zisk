use std::sync::{Arc, Mutex};

use named_sem::NamedSemaphore;
use zisk_common::{io::StreamSink, reinterpret_vec};
use zisk_core::MAX_INPUT_SIZE;

use crate::{
    shmem_input_avail_name, shmem_input_name, AsmInputHeader, AsmServices, ControlShmem,
    SharedMemoryWriter,
};

use anyhow::Result;

pub struct InputsShmemWriter {
    writer: Mutex<SharedMemoryWriter>,
    control_writer: Arc<ControlShmem>,
    sem_avail: Mutex<NamedSemaphore>,
}

unsafe impl Send for InputsShmemWriter {}
unsafe impl Sync for InputsShmemWriter {}

impl InputsShmemWriter {
    pub fn new(
        base_port: Option<u16>,
        local_rank: i32,
        unlock_mapped_memory: bool,
        control_writer: Arc<ControlShmem>,
    ) -> Result<Self> {
        let port = AsmServices::port_base_for(base_port, local_rank);

        let mut writer = SharedMemoryWriter::new(
            &shmem_input_name(port, local_rank),
            MAX_INPUT_SIZE as usize,
            unlock_mapped_memory,
        )?;

        writer.reset();
        writer.append_input(&0u64.to_le_bytes())?;

        let sem_avail = Mutex::new(NamedSemaphore::create(
            shmem_input_avail_name(port, local_rank).clone(),
            0,
        )?);

        Ok(Self { writer: Mutex::new(writer), control_writer, sem_avail })
    }

    pub fn write_input(&self, inputs: &[u8]) -> Result<()> {
        const HEADER_SIZE: usize = size_of::<AsmInputHeader>();

        let shmem_input_size = (HEADER_SIZE + inputs.len() + 7) & !7;

        let mut full_input = Vec::with_capacity(shmem_input_size);

        let header = AsmInputHeader { zero: 0, input_data_size: 0 };
        full_input.extend_from_slice(&header.to_bytes());
        full_input.extend_from_slice(inputs);
        while full_input.len() < shmem_input_size {
            full_input.push(0);
        }

        let writer = self.writer.lock().unwrap();
        writer.write_input(&full_input)?;
        writer.write_u64_at(8, inputs.len() as u64);

        Ok(())
    }

    pub fn append_input(&self, inputs: &[u8]) -> Result<()> {
        self.writer.lock().unwrap().append_input(&inputs)?;
        self.control_writer.inc_inputs_size(inputs.len());
        self.sem_avail.lock().unwrap().post()?;

        Ok(())
    }

    pub fn reset(&self) {
        let mut writer = self.writer.lock().unwrap();
        writer.reset();
        writer
            .append_input(&0u64.to_le_bytes())
            .expect("Failed to write initial header after reset");

        self.control_writer.reset();
        let mut sem_avail_guard = self.sem_avail.lock().unwrap();
        while sem_avail_guard.try_wait().is_ok() {}
    }
}

impl StreamSink for InputsShmemWriter {
    fn submit(&self, hints: Vec<u64>) -> anyhow::Result<()> {
        self.append_input(&reinterpret_vec(hints)?)
    }

    fn reset(&self) {
        self.reset();
    }
}
