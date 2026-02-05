use crate::handlers::validate_hint_length;
use crate::hint_fields;
use crate::zisklib;

use anyhow::Result;

/// Processes an `HINT_SECP256K1_RECOVER` hint.
#[inline]
pub fn secp256k1_recover_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![SIG: 64, RECID: 8, MSG: 32, LO_S: 8];

    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 8) };

    validate_hint_length(bytes, EXPECTED_LEN, "HINT_SECP256K1_RECOVER")?;

    let sig: &[u8; SIG_SIZE] = bytes[SIG_OFFSET..SIG_OFFSET + SIG_SIZE].try_into().unwrap();
    let recid: &[u8; RECID_SIZE] =
        bytes[RECID_OFFSET..RECID_OFFSET + RECID_SIZE].try_into().unwrap();
    let recid: u8 = u64::from_le_bytes(*recid) as u8;
    let msg: &[u8; MSG_SIZE] = bytes[MSG_OFFSET..MSG_OFFSET + MSG_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    let result: &mut [u8; 32] = &mut [0u8; 32];
    unsafe {
        zisklib::secp256k1_ecdsa_address_recover_c(
            sig.as_ptr(),
            recid,
            msg.as_ptr(),
            result.as_mut_ptr(),
            &mut hints,
        );
    }

    Ok(hints)
}
