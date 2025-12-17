extern crate libc;

mod asm_mo;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod asm_mo_runner;
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod asm_mo_runner_stub;
mod asm_mt;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod asm_mt_runner;
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod asm_mt_runner_stub;
mod asm_rh;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod asm_rh_runner;
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
mod asm_rh_runner_stub;
mod asm_runner;
mod asm_services;
mod shmem_reader;
mod shmem_utils;
mod shmem_writer;

pub use asm_mo::*;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use asm_mo_runner::*;
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub use asm_mo_runner_stub::*;
pub use asm_mt::*;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use asm_mt_runner::*;
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub use asm_mt_runner_stub::*;
pub use asm_rh::*;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use asm_rh_runner::*;
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
pub use asm_rh_runner_stub::*;
pub use asm_runner::*;
pub use asm_services::*;
pub use shmem_reader::*;
pub use shmem_utils::*;
pub use shmem_writer::*;

fn build_name(
    prefix: &str,
    port: u16,
    asm_service: AsmService,
    local_rank: i32,
    suffix: &str,
) -> String {
    format!(
        "{}{}_{}_{}",
        prefix,
        AsmServices::shmem_prefix(port, local_rank),
        asm_service.as_str(),
        suffix
    )
}

fn build_shmem_name(port: u16, asm_service: AsmService, local_rank: i32, suffix: &str) -> String {
    build_name("", port, asm_service, local_rank, suffix)
}

fn build_sem_name(port: u16, asm_service: AsmService, local_rank: i32, suffix: &str) -> String {
    build_name("/", port, asm_service, local_rank, suffix)
}

pub fn shmem_input_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_shmem_name(port, asm_service, local_rank, "input")
}

/// Shared memory name for precompile hints data
pub fn shmem_precompile_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_shmem_name(port, asm_service, local_rank, "precompile")
}

/// Shared memory name for precompile hints data
pub fn sem_available_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_sem_name(port, asm_service, local_rank, "prec_avail")
}

/// Shared memory name for precompile hints data
pub fn sem_read_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_sem_name(port, asm_service, local_rank, "prec_read")
}

/// Shared memory name for precompile hints data control
pub fn shmem_control_writer_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_shmem_name(port, asm_service, local_rank, "control_input")
}

pub fn shmem_control_reader_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_shmem_name(port, asm_service, local_rank, "control_output")
}

pub fn shmem_output_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_shmem_name(port, asm_service, local_rank, "output")
}

pub fn sem_chunk_done_name(port: u16, asm_service: AsmService, local_rank: i32) -> String {
    build_sem_name(port, asm_service, local_rank, "chunk_done")
}
