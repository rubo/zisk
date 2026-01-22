//! Pairing over BLS12-381 curve

use crate::zisklib::lib::utils::{gt, is_one};

use super::{
    constants::{G1_IDENTITY, G2_IDENTITY, P_MINUS_ONE},
    curve::{
        g1_bytes_be_to_u64_le, is_on_curve_bls12_381, is_on_subgroup_bls12_381, neg_bls12_381,
    },
    final_exp::final_exp_bls12_381,
    miller_loop::{miller_loop_batch_bls12_381, miller_loop_bls12_381},
    twist::{g2_bytes_be_to_u64_le, is_on_curve_twist_bls12_381, is_on_subgroup_twist_bls12_381},
};

/// Optimal Ate Pairing e: G1 x G2 -> GT over the BLS12-381 curve
/// where G1 = E(Fp)[r] = E(Fp), G2 = E'(Fp2)[r] and GT = μ_r (the r-th roots of unity over Fp12*)
/// the involved curves are E/Fp: y² = x³ + 4 and E'/Fp2: y² = x³ + 4·(1+u)
///  pairingBLS12-381:
///          input: P ∈ G1 and Q ∈ G2
///          output: e(P,Q) ∈ GT
pub fn pairing_bls12_381(
    p: &[u64; 12],
    q: &[u64; 24],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 72] {
    // e(P, 𝒪) = e(𝒪, Q) = 1;
    if *p == G1_IDENTITY || *q == G2_IDENTITY {
        let mut one = [0; 72];
        one[0] = 1;
        return one;
    }

    // Miller loop
    let miller_loop = miller_loop_bls12_381(
        p,
        q,
        #[cfg(feature = "hints")]
        hints,
    );

    // Final exponentiation
    final_exp_bls12_381(
        &miller_loop,
        #[cfg(feature = "hints")]
        hints,
    )
}

/// Computes the optimal Ate pairing for a batch of G1 and G2 points over the BN254 curve
/// and multiplies the results together, i.e.:
///     e(P₁, Q₁) · e(P₂, Q₂) · ... · e(Pₙ, Qₙ) ∈ GT
pub fn pairing_batch_bls12_381(
    g1_points: &[[u64; 12]],
    g2_points: &[[u64; 24]],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 72] {
    // Since each e(Pi, Qi) := FinalExp(MillerLoop(Pi, Qi))
    // We have:
    //  e(P₁, Q₁) · e(P₂, Q₂) · ... · e(Pₙ, Qₙ) = FinalExp(MillerLoop(P₁, Q₁) · MillerLoop(P₂, Q₂) · ... · MillerLoop(Pₙ, Qₙ))
    // We can compute the Miller loop for each pair, multiplying the results together
    // and then just do the final exponentiation once at the end.

    let n = g1_points.len();
    assert_eq!(n, g2_points.len(), "Number of G1 and G2 points must be equal");

    // Miller loop and multiplication
    let mut g1_points_ml = Vec::with_capacity(n);
    let mut g2_points_ml = Vec::with_capacity(n);
    for (p, q) in g1_points.iter().zip(g2_points.iter()) {
        // If p = 𝒪 or q = 𝒪 => MillerLoop(P, 𝒪) = MillerLoop(𝒪, Q) = 1; we can skip
        if *p != G1_IDENTITY && *q != G2_IDENTITY {
            g1_points_ml.push(*p);
            g2_points_ml.push(*q);
        }
    }

    if g1_points_ml.is_empty() {
        // If all pairing computations were skipped, return 1
        let mut one = [0; 72];
        one[0] = 1;
        return one;
    }

    // Miller loop
    let miller_loop = miller_loop_batch_bls12_381(
        &g1_points_ml,
        &g2_points_ml,
        #[cfg(feature = "hints")]
        hints,
    );

    // Final exponentiation
    final_exp_bls12_381(
        &miller_loop,
        #[cfg(feature = "hints")]
        hints,
    )
}

/// BLS12-381 pairing check for big-endian byte format
///
/// This function is designed to patch:
/// `fn bls12_381_pairing_check(&self, pairs: &[(G1Point, G2Point)]) -> Result<bool, PrecompileError>`
///
/// Input format per pair: 288 bytes = 96 bytes G1 point + 192 bytes G2 point (big-endian)
/// - G1 point: 48 bytes x + 48 bytes y
/// - G2 point: 48 bytes x_i + 48 bytes x_r + 48 bytes y_i + 48 bytes y_r
///
/// ### Safety
/// - `pairs` must point to an array of `num_pairs * 288` bytes
///
/// Returns:
/// - true if the pairing check passes (the product of pairings is equal to 1 in GT)
/// - false otherwise
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_bls12_381_pairing_check_c")]
pub unsafe extern "C" fn bls12_381_pairing_check_c(
    pairs: *const u8,
    num_pairs: usize,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> bool {
    // Handle empty input - empty product is 1, so pairing check passes
    if num_pairs == 0 {
        return true;
    }

    let mut g1_points: Vec<[u64; 12]> = Vec::with_capacity(num_pairs);
    let mut g2_points: Vec<[u64; 24]> = Vec::with_capacity(num_pairs);

    for i in 0..num_pairs {
        let pair_ptr = pairs.add(i * 288);

        // Extract G1 point (96 bytes) and G2 point (192 bytes)
        let g1_bytes: &[u8; 96] = &*(pair_ptr as *const [u8; 96]);
        let g2_bytes: &[u8; 192] = &*(pair_ptr.add(96) as *const [u8; 192]);

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

        // Verify G1 point is on curve
        if !is_on_curve_bls12_381(
            &g1_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return false;
        }

        // Verify G1 point is in subgroup
        if !is_on_subgroup_bls12_381(
            &g1_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return false;
        }

        // Convert G2 from big-endian bytes to u64 limbs
        let g2_u64 = g2_bytes_be_to_u64_le(g2_bytes);

        // Verify G2 point is on twist curve
        if !is_on_curve_twist_bls12_381(
            &g2_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return false;
        }

        // Verify G2 point is in subgroup
        if !is_on_subgroup_twist_bls12_381(
            &g2_u64,
            #[cfg(feature = "hints")]
            hints,
        ) {
            return false;
        }

        g1_points.push(g1_u64);
        g2_points.push(g2_u64);
    }

    // If all pairs were skipped (all infinities), result is 1
    if g1_points.is_empty() {
        return true;
    }

    // Compute batch pairing and check if result is 1
    is_one(&pairing_batch_bls12_381(
        &g1_points,
        &g2_points,
        #[cfg(feature = "hints")]
        hints,
    ))
}
