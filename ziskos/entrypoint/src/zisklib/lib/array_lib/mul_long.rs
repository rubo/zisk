use crate::syscalls::{
    syscall_add256, syscall_arith256, SyscallAdd256Params, SyscallArith256Params,
};

use super::{mul_short, rem_long, U256};

/// Multiplication of two large numbers (represented as arrays of U256)
///
/// It assumes that a,b > 0 and len(b) > 1
pub fn mul_long(a: &[U256], b: &[U256]) -> Vec<U256> {
    let len_a = a.len();
    let len_b = b.len();
    #[cfg(debug_assertions)]
    {
        assert_ne!(len_a, 0, "Input 'a' must have at least one limb");
        assert!(len_b > 1, "Input 'b' must have more than one limb");
        assert!(!a[len_a - 1].is_zero(), "Input 'a' must not have leading zeros");
        assert!(!b[len_b - 1].is_zero(), "Input 'b' must not have leading zeros");
    }

    let mut out = vec![U256::ZERO; len_a + len_b];

    // Start with a[0]·b[0]
    let mut params = SyscallArith256Params {
        a: a[0].as_limbs(),
        b: b[0].as_limbs(),
        c: U256::ZERO.as_limbs(),
        dl: out[0].as_limbs_mut(),
        dh: &mut [0, 0, 0, 0],
    };
    syscall_arith256(&mut params);

    // Propagate the carry
    out[1] = U256::from_u64s(params.dh);

    // Finish the first row
    for j in 1..len_b {
        // Compute a[0]·b[j] + out[j]
        let out_j = out[j];
        let mut params = SyscallArith256Params {
            a: a[0].as_limbs(),
            b: b[j].as_limbs(),
            c: out_j.as_limbs(),
            dl: out[j].as_limbs_mut(),
            dh: &mut [0, 0, 0, 0],
        };
        syscall_arith256(&mut params);

        // Propagate the carry
        out[j + 1] = U256::from_u64s(params.dh);
    }

    // Finish the remaining rows
    for i in 1..len_a {
        let mut carry_flag = 0u64;
        for j in 0..(len_b - 1) {
            // Compute a[i]·b[j] + out[i + j]
            let out_ij = out[i + j];
            let mut params_arith = SyscallArith256Params {
                a: a[i].as_limbs(),
                b: b[j].as_limbs(),
                c: out_ij.as_limbs(),
                dl: &mut [0, 0, 0, 0],
                dh: &mut [0, 0, 0, 0],
            };
            syscall_arith256(&mut params_arith);

            // Set the result
            out[i + j] = U256::from_u64s(params_arith.dl);

            if carry_flag == 1 {
                let mut params_add = SyscallAdd256Params {
                    a: &params_arith.dh.clone(),
                    b: U256::ZERO.as_limbs(),
                    cin: 1,
                    c: params_arith.dh,
                };
                let _carry = syscall_add256(&mut params_add);

                debug_assert!(_carry == 0, "Unexpected carry in intermediate addition");
            }

            // Update out[i+j+1] with carry
            let out_ij1 = out[i + j + 1];
            let mut params_add = SyscallAdd256Params {
                a: out_ij1.as_limbs(),
                b: params_arith.dh,
                cin: 0,
                c: out[i + j + 1].as_limbs_mut(),
            };
            carry_flag = syscall_add256(&mut params_add);
        }

        // Last chunk isolated

        // Compute a[i]·b[len_b - 1] + out[i + len_b - 1]
        let out_ilb1 = out[i + len_b - 1];
        let mut params_arith = SyscallArith256Params {
            a: a[i].as_limbs(),
            b: b[len_b - 1].as_limbs(),
            c: out_ilb1.as_limbs(),
            dl: out[i + len_b - 1].as_limbs_mut(),
            dh: &mut [0, 0, 0, 0],
        };
        syscall_arith256(&mut params_arith);

        if carry_flag == 1 {
            let a_in = *params_arith.dh;
            let mut params_add = SyscallAdd256Params {
                a: &a_in,
                b: U256::ZERO.as_limbs(),
                cin: 1,
                c: params_arith.dh,
            };
            let _carry = syscall_add256(&mut params_add);

            debug_assert!(_carry == 0, "Unexpected carry in intermediate addition");
        }

        // Set out[i+j+1] = carry
        out[i + len_b] = U256::from_u64s(params_arith.dh);
    }

    if out[len_a + len_b - 1].is_zero() {
        out.pop();
    }

    out
}

/// Multiplication of two large numbers (represented as arrays of U256) followed by reduction modulo a third large number
///
/// It assumes that modulus > 0
pub fn mul_and_reduce_long(a: &[U256], b: &[U256], modulus: &[U256]) -> Vec<U256> {
    #[cfg(debug_assertions)]
    {
        let len_m = modulus.len();
        assert_ne!(len_m, 0, "Input 'modulus' must have at least one limb");
        assert!(!modulus[len_m - 1].is_zero(), "Input 'modulus' must not have leading zeros");
    }

    let mul = if b.len() == 1 { mul_short(a, &b[0]) } else { mul_long(a, b) };

    rem_long(&mul, modulus)
}
