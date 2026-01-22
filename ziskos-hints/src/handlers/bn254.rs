use crate::{
    handlers::{validate_hint_length, validate_hint_min_length},
    hint_fields, zisklib,
};

use anyhow::Result;

/// Processes an `HINT_BN254_G1_ADD` hint.
#[inline]
pub fn bn254_g1_add_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P1: 64, P2: 64];

    validate_hint_min_length(data, EXPECTED_LEN_U64, "HINT_BN254_G1_ADD")?;

    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, EXPECTED_LEN) };

    let p1: &[u8; P1_SIZE] = bytes[P1_OFFSET..P1_OFFSET + P1_SIZE].try_into().unwrap();
    let p2: &[u8; P2_SIZE] = bytes[P2_OFFSET..P2_OFFSET + P2_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    let result: &mut [u8; 64] = &mut [0u8; 64];
    unsafe {
        zisklib::bn254_g1_add_c(p1.as_ptr(), p2.as_ptr(), result.as_mut_ptr(), &mut hints);
    }

    Ok(hints)
}

/// Processes an `HINT_BN254_G1_MUL` hint.
#[inline]
pub fn bn254_g1_mul_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![POINT: 64, SCALAR: 32];

    validate_hint_min_length(data, EXPECTED_LEN_U64, "HINT_BN254_G1_MUL")?;

    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, EXPECTED_LEN) };

    let point: &[u8; POINT_SIZE] =
        bytes[POINT_OFFSET..POINT_OFFSET + POINT_SIZE].try_into().unwrap();
    let scalar: &[u8; SCALAR_SIZE] =
        bytes[SCALAR_OFFSET..SCALAR_OFFSET + SCALAR_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    let result: &mut [u8; 64] = &mut [0u8; 64];
    unsafe {
        zisklib::bn254_g1_mul_c(point.as_ptr(), scalar.as_ptr(), result.as_mut_ptr(), &mut hints);
    }

    Ok(hints)
}

/// Processes an `IS_ON_CURVE_BN254`` hint.
#[inline]
pub fn bn254_is_on_curve_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P: 8];

    validate_hint_length(data, EXPECTED_LEN, "IS_ON_CURVE_BN254")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::is_on_curve_bn254(p, &mut hints);

    Ok(hints)
}

/// Processes a `TO_AFFINE_BN254` hint.
#[inline]
pub fn bn254_to_affine_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P: 12];

    validate_hint_length(data, EXPECTED_LEN, "TO_AFFINE_BN254")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::to_affine_bn254(p, &mut hints);

    Ok(hints)
}

/// Processes an `ADD_BN254` hint.
#[inline]
pub fn bn254_add_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P1: 8, P2: 8];

    validate_hint_length(data, EXPECTED_LEN, "ADD_BN254")?;

    let p1: &[u64; P1_SIZE] = data[P1_OFFSET..P1_OFFSET + P1_SIZE].try_into().unwrap();
    let p2: &[u64; P2_SIZE] = data[P2_OFFSET..P2_OFFSET + P2_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::add_bn254(p1, p2, &mut hints);

    Ok(hints)
}

/// Processes a `MUL_BN254` hint.
#[inline]
pub fn bn254_mul_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P: 8, K: 4];

    validate_hint_length(data, EXPECTED_LEN, "MUL_BN254")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();
    let k: &[u64; K_SIZE] = data[K_OFFSET..K_OFFSET + K_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::mul_bn254(p, k, &mut hints);

    Ok(hints)
}

/// Processes a `TO_AFFINE_TWIST_BN254` hint.
#[inline]
pub fn bn254_to_affine_twist_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P: 24];

    validate_hint_length(data, EXPECTED_LEN, "TO_AFFINE_TWIST_BN254")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::to_affine_twist_bn254(p, &mut hints);

    Ok(hints)
}

/// Processes an `IS_ON_CURVE_TWIST_BN254` hint.
#[inline]
pub fn bn254_is_on_curve_twist_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P: 16];

    validate_hint_length(data, EXPECTED_LEN, "IS_ON_CURVE_TWIST_BN254")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::is_on_curve_twist_bn254(p, &mut hints);

    Ok(hints)
}

/// Processes an `IS_ON_SUBGROUP_TWIST_BN254` hint.
#[inline]
pub fn bn254_is_on_subgroup_twist_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P: 16];

    validate_hint_length(data, EXPECTED_LEN, "IS_ON_SUBGROUP_TWIST_BN254")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::is_on_subgroup_twist_bn254(p, &mut hints);

    Ok(hints)
}

/// Processes a `PAIRING_BATCH_BN254` hint.
/// Format: [num_points:u64][g1_points:&[u64]][g2_points:&[u64]]
/// where g1_points has length num_points * 8 and g2_points has length num_points * 16
#[inline]
pub fn bn254_pairing_batch_hint(data: &[u64]) -> Result<Vec<u64>> {
    if data.is_empty() {
        anyhow::bail!("PAIRING_BATCH_BN254: data is empty");
    }

    let num_points = data[0] as usize;

    const G1_POINT_SIZE: usize = 8;
    const G2_POINT_SIZE: usize = 16;

    let expected_len = 1 + num_points * G1_POINT_SIZE + num_points * G2_POINT_SIZE;

    validate_hint_length(data, expected_len, "PAIRING_BATCH_BN254")?;

    let g1_start = 1;
    let g1_end = g1_start + num_points * G1_POINT_SIZE;
    let g2_start = g1_end;
    let g2_end = g2_start + num_points * G2_POINT_SIZE;

    let g1_points_slice = &data[g1_start..g1_end];
    let g2_points_slice = &data[g2_start..g2_end];

    // SAFETY: We've validated the length, and the memory layout of &[u64] with length num_points * 8
    // is identical to &[[u64; 8]] with length num_points
    let g1_points: &[[u64; G1_POINT_SIZE]] = unsafe {
        std::slice::from_raw_parts(
            g1_points_slice.as_ptr() as *const [u64; G1_POINT_SIZE],
            num_points,
        )
    };

    // SAFETY: We've validated the length, and the memory layout of &[u64] with length num_points * 16
    // is identical to &[[u64; 16]] with length num_points
    let g2_points: &[[u64; G2_POINT_SIZE]] = unsafe {
        std::slice::from_raw_parts(
            g2_points_slice.as_ptr() as *const [u64; G2_POINT_SIZE],
            num_points,
        )
    };

    let mut hints = Vec::new();
    zisklib::pairing_batch_bn254(g1_points, g2_points, &mut hints);

    Ok(hints)
}
