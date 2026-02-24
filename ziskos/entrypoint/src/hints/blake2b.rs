use crate::hints::{HINT_BUFFER, macros::{define_hint, register_hint_meta}};
use zisk_common::{
    HINT_BLAKE2B_COMPRESS,
};

#[no_mangle]
pub unsafe extern "C" fn hint_blake2b_compress(
    rounds: u32,
    state: *mut u64,
    message: *const u64,
    offset: *const u64,
    final_block: u8,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) {
    if !HINT_BUFFER.is_enabled() {
        return;
    }

    #[cfg(zisk_hints_single_thread)]
    crate::hints::check_main_thread();

    let total_len = 8 + 64 + 128 + 16 + 8; // rounds + state + message + offset + final_block

    let mut w = HINT_BUFFER.begin_hint(HINT_BLAKE2B_COMPRESS, total_len, false);

    let rounds_bytes: [u8; 8] = (rounds as u64).to_le_bytes();
    w.write_hint_data_slice(&rounds_bytes);

    w.write_hint_data_ptr(state as *const u8, 64);
    w.write_hint_data_ptr(message as *const u8, 128);
    w.write_hint_data_ptr(offset as *const u8, 16);

    let final_block_bytes: [u8; 8] = (final_block as u64).to_le_bytes();
    w.write_hint_data_slice(&final_block_bytes);

    w.commit();
}

register_hint_meta!(blake2b_compress, HINT_BLAKE2B_COMPRESS);