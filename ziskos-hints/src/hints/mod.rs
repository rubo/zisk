//! Hint processing utilities for ziskos-hints

use crate::zisklib;

/// Macro to generate size, offset, and expected length constants for hint data fields.
///
/// # Example
/// ```ignore
/// hint_fields! {
///     A: 4,
///     B: 4,
///     M: 4
/// }
/// ```
/// Generates:
/// - `A_SIZE`, `B_SIZE`, `M_SIZE` constants
/// - `A_OFFSET`, `B_OFFSET`, `M_OFFSET` constants (cumulative offsets)
/// - `EXPECTED_LEN` constant (sum of all sizes)
macro_rules! hint_fields {
    ($($name:ident: $size:expr),+ $(,)?) => {
        paste::paste! {
            $(
                #[allow(dead_code)]
                const [<$name _SIZE>]: usize = $size;
            )+
        }

        hint_fields!(@offsets 0, $($name: $size),+);

        const EXPECTED_LEN: usize = hint_fields!(@sum $($size),+);
    };

    (@offsets $offset:expr, $name:ident: $size:expr) => {
        paste::paste! {
            const [<$name _OFFSET>]: usize = $offset;
        }
    };

    (@offsets $offset:expr, $name:ident: $size:expr, $($rest_name:ident: $rest_size:expr),+) => {
        paste::paste! {
            const [<$name _OFFSET>]: usize = $offset;
        }
        hint_fields!(@offsets $offset + $size, $($rest_name: $rest_size),+);
    };

    (@sum $size:expr) => { $size };
    (@sum $size:expr, $($rest:expr),+) => {
        $size + hint_fields!(@sum $($rest),+)
    };
}

/// Processes an ECRECOVER hint.
///
/// # Arguments
///
/// * `data` - The hint data containing pk(8) + z(4) + r(4) + s(4) = 20 u64 values
///
/// # Returns
///
/// * `Ok(Vec<u64>)` - The processed hints from the verification
/// * `Err` - If the data length is invalid
#[inline]
pub fn process_ecrecover_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![PK: 8, Z: 4, R: 4, S: 4];

    validate_hint_length(data, EXPECTED_LEN, "ECRECOVER")?;

    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::secp256k1_ecdsa_verify_c(
            &data[PK_OFFSET],
            &data[Z_OFFSET],
            &data[R_OFFSET],
            &data[S_OFFSET],
            &mut processed_hints,
        );
    }

    Ok(processed_hints)
}

/// Processes a REDMOD256 hint.
#[inline]
pub fn process_redmod256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, M: 4];

    validate_hint_length(data, EXPECTED_LEN, "REDMOD256")?;

    let mut result: [u64; 4] = [0; 4];
    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::redmod256_c(
            &data[A_OFFSET],
            &data[M_OFFSET],
            &mut result[0],
            &mut processed_hints,
        );
    }

    Ok(processed_hints)
}

/// Processes an ADDMOD256 hint.
#[inline]
pub fn process_addmod256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, B: 4, M: 4];

    validate_hint_length(data, EXPECTED_LEN, "ADDMOD256")?;

    let mut result: [u64; 4] = [0; 4];
    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::addmod256_c(
            &data[A_OFFSET],
            &data[B_OFFSET],
            &data[M_OFFSET],
            &mut result[0],
            &mut processed_hints,
        );
    }

    Ok(processed_hints)
}

/// Processes a MULMOD256 hint.
#[inline]
pub fn process_mulmod256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, B: 4, M: 4];

    validate_hint_length(data, EXPECTED_LEN, "MULMOD256")?;

    let mut result: [u64; 4] = [0; 4];
    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::mulmod256_c(
            &data[A_OFFSET],
            &data[B_OFFSET],
            &data[M_OFFSET],
            &mut result[0],
            &mut processed_hints,
        );
    }

    Ok(processed_hints)
}

/// Processes a DIVREM256 hint.
#[inline]
pub fn process_divrem256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, B: 4];

    validate_hint_length(data, EXPECTED_LEN, "DIVREM256")?;

    let mut processed_hints = Vec::new();

    let mut q: [u64; 4] = [0; 4];
    let mut r: [u64; 4] = [0; 4];

    unsafe {
        zisklib::divrem256_c(
            &data[A_OFFSET],
            &data[B_OFFSET],
            &mut q[0],
            &mut r[0],
            &mut processed_hints,
        );
    }

    Ok(processed_hints)
}

/// Processes a WPOW256 hint.
#[inline]
pub fn process_wpow256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, EXP: 4];

    validate_hint_length(data, EXPECTED_LEN, "WPOW256")?;

    let mut result: [u64; 4] = [0; 4];
    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::wpow256_c(
            &data[A_OFFSET],
            &data[EXP_OFFSET],
            &mut result[0],
            &mut processed_hints,
        );
    }

    Ok(processed_hints)
}

/// Processes an OMUL256 hint.
#[inline]
pub fn process_omul256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, B: 4];

    validate_hint_length(data, EXPECTED_LEN, "OMUL256")?;

    let mut result: [u64; 4] = [0; 4];
    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::omul256_c(&data[A_OFFSET], &data[B_OFFSET], &mut result[0], &mut processed_hints);
    }

    Ok(processed_hints)
}

/// Processes a WMUL256 hint.
#[inline]
pub fn process_wmul256_hint(data: &[u64]) -> Result<Vec<u64>, String> {
    hint_fields![A: 4, B: 4];

    validate_hint_length(data, EXPECTED_LEN, "WMUL256")?;

    let mut result: [u64; 4] = [0; 4];
    let mut processed_hints = Vec::new();

    unsafe {
        zisklib::wmul256_c(&data[A_OFFSET], &data[B_OFFSET], &mut result[0], &mut processed_hints);
    }

    Ok(processed_hints)
}

/// Validates that the hint data has the expected length.
///
/// # Arguments
///
/// * `data` - The hint data to validate
/// * `expected_len` - The expected number of u64 values
/// * `hint_name` - The name of the hint type for error messages
///
/// # Returns
///
/// * `Ok(())` - If the length is correct
/// * `Err(String)` - If the length is incorrect
#[inline]
fn validate_hint_length(data: &[u64], expected_len: usize, hint_name: &str) -> Result<(), String> {
    if data.len() != expected_len {
        return Err(format!(
            "Invalid {} hint length: expected {}, got {}",
            hint_name,
            expected_len,
            data.len()
        ));
    }
    Ok(())
}
