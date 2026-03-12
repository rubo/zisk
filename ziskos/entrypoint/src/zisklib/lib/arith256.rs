use crate::syscalls::{syscall_arith256_mod, SyscallArith256ModParams};

pub fn mulmod256(
    a: &[u64; 4],
    b: &[u64; 4],
    m: &[u64; 4],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> [u64; 4] {
    let mut params = SyscallArith256ModParams { a, b, c: &[0u64; 4], module: m, d: &mut [0u64; 4] };
    syscall_arith256_mod(
        &mut params,
        #[cfg(feature = "hints")]
        hints,
    );
    *params.d
}

// ========== Pointer-based API ==========

/// Modular multiplication of 256-bit integers
///
/// # Safety
/// - `a` must point to a valid `[u64; 4]` (32 bytes).
/// - `b` must point to a valid `[u64; 4]` (32 bytes).
/// - `m` must point to a valid `[u64; 4]` (32 bytes).
/// - `result` must point to a valid `[u64; 4]` (32 bytes).
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_mulmod256_c")]
pub unsafe extern "C" fn mulmod256_c(
    a: *const u64,
    b: *const u64,
    m: *const u64,
    result: *mut u64,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) {
    let a = unsafe { &*(a as *const [u64; 4]) };
    let b = unsafe { &*(b as *const [u64; 4]) };
    let m = unsafe { &*(m as *const [u64; 4]) };
    let result = unsafe { &mut *(result as *mut [u64; 4]) };

    let dl = mulmod256(
        a,
        b,
        m,
        #[cfg(feature = "hints")]
        hints,
    );
    *result = dl;
}
