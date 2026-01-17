use crate::handlers::validate_hint_length;
use crate::hint_fields;
use crate::zisklib;

// Processes a SECP256K1_FN_REDUCE hint.
#[inline]
pub fn secp256k1_fn_reduce_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FN_REDUCE")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fn_reduce(x, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FN_ADD hint.
#[inline]
pub fn secp256k1_fn_add_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4, Y: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FN_ADD")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();
    let y: &[u64; Y_SIZE] = data[Y_OFFSET..Y_OFFSET + Y_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fn_add(x, y, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FN_NEG hint.
#[inline]
pub fn secp256k1_fn_neg_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FN_NEG")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fn_neg(x, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FN_SUB hint.
#[inline]
pub fn secp256k1_fn_sub_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4, Y: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FN_SUB")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();
    let y: &[u64; Y_SIZE] = data[Y_OFFSET..Y_OFFSET + Y_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fn_sub(x, y, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FN_MUL hint.
#[inline]
pub fn secp256k1_fn_mul_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4, Y: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FN_MUL")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();
    let y: &[u64; Y_SIZE] = data[Y_OFFSET..Y_OFFSET + Y_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fn_mul(x, y, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FN_INV hint.
#[inline]
pub fn secp256k1_fn_inv_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FN_INV")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fn_inv(x, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FP_REDUCE hint.
#[inline]
pub fn secp256k1_fp_reduce_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FP_REDUCE")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fp_reduce(x, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FP_ADD hint.
#[inline]
pub fn secp256k1_fp_add_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4, Y: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FP_ADD")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();
    let y: &[u64; Y_SIZE] = data[Y_OFFSET..Y_OFFSET + Y_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fp_add(x, y, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FP_NEGATE hint.
#[inline]
pub fn secp256k1_fp_negate_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FP_NEGATE")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fp_negate(x, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FP_MUL hint.
#[inline]
pub fn secp256k1_fp_mul_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4, Y: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FP_MUL")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();
    let y: &[u64; Y_SIZE] = data[Y_OFFSET..Y_OFFSET + Y_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_fp_mul(x, y, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_FP_MUL_SCALAR hint.
#[inline]
pub fn secp256k1_fp_mul_scalar_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X: 4, SCALAR: 1];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_FP_MUL_SCALAR")?;

    let x: &[u64; X_SIZE] = data[X_OFFSET..X_OFFSET + X_SIZE].try_into().unwrap();
    let scalar: u64 = data[SCALAR_OFFSET];

    let mut hints = Vec::new();
    zisklib::secp256k1_fp_mul_scalar(x, scalar, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_TO_AFFINE hint.
#[inline]
pub fn secp256k1_to_affine_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![P: 12];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_TO_AFFINE")?;

    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_to_affine(p, &mut hints);

    Ok(hints)
}

// Processes a SECP256K1_DECOMPRESS hint.
#[inline]
pub fn secp256k1_decompress_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![X_BYTES: 4, Y_IS_ODD: 1];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_DECOMPRESS")?;

    let x: &[u64; X_BYTES_SIZE] =
        data[X_BYTES_OFFSET..X_BYTES_OFFSET + X_BYTES_SIZE].try_into().unwrap();
    let y_is_odd = (data[Y_IS_ODD_OFFSET] >> 56) != 0;

    let mut hints = Vec::new();
    zisklib::secp256k1_decompress(x, y_is_odd, &mut hints)
        .map_err(|e| format!("secp256k1_decompress failed: {}", e))?;

    Ok(hints)
}

// Processes a SECP256K1_DOUBLE_SCALAR_MUL_WITH_G hint.
#[inline]
pub fn secp256k1_double_scalar_mul_with_g_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![K1: 4, K2: 4, P: 8];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_DOUBLE_SCALAR_MUL_WITH_G")?;

    let k1: &[u64; K1_SIZE] = data[K1_OFFSET..K1_OFFSET + K1_SIZE].try_into().unwrap();
    let k2: &[u64; K2_SIZE] = data[K2_OFFSET..K2_OFFSET + K2_SIZE].try_into().unwrap();
    let p: &[u64; P_SIZE] = data[P_OFFSET..P_OFFSET + P_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_double_scalar_mul_with_g(k1, k2, p, &mut hints);

    Ok(hints)
}

/// Processes an ECDSA_VERIFY hint.
#[inline]
pub fn secp256k1_ecdsa_verify_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![PK: 8, Z: 4, R: 4, S: 4];

    validate_hint_length(data, EXPECTED_LEN, "SECP256K1_ECDSA_VERIFY")?;

    let pk: &[u64; PK_SIZE] = data[PK_OFFSET..PK_OFFSET + PK_SIZE].try_into().unwrap();
    let z: &[u64; Z_SIZE] = data[Z_OFFSET..Z_OFFSET + Z_SIZE].try_into().unwrap();
    let r: &[u64; R_SIZE] = data[R_OFFSET..R_OFFSET + R_SIZE].try_into().unwrap();
    let s: &[u64; S_SIZE] = data[S_OFFSET..S_OFFSET + S_SIZE].try_into().unwrap();

    let mut hints = Vec::new();
    zisklib::secp256k1_ecdsa_verify(pk, z, r, s, &mut hints);

    Ok(hints)
}
