use crate::syscalls::{
    syscall_add256, syscall_arith256, SyscallAdd256Params, SyscallArith256Params,
};

use super::{rem_long, rem_short, U256};

/// Squaring of a large number (represented as an array of U256)
//                                        a3    a2    a1      a0
//                                      * a3    a2    a1      a0
//         ------------------------------------------------------- 0
//                               Y       2*a0*a2   2*a0*a1  a0*a0
//         ------------------------------------------------------- 1
//               2*a1*a3+Z    2*a1*a2     a1*a1        X      0
//         ------------------------------------------------------- 2
//  Z   Y     2*a2*a3   a2*a2        X      X          0      0
//         ------------------------------------------------------- 3
//    a3*a3     X        X          X           0      0      0
//         ------------------------------------------------------- 4
//                          RESULT
pub fn square(a: &[U256]) -> Vec<U256> {
    let len_a = a.len();
    #[cfg(debug_assertions)]
    {
        assert_ne!(len_a, 0, "Input 'a' must have at least one limb");
        assert_ne!(a.last().unwrap(), &U256::ZERO, "Input 'a' must not have leading zeros");
    }

    let mut out = vec![U256::ZERO; 2 * len_a];

    // Step 1: Compute all diagonal terms a[i] * a[i]
    for i in 0..len_a {
        // Compute the diagonal:
        //      a[i]·a[i] = dh·B + dl
        // and set out[2 * i] = dl and out[2 * i + 1] = dh
        let mut ai_ai = SyscallArith256Params {
            a: &a[i],
            b: &a[i],
            c: &U256::ZERO,
            dl: &mut out[2 * i],
            dh: &mut [0, 0, 0, 0],
        };
        syscall_arith256(&mut ai_ai);

        out[2 * i + 1] = U256::from_u64s(ai_ai.dh);
    }

    // Step 2: Compute all cross terms 2·a[i]·a[j] for i < j
    for i in 0..len_a {
        for j in (i + 1)..len_a {
            // First compute a[i]·a[j] = h₁·B + l₁
            let mut ai_aj = SyscallArith256Params {
                a: &a[i],
                b: &a[j],
                c: &U256::ZERO,
                dl: &mut [0, 0, 0, 0],
                dh: &mut [0, 0, 0, 0],
            };
            syscall_arith256(&mut ai_aj);

            // Double the result 2·a[i]·a[j]

            // Start by doubling the lower chunk: 2·l₁ = [1/0]·B + l₂
            let mut dbl_low = SyscallAdd256Params {
                a: &ai_aj.dl.clone(),
                b: &ai_aj.dl.clone(),
                cin: 0,
                c: &mut [0, 0, 0, 0],
            };
            let dbl_low_carry = syscall_add256(&mut dbl_low);

            // Next, double the higher chunk: 2·h₁·B = [1/0]·B² + h₂·B
            let mut dbl_high = SyscallAdd256Params {
                a: &ai_aj.dh.clone(),
                b: &ai_aj.dh.clone(),
                cin: 0,
                c: &mut [0, 0, 0, 0],
            };
            let dbl_high_carry = syscall_add256(&mut dbl_high);

            // If there's a carry from doubling the low part, add it to the high part
            if dbl_low_carry != 0 {
                let mut add = SyscallAdd256Params {
                    a: &dbl_high.c.clone(),
                    b: &U256::ZERO,
                    cin: 1,
                    c: dbl_high.c,
                };
                let _carry = syscall_add256(&mut add);

                debug_assert!(_carry == 0, "Unexpected carry in intermediate addition");
            }

            // The result is expressed as: dbl_high.dh·B² + dbl_high.dl·B + dbl_low.dl

            // Now update out[i+j], out[i+j+1] and out[i+j+2] with this result

            // Update out[i+j]
            let mut add_low = SyscallAdd256Params {
                a: &out[i + j].clone(),
                b: dbl_low.c,
                cin: 0,
                c: &mut [0, 0, 0, 0],
            };
            let add_low_carry = syscall_add256(&mut add_low);
            out[i + j] = U256::from_u64s(add_low.c);

            if add_low_carry != 0 {
                let mut add = SyscallAdd256Params {
                    a: &out[i + j + 1].clone(),
                    b: &U256::ZERO,
                    cin: 1,
                    c: &mut out[i + j + 1],
                };
                let add_carry = syscall_add256(&mut add);

                if add_carry != 0 {
                    let mut add2 = SyscallAdd256Params {
                        a: &out[i + j + 2].clone(),
                        b: &U256::ZERO,
                        cin: 1,
                        c: &mut out[i + j + 2],
                    };
                    let _carry = syscall_add256(&mut add2);

                    debug_assert!(_carry == 0, "Unexpected carry in intermediate addition");
                }
            }

            // Update out[i+j+1]
            let mut add_mid = SyscallAdd256Params {
                a: &out[i + j + 1].clone(),
                b: dbl_high.c,
                cin: 0,
                c: &mut [0, 0, 0, 0],
            };
            let add_mid_carry = syscall_add256(&mut add_mid);
            out[i + j + 1] = U256::from_u64s(add_mid.c);

            if add_mid_carry != 0 {
                let mut add = SyscallAdd256Params {
                    a: &out[i + j + 2].clone(),
                    b: &U256::ZERO,
                    cin: 1,
                    c: &mut out[i + j + 2],
                };
                let _carry = syscall_add256(&mut add);

                debug_assert!(_carry == 0, "Unexpected carry in intermediate addition");
            }

            // Update out[i+j+2]
            if dbl_high_carry != 0 {
                let mut add = SyscallAdd256Params {
                    a: &out[i + j + 2].clone(),
                    b: &U256::ZERO,
                    cin: 1,
                    c: &mut out[i + j + 2],
                };
                let _carry = syscall_add256(&mut add);

                debug_assert!(_carry == 0, "Unexpected carry in intermediate addition");
            }
        }
    }

    if out.last() == Some(&U256::ZERO) {
        out.pop();
    }

    out
}

/// Squaring of a large number (represented as an array of U256) followed by reduction modulo a second large number
///
/// It assumes that modulus > 0
pub fn square_and_reduce(a: &[U256], modulus: &[U256]) -> Vec<U256> {
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

    let sq = square(a);

    // If a·b < modulus, then the result is just a·b
    if U256::lt_slices(&sq, modulus) {
        return sq;
    }

    if len_m == 1 {
        // If modulus has only one limb, we can use short division
        vec![rem_short(&sq, &modulus[0])]
    } else {
        // Otherwise, use long division
        rem_long(&sq, modulus)
    }
}
