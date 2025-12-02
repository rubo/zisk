use crate::syscalls::{syscall_add256, SyscallAdd256Params};

use super::U256;

/// Addition of two large numbers (represented as arrays of U256)
///
/// It assumes that a,b > 0 and len(a) >= len(b)
pub fn add_agtb(a: &[U256], b: &[U256]) -> Vec<U256> {
    let len_a = a.len();
    let len_b = b.len();
    #[cfg(debug_assertions)]
    {
        assert_ne!(len_b, 0, "Input 'b' must have at least one limb");
        assert!(len_a >= len_b, "Input 'a' must be greater than 'b'");
        assert_ne!(a.last().unwrap(), &U256::ZERO, "Input 'a' must not have leading zeros");
        assert_ne!(b.last().unwrap(), &U256::ZERO, "Input 'b' must not have leading zeros");
    }

    let mut out: Vec<U256> = vec![U256::ONE; len_a + 1];

    // Start with a[0] + b[0]
    let mut params = SyscallAdd256Params { a: &a[0], b: &b[0], cin: 0, c: &mut out[0] };
    let mut carry = syscall_add256(&mut params);

    for i in 1..len_b {
        // Compute a[i] + b[i] + carry
        let mut params = SyscallAdd256Params { a: &a[i], b: &b[i], cin: carry, c: &mut out[i] };
        carry = syscall_add256(&mut params);
    }

    for i in len_b..len_a {
        if carry == 1 {
            // Compute a[i] + carry
            let mut params =
                SyscallAdd256Params { a: &a[i], b: &U256::ZERO, cin: 1, c: &mut out[i] };
            carry = syscall_add256(&mut params);
        } else {
            // Directly copy a[i] to out[i]
            out[i] = a[i];
        }
    }

    if carry == 0 {
        out.pop();
    }

    out
}
