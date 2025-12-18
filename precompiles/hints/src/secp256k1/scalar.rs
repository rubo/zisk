use lib_c::{arith256_mod_c, secp256k1_fn_inv_c};
use ziskos::zisklib::lib::secp256k1::constants::N;
use ziskos::zisklib::lt;

const Z: [u64; 4] = [0, 0, 0, 0];
const O: [u64; 4] = [0, 0, 0, 0];

pub fn secp256k1_fn_reduce(x: &[u64; 4], hints: &mut Vec<u64>) -> [u64; 4] {
    if lt(x, &N) {
        return *x;
    }

    // x·1 + 0
    let mut module: [u64; 4] = N.clone();
    let mut d: [u64; 4] = [0; 4];
    arith256_mod_c(x, &O, &Z, &mut module, &mut d);
    hints.extend_from_slice(&d);

    d
}

pub fn secp256k1_fn_mul(x: &[u64; 4], y: &[u64; 4], result: &mut [u64; 4], hints: &mut Vec<u64>) {
    // x·y + 0
    let mut module: [u64; 4] = N.clone();
    arith256_mod_c(x, y, &Z, &mut module, result);
    hints.extend_from_slice(result);
}

/// Inverts a non-zero element `x`
pub fn secp256k1_fn_inv(x: &[u64; 4], x_inv: &mut [u64; 4], hints: &mut Vec<u64>) {
    // Hint the inverse
    secp256k1_fn_inv_c(x, x_inv);
    hints.extend_from_slice(x_inv);

    // Check that x·x_inv = 1 (N)
    let mut module: [u64; 4] = N.clone();
    let mut d: [u64; 4] = [0; 4];
    arith256_mod_c(x, x_inv, &Z, &mut module, &mut d);
    hints.extend_from_slice(&module);
    hints.extend_from_slice(&d);
    assert_eq!(d, [0x1, 0x0, 0x0, 0x0]);
}
