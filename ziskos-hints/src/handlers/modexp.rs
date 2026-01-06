use crate::{handlers::validate_hint_length, zisklib};

// Processes a MODEXP hint.
#[inline]
pub fn modexp_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    if data.is_empty() {
        return Err("MODEXP hint data is empty".to_string());
    }

    // Parse base
    let base_len = data[0] as usize;
    let base_start = 1;
    let base_end = base_start + base_len;

    if data.len() < base_end + 1 {
        return Err(format!(
            "MODEXP hint data too short for base (expected at least {} elements)",
            base_end + 1
        ));
    }
    let base = &data[base_start..base_end];

    // Parse exponent
    let exp_len = data[base_end] as usize;
    let exp_start = base_end + 1;
    let exp_end = exp_start + exp_len;

    if data.len() < exp_end + 1 {
        return Err(format!(
            "MODEXP hint data too short for exponent (expected at least {} elements)",
            exp_end + 1
        ));
    }
    let exp = &data[exp_start..exp_end];

    // Parse modulus
    let modulus_len = data[exp_end] as usize;
    let modulus_start = exp_end + 1;
    let expected_len = modulus_start + modulus_len;

    validate_hint_length(data, expected_len, "MODEXP")?;

    let modulus = &data[modulus_start..expected_len];

    let mut processed_hints = Vec::new();

    zisklib::modexp_u64(base, exp, modulus, &mut processed_hints);

    Ok(processed_hints)
}
