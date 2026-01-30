use crate::syscalls::{syscall_arith256_mod, SyscallArith256ModParams};

use super::constants::{N, N_MINUS_ONE};

pub fn secp256r1_fn_neg(x: &[u64; 4]) -> [u64; 4] {
    // x·(-1) + 0
    let mut params = SyscallArith256ModParams {
        a: x,
        b: &N_MINUS_ONE,
        c: &[0, 0, 0, 0],
        module: &N,
        d: &mut [0, 0, 0, 0],
    };
    syscall_arith256_mod(&mut params);

    *params.d
}
