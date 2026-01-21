// This example program takes a number `n` as input and computes the SHA-256 hash `n` times sequentially.

// Mark the main function as the entry point for ZisK
#![no_main]
ziskos::entrypoint!(main);

use ziskos::{read_input_slice, set_output};
use zisk_verifier::verify_zisk_proof;

fn main() {
    // Read the input data as a byte array from ziskos
    let zisk_proof = read_input_slice();

    let vk: &[u8] = include_bytes!("../build/zisk.vk.bin");

    let result = verify_zisk_proof(&zisk_proof, &vk);

    set_output(0, result.is_ok() as u32);
}
