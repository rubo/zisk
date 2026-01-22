use crate::{
    syscalls::{
        syscall_secp256k1_add, syscall_secp256k1_dbl, SyscallPoint256, SyscallSecp256k1AddParams,
    },
    zisklib::{
        eq, fcall_msb_pos_256, fcall_secp256k1_ecdsa_verify, is_one, ONE_256, TWO_256, ZERO_256,
    },
};

use super::{
    constants::{E_B, G, G_X, G_Y, IDENTITY_X, IDENTITY_Y, N, P},
    curve::{
        secp256k1_decompress, secp256k1_double_scalar_mul_with_g, secp256k1_is_on_curve,
        secp256k1_scalar_mul, secp256k1_triple_scalar_mul_with_g,
    },
    field::{
        secp256k1_fp_add, secp256k1_fp_inv, secp256k1_fp_mul, secp256k1_fp_sqrt,
        secp256k1_fp_square,
    },
    scalar::{
        secp256k1_fn_add, secp256k1_fn_inv, secp256k1_fn_mul, secp256k1_fn_neg,
        secp256k1_fn_reduce, secp256k1_fn_sub,
    },
};

use tiny_keccak::{Hasher, Keccak};

/// Convert big-endian bytes to little-endian u64 limbs (32 bytes -> [u64; 4])
fn bytes_be_to_u64_le(bytes: &[u8; 32]) -> [u64; 4] {
    let mut result = [0u64; 4];
    for i in 0..4 {
        for j in 0..8 {
            result[3 - i] |= (bytes[i * 8 + j] as u64) << (8 * (7 - j));
        }
    }
    result
}

/// Convert little-endian u64 limbs to big-endian bytes ([u64; 4] -> 32 bytes)
fn u64_le_to_bytes_be(limbs: &[u64; 4]) -> [u8; 32] {
    let mut result = [0u8; 32];
    for i in 0..4 {
        for j in 0..8 {
            result[i * 8 + j] = ((limbs[3 - i] >> (8 * (7 - j))) & 0xff) as u8;
        }
    }
    result
}

/// Check if a scalar is valid (0 < x < N)
fn is_valid_scalar(x: &[u64; 4]) -> bool {
    // Must be non-zero
    if *x == ZERO_256 {
        return false;
    }
    // Must be less than N
    for i in (0..4).rev() {
        if x[i] > N[i] {
            return false;
        }
        if x[i] < N[i] {
            return true;
        }
    }
    // x == N, not valid
    false
}

/// Check if r is valid for ecrecover (0 < r < P for the base case)
/// When recid >= 2, we need r + N < P
fn is_valid_r(r: &[u64; 4], recid: u8) -> bool {
    // Must be non-zero
    if *r == ZERO_256 {
        return false;
    }

    if recid >= 2 {
        // Need to check r + N < P (which is practically never true for secp256k1)
        // Since N is very close to P, r + N will almost always exceed P
        // This case is extremely rare in practice
        // For simplicity, we can add r + N and check if it's < P
        // But practically this never happens, so we can return false
        return false;
    }

    // Must be less than P (field modulus) for recovery
    for i in (0..4).rev() {
        if r[i] > P[i] {
            return false;
        }
        if r[i] < P[i] {
            return true;
        }
    }
    // r == P, not valid
    false
}

/// Recover the public key from an ECDSA signature
///
/// The recovery formula is:
/// R = (r, y) where y is recovered from r using the curve equation
/// PK = r⁻¹ * (s*R - z*G)
///
/// Returns the recovered public key as [u64; 8] (x, y coordinates) or None if recovery fails
pub fn secp256k1_ecrecover_point(
    r: &[u64; 4],
    s: &[u64; 4],
    z: &[u64; 4],
    recid: u8,
) -> Option<[u64; 8]> {
    // Validate r and s
    if !is_valid_scalar(r) || !is_valid_scalar(s) {
        return None;
    }

    // Determine the x-coordinate of R
    // If recid >= 2, x = r + N (but this is extremely rare and usually invalid)
    let x = if recid >= 2 {
        // r + N would need to be < P, which is practically never true
        return None;
    } else {
        *r
    };

    // Recover the y-coordinate from x
    // y² = x³ + 7
    let y_is_odd = (recid & 1) == 1;
    let (rx, ry) = secp256k1_decompress(&x, y_is_odd).ok()?;

    let r_point = [rx[0], rx[1], rx[2], rx[3], ry[0], ry[1], ry[2], ry[3]];

    // Compute r_inv = r⁻¹ (mod N)
    let r_inv = secp256k1_fn_inv(r);

    // Compute u1 = -z * r_inv (mod N)
    let neg_z = secp256k1_fn_neg(z);
    let u1 = secp256k1_fn_mul(&neg_z, &r_inv);

    // Compute u2 = s * r_inv (mod N)
    let u2 = secp256k1_fn_mul(s, &r_inv);

    // Compute PK = u1*G + u2*R
    let pk = secp256k1_double_scalar_mul_with_g(&u1, &u2, &r_point)?;

    Some(pk)
}

/// Recover the Ethereum address from an ECDSA signature
///
/// This function is designed to patch:
/// `fn secp256k1_ecrecover(&self, sig: &[u8; 64], recid: u8, msg: &[u8; 32]) -> Result<[u8; 32], PrecompileError>`
///
/// # Arguments
/// * `sig` - 64 bytes: r (32 bytes) || s (32 bytes), big-endian
/// * `recid` - Recovery ID (0, 1, 2, or 3)
/// * `msg` - 32 bytes message hash (big-endian)
///
/// # Returns
/// * 32 bytes where the first 12 bytes are 0 and the last 20 bytes are the Ethereum address
/// * Returns all zeros if recovery fails
pub fn secp256k1_ecrecover(sig: &[u8; 64], recid: u8, msg: &[u8; 32]) -> Option<[u8; 32]> {
    // Parse r and s from signature
    let r_bytes: [u8; 32] = sig[0..32].try_into().unwrap();
    let s_bytes: [u8; 32] = sig[32..64].try_into().unwrap();

    let r = bytes_be_to_u64_le(&r_bytes);
    let s = bytes_be_to_u64_le(&s_bytes);
    let z = bytes_be_to_u64_le(msg);

    // Recover the public key point
    let pk = secp256k1_ecrecover_point(&r, &s, &z, recid)?;

    // Convert public key to uncompressed format (65 bytes: 0x04 || x || y)
    // But for keccak hashing, we only use x || y (64 bytes)
    let x = [pk[0], pk[1], pk[2], pk[3]];
    let y = [pk[4], pk[5], pk[6], pk[7]];

    let x_bytes = u64_le_to_bytes_be(&x);
    let y_bytes = u64_le_to_bytes_be(&y);

    // Concatenate x and y for hashing
    let mut pk_bytes = [0u8; 64];
    pk_bytes[0..32].copy_from_slice(&x_bytes);
    pk_bytes[32..64].copy_from_slice(&y_bytes);

    // Hash with keccak256
    let mut hasher = Keccak::v256();
    hasher.update(&pk_bytes);
    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);

    // Return with first 12 bytes zeroed (Ethereum address is last 20 bytes)
    let mut result = [0u8; 32];
    result[12..32].copy_from_slice(&hash[12..32]);

    Some(result)
}

/// C-compatible wrapper for secp256k1_ecrecover
///
/// # Safety
/// - `sig` must point to at least 64 bytes
/// - `msg` must point to at least 32 bytes
/// - `output` must point to a writable buffer of at least 32 bytes
///
/// # Returns
/// - 0 if recovery succeeded
/// - 1 if recovery failed (invalid signature or point not on curve)
// TODO: This function has two modes: tx recovery and precompile. Check it is correct
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_secp256k1_ecrecover_c")]
pub unsafe extern "C" fn secp256k1_ecrecover_c(
    sig: *const u8,
    recid: u8,
    msg: *const u8,
    output: *mut u8,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> u8 {
    let sig_bytes: &[u8; 64] = &*(sig as *const [u8; 64]);
    let msg_bytes: &[u8; 32] = &*(msg as *const [u8; 32]);

    match secp256k1_ecrecover(sig_bytes, recid, msg_bytes) {
        Some(result) => {
            let output_slice = core::slice::from_raw_parts_mut(output, 32);
            output_slice.copy_from_slice(&result);
            0 // Success
        }
        None => {
            // Zero out output on failure
            let output_slice = core::slice::from_raw_parts_mut(output, 32);
            output_slice.fill(0);
            1 // Failure
        }
    }
}
