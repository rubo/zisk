//! Operations on the BN254 curve E: y² = x³ + 3

use crate::{
    syscalls::{
        syscall_bn254_curve_add, syscall_bn254_curve_dbl, SyscallBn254CurveAddParams,
        SyscallPoint256,
    },
    zisklib::{eq, fcall_msb_pos_256},
};

use super::{
    constants::{E_B, IDENTITY_G1, P},
    fp::{add_fp_bn254, inv_fp_bn254, mul_fp_bn254, square_fp_bn254},
};

/// Check if a non-zero point `p` is on the BN254 curve
pub fn is_on_curve_bn254(p: &[u64; 8], #[cfg(feature = "hints")] hints: &mut Vec<u64>) -> bool {
    let x: [u64; 4] = p[0..4].try_into().unwrap();
    let y: [u64; 4] = p[4..8].try_into().unwrap();

    // p in E iff y² == x³ + 3
    let lhs = square_fp_bn254(
        &y,
        #[cfg(feature = "hints")]
        hints,
    );
    let mut rhs = square_fp_bn254(
        &x,
        #[cfg(feature = "hints")]
        hints,
    );
    rhs = mul_fp_bn254(
        &rhs,
        &x,
        #[cfg(feature = "hints")]
        hints,
    );
    rhs = add_fp_bn254(
        &rhs,
        &E_B,
        #[cfg(feature = "hints")]
        hints,
    );
    eq(&lhs, &rhs)
}

/// Converts a point `p` on the BN254 curve from Jacobian coordinates to affine coordinates
pub fn to_affine_bn254(p: &[u64; 12], #[cfg(feature = "hints")] hints: &mut Vec<u64>) -> [u64; 8] {
    let z: [u64; 4] = p[8..12].try_into().unwrap();

    if z == [0u64; 4] {
        return IDENTITY_G1;
    } else if z == [1u64, 0, 0, 0] {
        return [p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]];
    }

    let x: [u64; 4] = p[0..4].try_into().unwrap();
    let y: [u64; 4] = p[4..8].try_into().unwrap();

    let zinv = inv_fp_bn254(
        &z,
        #[cfg(feature = "hints")]
        hints,
    );
    let zinv_sq = square_fp_bn254(
        &zinv,
        #[cfg(feature = "hints")]
        hints,
    );

    let x_res = mul_fp_bn254(
        &x,
        &zinv_sq,
        #[cfg(feature = "hints")]
        hints,
    );
    let mut y_res = mul_fp_bn254(
        &y,
        &zinv_sq,
        #[cfg(feature = "hints")]
        hints,
    );
    y_res = mul_fp_bn254(
        &y_res,
        &zinv,
        #[cfg(feature = "hints")]
        hints,
    );
    [x_res[0], x_res[1], x_res[2], x_res[3], y_res[0], y_res[1], y_res[2], y_res[3]]
}

/// Adds two points `p1` and `p2` on the BN254 curve
pub fn add_bn254(
    p1: &[u64; 8],
    p2: &[u64; 8],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 8] {
    if *p1 == IDENTITY_G1 {
        return *p2;
    } else if *p2 == IDENTITY_G1 {
        return *p1;
    }

    let x1: [u64; 4] = p1[0..4].try_into().unwrap();
    let y1: [u64; 4] = p1[4..8].try_into().unwrap();
    let x2: [u64; 4] = p2[0..4].try_into().unwrap();
    let y2: [u64; 4] = p2[4..8].try_into().unwrap();

    // Is x1 == x2?
    if eq(&x1, &x2) {
        // Is y1 == y2?
        if eq(&y1, &y2) {
            // Compute the doubling
            return dbl_bn254(
                p1,
                #[cfg(feature = "hints")]
                hints,
            );
        } else {
            // Return 𝒪
            return IDENTITY_G1;
        }
    }

    // As p1 != p2,-p2, compute the addition

    // Convert the input points to SyscallPoint256
    let mut p1 = SyscallPoint256 { x: x1, y: y1 };
    let p2 = SyscallPoint256 { x: x2, y: y2 };

    // Call the syscall to add the two points
    let mut params = SyscallBn254CurveAddParams { p1: &mut p1, p2: &p2 };
    syscall_bn254_curve_add(
        &mut params,
        #[cfg(feature = "hints")]
        hints,
    );

    // Convert the result back to a single array
    let x3 = params.p1.x;
    let y3 = params.p1.y;
    [x3[0], x3[1], x3[2], x3[3], y3[0], y3[1], y3[2], y3[3]]
}

pub fn dbl_bn254(p: &[u64; 8], #[cfg(feature = "hints")] hints: &mut Vec<u64>) -> [u64; 8] {
    let mut p1 = SyscallPoint256 { x: p[0..4].try_into().unwrap(), y: p[4..8].try_into().unwrap() };
    syscall_bn254_curve_dbl(
        &mut p1,
        #[cfg(feature = "hints")]
        hints,
    );
    [p1.x[0], p1.x[1], p1.x[2], p1.x[3], p1.y[0], p1.y[1], p1.y[2], p1.y[3]]
}

/// Multiplies a point `p` on the BN254 curve by a scalar `k` on the BN254 scalar field
pub fn mul_bn254(
    p: &[u64; 8],
    k: &[u64; 4],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 8] {
    if *p == IDENTITY_G1 {
        return IDENTITY_G1;
    }

    // Direct cases: k = 0, k = 1, k = 2
    match k {
        [0, 0, 0, 0] => {
            // Return 𝒪
            return IDENTITY_G1;
        }
        [1, 0, 0, 0] => {
            // Return p
            return *p;
        }
        [2, 0, 0, 0] => {
            // Return 2p
            return dbl_bn254(
                p,
                #[cfg(feature = "hints")]
                hints,
            );
        }
        _ => {}
    }

    // We can assume k > 2 from now on
    // Hint the length the binary representations of k
    // We will verify the output by recomposing k
    // Moreover, we should check that the first received bit is 1
    let (max_limb, max_bit) = fcall_msb_pos_256(
        k,
        &[0, 0, 0, 0],
        #[cfg(feature = "hints")]
        hints,
    );

    // Perform the loop, based on the binary representation of k

    // We do the first iteration separately
    let max_limb = max_limb as usize;
    let max_bit = max_bit as usize;

    // The first received bit should be 1
    assert_eq!((k[max_limb] >> max_bit) & 1, 1);

    // Start at P
    let x1: [u64; 4] = p[0..4].try_into().unwrap();
    let y1: [u64; 4] = p[4..8].try_into().unwrap();
    let mut q = SyscallPoint256 { x: x1, y: y1 };
    let mut k_rec = [0u64; 4];
    k_rec[max_limb] |= 1 << max_bit;

    // Determine starting limb/bit for the loop
    let mut limb = max_limb;
    let mut bit = if max_bit == 0 {
        // If max_bit is 0 then limb > 0; otherwise k = 1, which is excluded here
        limb -= 1;
        63
    } else {
        max_bit - 1
    };

    // Perform the rest of the loop
    let p = SyscallPoint256 { x: x1, y: y1 };
    for i in (0..=limb).rev() {
        for j in (0..=bit).rev() {
            // Always double
            syscall_bn254_curve_dbl(
                &mut q,
                #[cfg(feature = "hints")]
                hints,
            );

            // Get the next bit b of k.
            // If b == 1, we should add P to Q, otherwise start the next iteration
            if ((k[i] >> j) & 1) == 1 {
                let mut params = SyscallBn254CurveAddParams { p1: &mut q, p2: &p };
                syscall_bn254_curve_add(
                    &mut params,
                    #[cfg(feature = "hints")]
                    hints,
                );

                // Reconstruct k
                k_rec[i] |= 1 << j;
            }
        }
        bit = 63;
    }

    // Check that the reconstructed k is equal to the input k
    assert_eq!(k_rec, *k);

    // Convert the result back to a single array
    let x3 = q.x;
    let y3 = q.y;
    [x3[0], x3[1], x3[2], x3[3], y3[0], y3[1], y3[2], y3[3]]
}

/// Convert big-endian bytes to little-endian u64 limbs for a G1 point (64 bytes -> [u64; 8])
/// Format: 32 bytes x (big-endian) + 32 bytes y (big-endian)
fn g1_bytes_be_to_u64_le(bytes: &[u8; 64]) -> [u64; 8] {
    let mut result = [0u64; 8];

    // Parse x coordinate (first 32 bytes, big-endian)
    for i in 0..4 {
        for j in 0..8 {
            result[3 - i] |= (bytes[i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    // Parse y coordinate (next 32 bytes, big-endian)
    for i in 0..4 {
        for j in 0..8 {
            result[7 - i] |= (bytes[32 + i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    result
}

/// Convert little-endian u64 limbs to big-endian bytes for a G1 point ([u64; 8] -> 64 bytes)
fn g1_u64_le_to_bytes_be(point: &[u64; 8]) -> [u8; 64] {
    let mut result = [0u8; 64];

    // Encode x coordinate (first 32 bytes, big-endian)
    for i in 0..4 {
        for j in 0..8 {
            result[i * 8 + j] = ((point[3 - i] >> (8 * (7 - j))) & 0xff) as u8;
        }
    }

    // Encode y coordinate (next 32 bytes, big-endian)
    for i in 0..4 {
        for j in 0..8 {
            result[32 + i * 8 + j] = ((point[7 - i] >> (8 * (7 - j))) & 0xff) as u8;
        }
    }

    result
}

/// Convert big-endian bytes to little-endian u64 limbs for a scalar (32 bytes -> [u64; 4])
fn scalar_bytes_be_to_u64_le(bytes: &[u8; 32]) -> [u64; 4] {
    let mut result = [0u64; 4];

    for i in 0..4 {
        for j in 0..8 {
            result[3 - i] |= (bytes[i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    result
}

/// Check if a field element is valid (< P)
fn is_valid_field_element(x: &[u64; 4]) -> bool {
    // Compare from most significant limb
    for i in (0..4).rev() {
        if x[i] > P[i] {
            return false;
        }
        if x[i] < P[i] {
            return true;
        }
    }
    // x == P, which is not valid
    false
}

/// BN254 G1 point addition with big-endian byte format
///
/// This function is designed to patch:
/// `fn bn254_g1_add(&self, p1: &[u8], p2: &[u8]) -> Result<[u8; 64], PrecompileError>`
///
/// Input format: 64 bytes per point = 32 bytes x + 32 bytes y (big-endian)
/// Output format: 64 bytes = 32 bytes x + 32 bytes y (big-endian)
///
/// # Safety
/// - `p1` must point to at least 64 bytes
/// - `p2` must point to at least 64 bytes
/// - `result` must point to a writable buffer of at least 64 bytes
///
/// # Returns
/// - 0 if the operation succeeded
/// - 1 if p1 is invalid (not on curve or invalid field element)
/// - 2 if p2 is invalid (not on curve or invalid field element)
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_bn254_g1_add_c")]
pub unsafe extern "C" fn bn254_g1_add_c(
    p1: *const u8,
    p2: *const u8,
    result: *mut u8,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> u8 {
    let p1_bytes: &[u8; 64] = &*(p1 as *const [u8; 64]);
    let p2_bytes: &[u8; 64] = &*(p2 as *const [u8; 64]);

    // Check if p1 is infinity (all zeros)
    let p1_is_inf = p1_bytes.iter().all(|&x| x == 0);

    // Check if p2 is infinity (all zeros)
    let p2_is_inf = p2_bytes.iter().all(|&x| x == 0);

    // Convert to internal format
    let p1_u64 = g1_bytes_be_to_u64_le(p1_bytes);
    let p2_u64 = g1_bytes_be_to_u64_le(p2_bytes);

    // Validate field elements and curve membership for non-infinity points
    if !p1_is_inf {
        let x1: [u64; 4] = p1_u64[0..4].try_into().unwrap();
        let y1: [u64; 4] = p1_u64[4..8].try_into().unwrap();

        if !is_valid_field_element(&x1) || !is_valid_field_element(&y1) {
            return 1; // Invalid field element
        }

        if !is_on_curve_bn254(
            &p1_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return 1; // Not on curve
        }
    }

    if !p2_is_inf {
        let x2: [u64; 4] = p2_u64[0..4].try_into().unwrap();
        let y2: [u64; 4] = p2_u64[4..8].try_into().unwrap();

        if !is_valid_field_element(&x2) || !is_valid_field_element(&y2) {
            return 2; // Invalid field element
        }

        if !is_on_curve_bn254(
            &p2_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return 2; // Not on curve
        }
    }

    // Handle infinity cases
    let sum = if p1_is_inf && p2_is_inf {
        IDENTITY_G1
    } else if p1_is_inf {
        p2_u64
    } else if p2_is_inf {
        p1_u64
    } else {
        add_bn254(
            &p1_u64,
            &p2_u64,
            #[cfg(feature = "hints")]
            hints,
        )
    };

    // Convert result to big-endian bytes
    let result_bytes = if sum == IDENTITY_G1 { [0u8; 64] } else { g1_u64_le_to_bytes_be(&sum) };

    // Write result
    let result_slice = core::slice::from_raw_parts_mut(result, 64);
    result_slice.copy_from_slice(&result_bytes);

    0 // Success
}

/// BN254 G1 scalar multiplication with big-endian byte format
///
/// This function is designed to patch:
/// `fn bn254_g1_mul(&self, point: &[u8], scalar: &[u8]) -> Result<[u8; 64], PrecompileError>`
///
/// Input format:
/// - point: 64 bytes = 32 bytes x + 32 bytes y (big-endian)
/// - scalar: 32 bytes (big-endian), does NOT need to be canonical
///
/// Output format: 64 bytes = 32 bytes x + 32 bytes y (big-endian)
///
/// # Safety
/// - `point` must point to at least 64 bytes
/// - `scalar` must point to at least 32 bytes
/// - `result` must point to a writable buffer of at least 64 bytes
///
/// # Returns
/// - 0 if the operation succeeded
/// - 1 if point is invalid (not on curve or invalid field element)
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_bn254_g1_mul_c")]
pub unsafe extern "C" fn bn254_g1_mul_c(
    point: *const u8,
    scalar: *const u8,
    result: *mut u8,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> u8 {
    let point_bytes: &[u8; 64] = &*(point as *const [u8; 64]);
    let scalar_bytes: &[u8; 32] = &*(scalar as *const [u8; 32]);

    // Check if point is infinity (all zeros)
    let point_is_inf = point_bytes.iter().all(|&x| x == 0);

    // Convert point to internal format
    let point_u64 = g1_bytes_be_to_u64_le(point_bytes);

    // Validate field elements and curve membership for non-infinity point
    if !point_is_inf {
        let x: [u64; 4] = point_u64[0..4].try_into().unwrap();
        let y: [u64; 4] = point_u64[4..8].try_into().unwrap();

        if !is_valid_field_element(&x) || !is_valid_field_element(&y) {
            return 1; // Invalid field element
        }

        if !is_on_curve_bn254(
            &point_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return 1; // Not on curve
        }
    }

    // Convert scalar to internal format
    let scalar_u64 = scalar_bytes_be_to_u64_le(scalar_bytes);

    // Perform scalar multiplication
    let product = if point_is_inf || scalar_u64 == [0, 0, 0, 0] {
        IDENTITY_G1
    } else {
        mul_bn254(
            &point_u64,
            &scalar_u64,
            #[cfg(feature = "hints")]
            hints,
        )
    };

    // Convert result to big-endian bytes
    let result_bytes =
        if product == IDENTITY_G1 { [0u8; 64] } else { g1_u64_le_to_bytes_be(&product) };

    // Write result
    let result_slice = core::slice::from_raw_parts_mut(result, 64);
    result_slice.copy_from_slice(&result_bytes);

    0 // Success
}
