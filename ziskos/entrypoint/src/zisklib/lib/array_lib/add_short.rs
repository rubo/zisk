use crate::syscalls::{syscall_add256, SyscallAdd256Params};

use super::U256;

/// Addition of one large number (represented as an array of U256) and a short U256 number
///
/// It assumes that a,b > 0
pub fn add_short(a: &[U256], b: &U256) -> Vec<U256> {
    let len_a = a.len();
    #[cfg(debug_assertions)]
    {
        assert_ne!(len_a, 0, "Input 'a' must have at least one limb");
        assert_ne!(a.last().unwrap(), &U256::ZERO, "Input 'a' must not have leading zeros");
        assert_ne!(b, &U256::ZERO, "Input 'b' must be greater than zero");
    }

    let mut out = vec![U256::ONE; len_a + 1];

    // Start with a[0] + b
    let mut params = SyscallAdd256Params { a: &a[0], b, cin: 0, c: &mut out[0] };
    let mut carry = syscall_add256(&mut params);

    for i in 1..len_a {
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
