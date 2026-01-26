use crate::{handlers::validate_hint_length, hint_fields, zisklib};

use anyhow::Result;

/// Processes an `HINT_BN254_G1_ADD` hint.
#[inline]
pub fn bn254_g1_add_hint(data: &[u64]) -> Result<Vec<u64>> {
    hint_fields![P1: 64, P2: 64];

    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 8) };

    validate_hint_length(bytes, EXPECTED_LEN, "HINT_BN254_G1_ADD")?;

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

    let bytes = unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 8) };

    validate_hint_length(bytes, EXPECTED_LEN, "HINT_BN254_G1_MUL")?;

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

/// Processes an `HINT_BN254_PAIRING_CHECK` hint.
#[inline]
pub fn bn254_pairing_check_hint(data: &[u64]) -> Result<Vec<u64>> {
    // ---------------------------------------------------------------------
    // Input format (INTERLEAVED):
    //
    // data[0]                = num_pairs
    // data[1 ..]             = Interleaved pairs: [G1[0], G2[0], G1[1], G2[1], ...]
    //
    // Each G1 point:  8 u64  = 64 bytes
    // Each G2 point: 16 u64  = 128 bytes
    // Each pair:     24 u64  = 192 bytes
    //
    // All data is interpreted in *native-endian* u64 layout.
    // ---------------------------------------------------------------------

    const G1_WORDS: usize = 8;
    const G2_WORDS: usize = 16;
    const PAIR_WORDS: usize = G1_WORDS + G2_WORDS;

    if data.is_empty() {
        anyhow::bail!("BN254_PAIRING_CHECK: data is empty");
    }

    let num_pairs = data[0] as usize;

    // Prevent absurd sizes early (optional but defensive)
    if num_pairs == 0 {
        anyhow::bail!("BN254_PAIRING_CHECK: num_pairs is zero");
    }

    let expected_len = 1 + num_pairs * PAIR_WORDS;

    validate_hint_length(data, expected_len, "PAIRING_BATCH_BN254")?;

    // Extract interleaved pairs
    // ---------------------------------------------------------------------
    // SAFETY INVARIANTS:
    // - Length has been validated exactly
    // - &[u64] memory is contiguous
    // - We only create immutable views
    // - Alignment is safe (u8 alignment = 1)
    // - Endianness is intentionally native
    // ---------------------------------------------------------------------

    let pairs_data = &data[1..];
    let mut g1_points = Vec::with_capacity(num_pairs);
    let mut g2_points = Vec::with_capacity(num_pairs);

    for i in 0..num_pairs {
        let pair_start = i * PAIR_WORDS;
        let g1_start = pair_start;
        let g2_start = pair_start + G1_WORDS;

        let g1_words = &pairs_data[g1_start..g1_start + G1_WORDS];
        let g2_words = &pairs_data[g2_start..g2_start + G2_WORDS];

        let g1_bytes =
            unsafe { std::slice::from_raw_parts(g1_words.as_ptr() as *const u8, G1_WORDS * 8) };
        let g2_bytes =
            unsafe { std::slice::from_raw_parts(g2_words.as_ptr() as *const u8, G2_WORDS * 8) };

        g1_points.push(g1_bytes);
        g2_points.push(g2_bytes);
    }

    // Build arrays of raw pointers for the FFI call
    let g1_ptrs: Vec<*const u8> = g1_points.iter().map(|p| p.as_ptr()).collect();
    let g2_ptrs: Vec<*const u8> = g2_points.iter().map(|p| p.as_ptr()).collect();

    let mut hints = Vec::new();
    unsafe {
        zisklib::bn254_pairing_check_c(g1_ptrs.as_ptr(), g2_ptrs.as_ptr(), num_pairs, &mut hints);
    }

    Ok(hints)
}
