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

/// Processes an `HINT_BN254_PAIRING_CHECK` hint.
#[inline]
pub fn bn254_pairing_check_hint(data: &[u64]) -> Result<Vec<u64>> {
    // ---------------------------------------------------------------------
    // Input format:
    //
    // data[0]                = num_pairs
    // data[1 ..]             = G1 points (num_pairs * 8 u64s)
    // data[1 + g1_len ..]    = G2 points (num_pairs * 16 u64s)
    //
    // Each G1 point:  8 u64  = 64 bytes
    // Each G2 point: 16 u64  = 128 bytes
    //
    // All data is interpreted in *native-endian* u64 layout.
    // ---------------------------------------------------------------------

    const G1_WORDS: usize = 8;
    const G2_WORDS: usize = 16;

    if data.is_empty() {
        anyhow::bail!("BN254_PAIRING_CHECK: data is empty");
    }

    let num_pairs = data[0] as usize;

    // Prevent absurd sizes early (optional but defensive)
    if num_pairs == 0 {
        anyhow::bail!("BN254_PAIRING_CHECK: num_pairs is zero");
    }

    println!("num_pairs: {}", num_pairs);

    let expected_len = 1 + num_pairs * (G1_WORDS + G2_WORDS);

    validate_hint_length(data, expected_len, "PAIRING_BATCH_BN254")?;

    // Split input buffer
    // ---------------------------------------------------------------------
    // Reinterpret &[u64] as grouped byte slices (zero-copy)
    //
    // SAFETY INVARIANTS:
    // - Length has been validated exactly
    // - &[u64] memory is contiguous
    // - We only create immutable views
    // - Alignment is safe (u8 alignment = 1)
    // - Endianness is intentionally native
    // ---------------------------------------------------------------------

    let g1_start = 1;
    let g1_end = g1_start + num_pairs * G1_WORDS;
    let g2_start = g1_end;
    let g2_end = g2_start + num_pairs * G2_WORDS;

    let g1_words = &data[g1_start..g1_end];
    let g2_words = &data[g2_start..g2_end];

    let g1_points: &[[u8; G1_WORDS * 8]] =
        unsafe { reinterpret_u64_as_bytes::<{ G1_WORDS * 8 }>(g1_words, num_pairs) };

    let g2_points: &[[u8; G2_WORDS * 8]] =
        unsafe { reinterpret_u64_as_bytes::<{ G2_WORDS * 8 }>(g2_words, num_pairs) };

    // Build arrays of raw pointers for the FFI call
    let g1_ptrs: Vec<*const u8> = g1_points.iter().map(|p| p.as_ptr()).collect();
    let g2_ptrs: Vec<*const u8> = g2_points.iter().map(|p| p.as_ptr()).collect();

    let mut hints = Vec::new();
    unsafe {
        zisklib::bn254_pairing_check_c(g1_ptrs.as_ptr(), g2_ptrs.as_ptr(), num_pairs, &mut hints);
    }

    Ok(hints)
}

#[inline]
unsafe fn reinterpret_u64_as_bytes<const BYTES: usize>(
    slice: &[u64],
    count: usize,
) -> &[[u8; BYTES]] {
    debug_assert_eq!(slice.len(), count * (BYTES / 8));
    std::slice::from_raw_parts(slice.as_ptr().cast(), count)
}
