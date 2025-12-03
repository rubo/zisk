use crate::syscalls::{syscall_arith256, SyscallArith256Params};

use super::{rem_short, U256};

/// Multiplication of a large number (represented as an array of U256) by a short U256 number
///
/// It assumes that a,b > 0
pub fn mul_short(a: &[U256], b: &U256) -> Vec<U256> {
    let len_a = a.len();
    #[cfg(debug_assertions)]
    {
        assert_ne!(len_a, 0, "Input 'a' must have at least one limb");
        assert!(!a[len_a - 1].is_zero(), "Input 'a' must not have leading zeros");
        assert!(!b.is_zero(), "Input 'b' must be greater than zero");
    }

    let mut out = vec![U256::ZERO; len_a + 1];
    let mut carry = U256::ZERO;

    for i in 0..len_a {
        // Compute a[i]·b + carry
        let cin = carry;
        let mut params = SyscallArith256Params {
            a: a[i].as_limbs(),
            b: b.as_limbs(),
            c: cin.as_limbs(),
            dl: out[i].as_limbs_mut(),
            dh: carry.as_limbs_mut(),
        };
        syscall_arith256(&mut params);
    }

    if !carry.is_zero() {
        out[len_a] = carry;
    } else {
        out.pop();
    }

    out
}

pub fn mul_short_short(a: &U256, b: &U256) -> ([U256; 2], usize) {
    #[cfg(debug_assertions)]
    {
        assert!(!a.is_zero(), "Input 'a' must not have leading zeros");
        assert!(!b.is_zero(), "Input 'b' must not be zero");
    }

    let mut out = [U256::ZERO; 2];

    // Compute a * b
    let mut dh = [0u64; 4];
    let mut mul_params = SyscallArith256Params {
        a: a.as_limbs(),
        b: b.as_limbs(),
        c: U256::ZERO.as_limbs(),
        dl: out[0].as_limbs_mut(),
        dh: &mut dh,
    };
    syscall_arith256(&mut mul_params);

    let len = if dh == [0u64; 4] {
        1
    } else {
        out[1] = U256::from_u64s(&dh);
        2
    };

    (out, len)
}

pub fn mul_and_reduce_short(
    a: &U256,
    b: &U256,
    modulus: &U256,
    quo: &mut [u64; 8],
    rem: &mut [u64; 4],
) -> U256 {
    #[cfg(debug_assertions)]
    {
        assert!(!modulus.is_zero(), "Input 'modulus' must not be zero");
    }

    let (mul, len) = mul_short_short(a, b);

    // Use short division
    rem_short(&mul[..len], modulus, quo, rem)
}
