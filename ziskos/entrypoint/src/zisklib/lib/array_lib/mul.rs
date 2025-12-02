use super::{mul_long, mul_short, rem_long, rem_short, U256};

/// Multiplication of two large numbers (represented as arrays of U256) followed by reduction modulo a third large number
///
/// It assumes that modulus > 0
pub fn mul_and_reduce(a: &[U256], b: &[U256], modulus: &[U256]) -> Vec<U256> {
    let len_m = modulus.len();
    #[cfg(debug_assertions)]
    {
        assert_ne!(len_m, 0, "Input 'modulus' must have at least one limb");
        assert_ne!(
            modulus.last().unwrap(),
            &U256::ZERO,
            "Input 'modulus' must not have leading zeros"
        );
    }

    let mul = if b.len() == 1 { mul_short(a, &b[0]) } else { mul_long(a, b) };

    // If a·b < modulus, then the result is just a·b
    if U256::lt_slices(&mul, modulus) {
        return mul;
    }

    if len_m == 1 {
        // If modulus has only one limb, we can use short division
        vec![rem_short(&mul, &modulus[0])]
    } else {
        // Otherwise, use long division
        rem_long(&mul, modulus)
    }
}
