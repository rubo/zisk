use crate::{
    syscalls::{
        syscall_secp256k1_add, syscall_secp256k1_dbl, SyscallPoint256, SyscallSecp256k1AddParams,
    },
    zisklib::{eq, fcall_msb_pos_256, is_one, ONE_256, TWO_256, ZERO_256},
};

use super::{
    constants::{E_B, G, G_X, G_Y, IDENTITY_X, IDENTITY_Y},
    field::{
        secp256k1_fp_add, secp256k1_fp_inv, secp256k1_fp_mul, secp256k1_fp_sqrt,
        secp256k1_fp_square,
    },
    scalar::{secp256k1_fn_inv, secp256k1_fn_mul, secp256k1_fn_reduce, secp256k1_fn_sub},
    secp256k1_decompress, secp256k1_double_scalar_mul_with_g, secp256k1_triple_scalar_mul_with_g,
};

pub fn secp256k1_ecdsa_verify(pk: &[u64; 8], z: &[u64; 4], r: &[u64; 4], s: &[u64; 4]) -> bool {
    // Ecdsa verification computes (x1, y1) = [s⁻¹·z (mod n)]G + [s⁻¹·r (mod n)]pk
    // and checks that r ≡ x1 (mod n)

    // TODO: We can equivalently hint y1 and verify that 𝒪 == [z]G + [r]pk + [-s](x1, y1)
    // saving us from fn arithmetic entirely

    // The recovery algorithm computes pk = [-r⁻¹·z (mod n)]G + [r⁻¹·s (mod n)]R
    // Equivalently, we can verify that [z]G + [r]pk + [-s]R == 𝒪
    let s_inv = secp256k1_fn_inv(s);
    let u1 = secp256k1_fn_mul(z, &s_inv);
    let u2 = secp256k1_fn_mul(r, &s_inv);

    match secp256k1_double_scalar_mul_with_g(&u1, &u2, pk) {
        None => false,
        Some(res) => eq(&secp256k1_fn_reduce(&[res[0], res[1], res[2], res[3]]), r),
    }
}

// ==================== C FFI Functions ====================

// TODO
// /// # Safety
// /// - `pk_ptr` must point to 64 bytes (public key: x[32] || y[32], big-endian)
// /// - `z_ptr` must point to 32 bytes (message hash, big-endian)
// /// - `r_ptr` must point to 32 bytes (signature r, big-endian)
// /// - `s_ptr` must point to 32 bytes (signature s, big-endian)
// ///
// /// Returns true if signature is valid, false otherwise
// #[no_mangle]
// pub unsafe extern "C" fn secp256k1_ecdsa_recover_c(
//     h_ptr: *const u8, // Message hash
//     r_ptr: *const u8, // Signature r
//     s_ptr: *const u8, // Signature s
//     rec_id: u8, // Recovery ID
// ) -> bool {
//     // Helper to convert 32 big-endian bytes to [u64; 4] little-endian limbs
//     #[inline]
//     fn bytes_be_to_u64_le(bytes: *const u8) -> [u64; 4] {
//         let mut result = [0u64; 4];
//         for i in 0..4 {
//             let offset = 24 - i * 8;
//             result[i] = unsafe {
//                 u64::from_be_bytes([
//                     *bytes.add(offset),
//                     *bytes.add(offset + 1),
//                     *bytes.add(offset + 2),
//                     *bytes.add(offset + 3),
//                     *bytes.add(offset + 4),
//                     *bytes.add(offset + 5),
//                     *bytes.add(offset + 6),
//                     *bytes.add(offset + 7),
//                 ])
//             };
//         }
//         result
//     }

//     let h = bytes_be_to_u64_le(h_ptr);
//     let r = bytes_be_to_u64_le(r_ptr);
//     let s = bytes_be_to_u64_le(s_ptr);

//     // secp256k1_ecdsa_verify(&pk, &z, &r, &s)
// }

/// # Safety
/// - `pk_ptr` must point to 8 u64s
/// - `z_ptr`, `r_ptr`, `s_ptr` must point to 4 u64s each
///
/// Returns true if signature is valid
#[no_mangle]
pub unsafe extern "C" fn secp256k1_ecdsa_verify_c(
    pk_ptr: *const u64,
    z_ptr: *const u64,
    r_ptr: *const u64,
    s_ptr: *const u64,
) -> bool {
    let pk: &[u64; 8] = &*(pk_ptr as *const [u64; 8]);
    let z: &[u64; 4] = &*(z_ptr as *const [u64; 4]);
    let r: &[u64; 4] = &*(r_ptr as *const [u64; 4]);
    let s: &[u64; 4] = &*(s_ptr as *const [u64; 4]);
    secp256k1_ecdsa_verify(pk, z, r, s)
}
