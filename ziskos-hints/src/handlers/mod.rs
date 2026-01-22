pub mod bls381;
pub mod bn254;
pub mod kzg;
pub mod modexp;
pub mod secp256k1;
pub mod sha256;

/// Macro to generate size, offset, and expected length constants for hint data fields.
///
/// # Example
/// ```ignore
/// hint_fields![A: 4, B: 4, M: 4]
/// ```
/// Generates:
/// - `A_SIZE`, `B_SIZE`, `M_SIZE` constants
/// - `A_OFFSET`, `B_OFFSET`, `M_OFFSET` constants (cumulative offsets)
/// - `EXPECTED_LEN` constant (sum of all sizes)
#[macro_export]
macro_rules! hint_fields {
    ($($name:ident: $size:expr),+ $(,)?) => {
        paste::paste! {
            $(
                #[allow(dead_code)]
                const [<$name _SIZE>]: usize = $size;
            )+
        }

        hint_fields!(@offsets 0, $($name: $size),+);

        #[allow(dead_code)]
        const EXPECTED_LEN: usize = hint_fields!(@sum $($size),+);
        #[allow(dead_code)]
        const EXPECTED_LEN_U64: usize = EXPECTED_LEN.div_ceil(8);
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

/// Read a length-prefixed field from hint data
#[inline]
fn read_field<'a>(data: &'a [u64], pos: &mut usize) -> anyhow::Result<&'a [u64]> {
    let len =
        *data.get(*pos).ok_or("MODEXP hint data too short").map_err(anyhow::Error::msg)? as usize;
    *pos += 1;
    let field = data
        .get(*pos..*pos + len)
        .ok_or("MODEXP hint data too short")
        .map_err(anyhow::Error::msg)?;
    *pos += len;
    Ok(field)
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
/// * `Err(anyhow::Error)` - If the length is incorrect
#[inline]
fn validate_hint_length<T>(data: &[T], expected_len: usize, hint_name: &str) -> anyhow::Result<()> {
    if data.len() != expected_len {
        anyhow::bail!(
            "Invalid {} hint length: expected {}, got {}",
            hint_name,
            expected_len,
            data.len(),
        );
    }
    Ok(())
}

/// Validates that the hint data has at least the minimum required length.
///
/// # Arguments
///
/// * `data` - The hint data to validate
/// * `min_len` - The minimum required length
/// * `hint_name` - The name of the hint type for error messages
///
/// # Returns
///
/// * `Ok(())` - If the length is sufficient
/// * `Err(anyhow::Error)` - If the length is too short
#[inline]
fn validate_hint_min_length<T>(data: &[T], min_len: usize, hint_name: &str) -> anyhow::Result<()> {
    if data.len() < min_len {
        anyhow::bail!(
            "Invalid {} hint length: expected at least {}, got {}",
            hint_name,
            min_len,
            data.len(),
        );
    }
    Ok(())
}
