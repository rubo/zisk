//! Pairing over BN254

use crate::zisklib::lib::utils::{gt, is_one};

use super::{
    constants::{IDENTITY_G1, IDENTITY_G2, P, P_MINUS_ONE},
    curve::is_on_curve_bn254,
    final_exp::final_exp_bn254,
    miller_loop::{miller_loop_batch_bn254, miller_loop_bn254},
    twist::{is_on_curve_twist_bn254, is_on_subgroup_twist_bn254},
};

/// Optimal Ate Pairing e: G1 x G2 -> GT over the BN254 curve
/// where G1 = E(Fp)[r] = E(Fp), G2 = E'(Fp2)[r] and GT = μ_r (the r-th roots of unity over Fp12*
/// the involved curves are E/Fp: y² = x³ + 3 and E'/Fp2: y² = x³ + 3/(9+u)
///  pairingBN254:
///          input: P ∈ G1 and Q ∈ G2
///          output: e(P,Q) ∈ GT
///
pub fn pairing_bn254(
    p: &[u64; 8],
    q: &[u64; 16],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 48] {
    // Is p = 𝒪?
    if *p == IDENTITY_G1 || *q == IDENTITY_G2 {
        // e(P, 𝒪) = e(𝒪, Q) = 1;
        let mut one = [0; 48];
        one[0] = 1;
        return one;
    }

    // Miller loop
    let miller_loop = miller_loop_bn254(
        p,
        q,
        #[cfg(feature = "hints")]
        hints,
    );

    // Final exponentiation
    final_exp_bn254(
        &miller_loop,
        #[cfg(feature = "hints")]
        hints,
    )
}

/// Computes the optimal Ate pairing for a batch of G1 and G2 points over the BN254 curve
/// and multiplies the results together, i.e.:
///     e(P₁, Q₁) · e(P₂, Q₂) · ... · e(Pₙ, Qₙ) ∈ GT
pub fn pairing_batch_bn254(
    g1_points: &[[u64; 8]],
    g2_points: &[[u64; 16]],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 48] {
    // Since each e(Pi, Qi) := FinalExp(MillerLoop(Pi, Qi))
    // We have:
    //  e(P₁, Q₁) · e(P₂, Q₂) · ... · e(Pₙ, Qₙ) = FinalExp(MillerLoop(P₁, Q₁) · MillerLoop(P₂, Q₂) · ... · MillerLoop(Pₙ, Qₙ))
    // We can compute the Miller loop for each pair, multiplying the results together
    // and then just do the final exponentiation once at the end.

    let num_points = g1_points.len();
    assert_eq!(num_points, g2_points.len(), "Number of G1 and G2 points must be equal");

    // Miller loop and multiplication
    let mut g1_points_ml = Vec::with_capacity(num_points);
    let mut g2_points_ml = Vec::with_capacity(num_points);
    for (p, q) in g1_points.iter().zip(g2_points.iter()) {
        // Is p = 𝒪 or q = 𝒪?
        if *p == IDENTITY_G1 || *q == IDENTITY_G2 {
            // MillerLoop(P, 𝒪) = MillerLoop(𝒪, Q) = 1; we can skip
            continue;
        }

        g1_points_ml.push(*p);
        g2_points_ml.push(*q);
    }

    if g1_points_ml.is_empty() {
        // If all pairing computations were skipped, return 1
        let mut one = [0; 48];
        one[0] = 1;
        return one;
    }

    // Compute the Miller loop for the batch
    let miller_loop = miller_loop_batch_bn254(
        &g1_points_ml,
        &g2_points_ml,
        #[cfg(feature = "hints")]
        hints,
    );

    // Final exponentiation
    final_exp_bn254(
        &miller_loop,
        #[cfg(feature = "hints")]
        hints,
    )
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

/// Convert big-endian bytes to little-endian u64 limbs for a G2 point (128 bytes -> [u64; 16])
/// Format: 64 bytes x (32 bytes x_i + 32 bytes x_r) + 64 bytes y (32 bytes y_i + 32 bytes y_r)
/// Note: In BN254, Fq2 elements are encoded as (imaginary, real), i.e., y + x*u
fn g2_bytes_be_to_u64_le(bytes: &[u8; 128]) -> [u64; 16] {
    let mut result = [0u64; 16];

    // Parse x coordinate (Fq2: first 32 bytes = x_i, next 32 bytes = x_r)
    // Internal format: x_r at [0..4], x_i at [4..8]

    // x_i (imaginary part, first 32 bytes)
    for i in 0..4 {
        for j in 0..8 {
            result[7 - i] |= (bytes[i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    // x_r (real part, next 32 bytes)
    for i in 0..4 {
        for j in 0..8 {
            result[3 - i] |= (bytes[32 + i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    // Parse y coordinate (Fq2: next 32 bytes = y_i, final 32 bytes = y_r)
    // Internal format: y_r at [8..12], y_i at [12..16]

    // y_i (imaginary part)
    for i in 0..4 {
        for j in 0..8 {
            result[15 - i] |= (bytes[64 + i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    // y_r (real part)
    for i in 0..4 {
        for j in 0..8 {
            result[11 - i] |= (bytes[96 + i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    result
}

/// BN254 pairing check with big-endian byte format
///
/// This function is designed to patch:
/// `fn bn254_pairing_check(&self, pairs: &[(&[u8], &[u8])]) -> Result<bool, PrecompileError>`
///
/// Input format per pair:
/// - G1 point: 64 bytes = 32 bytes x + 32 bytes y (big-endian)
/// - G2 point: 128 bytes = 64 bytes x (32 x_i + 32 x_r) + 64 bytes y (32 y_i + 32 y_r) (big-endian)
///
/// # Safety
/// - `pairs` must point to an array of `num_pairs` pair structures
/// - Each pair is: 8 bytes g1_ptr + 8 bytes g2_ptr (pointers to the actual data)
///
/// # Returns
/// - 1 if the pairing check passes (the product of pairings equals 1 in GT)
/// - 0 if the pairing check fails
/// - 2 if there was a parsing error (invalid input)
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_bn254_pairing_check_c")]
pub unsafe extern "C" fn bn254_pairing_check_c(
    g1_ptrs: *const *const u8,
    g2_ptrs: *const *const u8,
    num_pairs: usize,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> u8 {
    // Handle empty input - empty product is 1, so pairing check passes
    if num_pairs == 0 {
        return 1;
    }

    let mut g1_points: Vec<[u64; 8]> = Vec::with_capacity(num_pairs);
    let mut g2_points: Vec<[u64; 16]> = Vec::with_capacity(num_pairs);

    for i in 0..num_pairs {
        let g1_ptr = *g1_ptrs.add(i);
        let g2_ptr = *g2_ptrs.add(i);

        let g1_bytes: &[u8; 64] = &*(g1_ptr as *const [u8; 64]);
        let g2_bytes: &[u8; 128] = &*(g2_ptr as *const [u8; 128]);

        // Check if G1 point is infinity
        let g1_is_inf = g1_bytes.iter().all(|&x| x == 0);

        // Check if G2 point is infinity
        let g2_is_inf = g2_bytes.iter().all(|&x| x == 0);

        // If either point is infinity, skip this pair (contributes 1 to product)
        if g1_is_inf || g2_is_inf {
            continue;
        }

        // Convert G1 from big-endian bytes to u64 limbs
        let g1_u64 = g1_bytes_be_to_u64_le(g1_bytes);

        // Validate G1 field elements
        let x1: [u64; 4] = g1_u64[0..4].try_into().unwrap();
        let y1: [u64; 4] = g1_u64[4..8].try_into().unwrap();
        if !is_valid_field_element(&x1) || !is_valid_field_element(&y1) {
            return 2; // Invalid field element
        }

        // Verify G1 point is on curve
        if !is_on_curve_bn254(
            &g1_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return 2;
        }

        // Convert G2 from big-endian bytes to u64 limbs
        let g2_u64 = g2_bytes_be_to_u64_le(g2_bytes);

        // Validate G2 field elements (4 field elements for Fq2)
        let x2_r: [u64; 4] = g2_u64[0..4].try_into().unwrap();
        let x2_i: [u64; 4] = g2_u64[4..8].try_into().unwrap();
        let y2_r: [u64; 4] = g2_u64[8..12].try_into().unwrap();
        let y2_i: [u64; 4] = g2_u64[12..16].try_into().unwrap();
        if !is_valid_field_element(&x2_r)
            || !is_valid_field_element(&x2_i)
            || !is_valid_field_element(&y2_r)
            || !is_valid_field_element(&y2_i)
        {
            return 2; // Invalid field element
        }

        // Verify G2 point is on twist curve
        if !is_on_curve_twist_bn254(
            &g2_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return 2;
        }

        // Verify G2 point is in subgroup
        if !is_on_subgroup_twist_bn254(
            &g2_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return 2;
        }

        g1_points.push(g1_u64);
        g2_points.push(g2_u64);
    }

    // If all pairs were skipped (all infinities), result is 1
    if g1_points.is_empty() {
        return 1;
    }

    // Compute batch pairing and check if result is 1
    if is_one(&pairing_batch_bn254(
        &g1_points,
        &g2_points,
        #[cfg(feature = "hints")]
        hints,
    )) {
        1 // Pairing check passed
    } else {
        0 // Pairing check failed
    }
}
