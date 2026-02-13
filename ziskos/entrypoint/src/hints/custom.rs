use crate::hints::HINT_BUFFER;

#[no_mangle]
pub unsafe extern "C" fn hint_custom(hint_id: u32, data_ptr: *const u8, data_len: usize) {
    if !HINT_BUFFER.is_enabled() {
        return;
    }

    HINT_BUFFER.write_hint_header(hint_id, data_len, false);
    HINT_BUFFER.write_hint_data(data_ptr, data_len);

    let pad = (8 - (data_len & 7)) & 7;
    if pad > 0 {
        const ZERO_PAD: [u8; 8] = [0; 8];
        HINT_BUFFER.write_hint_data(ZERO_PAD.as_ptr(), pad);
    }

    HINT_BUFFER.commit();
}