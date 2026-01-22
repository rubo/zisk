use crate::zisklib::lib::utils::{is_one, lt};

use super::{
    constants::{
        G1_GENERATOR, G1_IDENTITY, G2_GENERATOR, G2_IDENTITY, R, TRUSTED_SETUP_TAU_G2_COMPRESSED,
    },
    curve::{decompress_bls12_381, scalar_mul_bls12_381, sub_bls12_381},
    pairing::pairing_batch_bls12_381,
    twist::{
        decompress_twist_bls12_381, neg_twist_bls12_381, scalar_mul_twist_bls12_381,
        sub_twist_bls12_381,
    },
};

/// Verify KZG proof using BLS12-381 implementation.
///
/// # Arguments
/// * `z` - 32 bytes big-endian scalar (evaluation point)
/// * `y` - 32 bytes big-endian scalar (claimed evaluation)
/// * `commitment` - 48 bytes compressed G1 point (polynomial commitment)
/// * `proof` - 48 bytes compressed G1 point (KZG proof)
///
/// # Safety
/// All pointers must be valid and properly aligned.
///
/// # Returns
/// * 1 if the proof is valid
/// * 0 if the proof is invalid
/// * 2 if there was a parsing error (invalid input)
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_verify_kzg_proof_c")]
pub unsafe extern "C" fn verify_kzg_proof_c(
    z: *const u8,
    y: *const u8,
    commitment: *const u8,
    proof: *const u8,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> u8 {
    let z_bytes: &[u8; 32] = &*(z as *const [u8; 32]);
    let y_bytes: &[u8; 32] = &*(y as *const [u8; 32]);
    let commitment_bytes: &[u8; 48] = &*(commitment as *const [u8; 48]);
    let proof_bytes: &[u8; 48] = &*(proof as *const [u8; 48]);

    // Parse the commitment (G1 point, compressed)
    let commitment_point = match decompress_bls12_381(commitment_bytes) {
        Ok((point, _is_inf)) => point,
        Err(_) => return 2, // Invalid commitment
    };

    // Parse the proof (G1 point, compressed)
    let proof_point = match decompress_bls12_381(proof_bytes) {
        Ok((point, _is_inf)) => point,
        Err(_) => return 2, // Invalid proof
    };

    // Parse z and y as scalar field elements (must be canonical < R)
    let z_scalar = match read_scalar_canonical(z_bytes) {
        Some(s) => s,
        None => return 2, // z not canonical
    };

    let y_scalar = match read_scalar_canonical(y_bytes) {
        Some(s) => s,
        None => return 2, // y not canonical
    };

    // Get the trusted setup G2 point [τ]₂
    let tau_g2 = get_trusted_setup_g2();

    // Get generators
    let g1 = G1_GENERATOR;
    let g2 = G2_GENERATOR;

    // Compute c_minus_y = commitment - [y]G₁
    let y_g1 = scalar_mul_bls12_381(&g1, &y_scalar);
    let c_minus_y = sub_bls12_381(&commitment_point, &y_g1);

    // Compute t_minus_z = [τ]₂ - [z]G₂
    let z_g2 = scalar_mul_twist_bls12_381(&g2, &z_scalar);
    let t_minus_z = sub_twist_bls12_381(&tau_g2, &z_g2);

    // The verification equation is:
    // e(commitment - [y]G₁, G₂) = e(proof, [τ]₂ - [z]G₂)
    //
    // Which is equivalent to checking:
    // e(commitment - [y]G₁, -G₂) · e(proof, [τ]₂ - [z]G₂) = 1
    let neg_g2 = neg_twist_bls12_381(&g2);

    // Batch pairing check
    let g1_points = [c_minus_y, proof_point];
    let g2_points = [neg_g2, t_minus_z];

    // Check if the pairing result equals 1
    if is_one(&pairing_batch_bls12_381(&g1_points, &g2_points)) {
        1 // Valid proof
    } else {
        0 // Invalid proof
    }
}

/// Read a scalar from 32 big-endian bytes and check if it's canonical (< R)
/// Returns None if the scalar is not canonical
fn read_scalar_canonical(bytes: &[u8; 32]) -> Option<[u64; 4]> {
    // Convert big-endian bytes to little-endian u64 limbs
    let mut scalar = [0u64; 4];
    for i in 0..4 {
        for j in 0..8 {
            scalar[3 - i] |= (bytes[i * 8 + j] as u64) << (8 * (7 - j));
        }
    }

    // Check if scalar < R (scalar field order)
    if !lt(&scalar, &R) {
        return None; // scalar >= R, not canonical
    }

    Some(scalar)
}

/// Get the trusted setup G2 point `[τ]₂`
fn get_trusted_setup_g2() -> [u64; 24] {
    decompress_twist_bls12_381(&TRUSTED_SETUP_TAU_G2_COMPRESSED)
        .expect("Failed to decompress trusted setup G2")
        .0
}
