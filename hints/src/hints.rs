use crate::secp256k1_ecdsa_verify;
use ziskos::syscalls::SyscallPoint256;

use crate::hints_processor::{HINTS_TYPE_ECRECOVER, HINTS_TYPE_RESULT};

pub fn process_hints(hints: Vec<u64>) -> Vec<u64> {
    let mut processed_hints = Vec::new();

    let mut i = 0;
    while i < hints.len() {
        let hint = hints[i];
        i += 1;
        let hint_type = (hint >> 32) as u32;
        let hint_length = (hint & 0xFFFFFFFF) as usize;
        if hint_length == 0 {
            panic!("process_hints() Invalid hint length: {}", hint_length);
        }
        assert!(i + hint_length <= hints.len(), "process_hints() Not enough data for RESULT hint");
        match hint_type {
            HINTS_TYPE_RESULT => {
                // Process result hint: just push the hint data as is
                processed_hints.extend_from_slice(&hints[i..i + hint_length]);
            }
            HINTS_TYPE_ECRECOVER => {
                assert!(
                    hint_length == 8 + 4 + 4 + 4,
                    "process_hints() Invalid ECRECOVER hint length: {}",
                    hint_length
                );
                let pk: &SyscallPoint256 = unsafe { &*(hints[i] as *const SyscallPoint256) };
                let z: &[u64; 4] = unsafe { &*(hints[i + 8] as *const [u64; 4]) };
                let r: &[u64; 4] = unsafe { &*(hints[i + 8 + 4] as *const [u64; 4]) };
                let s: &[u64; 4] = unsafe { &*(hints[i + 8 + 4 + 4] as *const [u64; 4]) };
                secp256k1_ecdsa_verify(pk, z, r, s, &mut processed_hints);
            }
            _ => {
                panic!("process_hints() Unknown hint type: {}", hint_type);
            }
        }
        i += hint_length;
    }

    processed_hints
}
