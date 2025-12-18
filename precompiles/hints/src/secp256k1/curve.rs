use crate::{secp256k1_fn_inv, secp256k1_fn_mul, secp256k1_fn_reduce};
use ziskos::{
    syscalls::SyscallPoint256,
    zisklib::{
        constants::{G_X, G_Y},
        eq,
        fcalls_impl::msb_pos_256::msb_pos_256,
    },
};

/// Given points `p1` and `p2`, performs the point addition `p1 + p2` and assigns the result to `p1`.
/// It assumes that `p1` and `p2` are from the Secp256k1 curve, that `p1,p2 != 𝒪` and that `p2 != p1,-p1`
fn add_points_assign(p1: &mut SyscallPoint256, p2: &SyscallPoint256, hints: &mut Vec<u64>) {
    let p1_local: [u64; 8] =
        [p1.x[0], p1.x[1], p1.x[2], p1.x[3], p1.y[0], p1.y[1], p1.y[2], p1.y[3]];
    let p2_local: [u64; 8] =
        [p2.x[0], p2.x[1], p2.x[2], p2.x[3], p2.y[0], p2.y[1], p2.y[2], p2.y[3]];
    let mut p3 = [0u64; 8];
    precompiles_helpers::secp256k1_add(&p1_local, &p2_local, &mut p3);
    p1.x = p3[0..4].try_into().unwrap();
    p1.y = p3[4..8].try_into().unwrap();
    hints.extend_from_slice(&p3);
}

/// Given a point `p1`, performs the point doubling `2·p1` and assigns the result to `p1`.
/// It assumes that `p1` is from the Secp256k1 curve and that `p1 != 𝒪`
///
/// Note: We don't need to assume that 2·p1 != 𝒪 because there are not points of order 2 on the Secp256k1 curve
fn double_point_assign(p1: &mut SyscallPoint256, hints: &mut Vec<u64>) {
    let p: [u64; 8] = [p1.x[0], p1.x[1], p1.x[2], p1.x[3], p1.y[0], p1.y[1], p1.y[2], p1.y[3]];
    let mut p3 = [0u64; 8];
    precompiles_helpers::secp256k1_dbl(&p, &mut p3);
    p1.x = p3[0..4].try_into().unwrap();
    p1.y = p3[4..8].try_into().unwrap();
    hints.extend_from_slice(&p3);
}

/// Given points `p1` and `p2`, performs the point addition `p1 + p2` and assigns the result to `p1`.
/// It assumes that `p1` and `p2` are from the Secp256k1 curve, that `p2 != 𝒪`
fn add_points_complete_assign(
    p1: &mut SyscallPoint256,
    p1_is_infinity: &mut bool,
    p2: &SyscallPoint256,
    hints: &mut Vec<u64>,
) {
    if p1.x != p2.x {
        add_points_assign(p1, p2, hints);
    } else if p1.y == p2.y {
        double_point_assign(p1, hints);
    } else {
        *p1_is_infinity = true;
    }
}

/// Given a point `p` and scalars `k1` and `k2`, computes the double scalar multiplication `k1·G + k2·p`
/// It assumes that `k1,k2 ∈ [1, N-1]` and that `p != 𝒪`
pub fn secp256k1_double_scalar_mul_with_g(
    k1: &[u64; 4],
    k2: &[u64; 4],
    p: &SyscallPoint256,
    hints: &mut Vec<u64>,
) -> (bool, SyscallPoint256) {
    // Start by precomputing g + p
    let mut gp = SyscallPoint256 { x: G_X, y: G_Y };
    let mut gp_is_infinity = false;
    add_points_complete_assign(&mut gp, &mut gp_is_infinity, p, hints);

    // Hint the maximum length between the binary representations of k1 and k2
    // We will verify the output by recomposing both k1 and k2
    // Moreover, we should check that the first received bit (of either k1 or k2) is 1
    let (max_limb, max_bit) = msb_pos_256(k1, k2);

    // Perform the loop, based on the binary representation of k1 and k2
    // Start at 𝒪
    let mut res = SyscallPoint256 { x: [0u64; 4], y: [0u64; 4] };
    let mut res_is_infinity = true;
    let mut k1_rec = [0u64; 4];
    let mut k2_rec = [0u64; 4];
    // We do the first iteration separately
    let _max_limb = max_limb as usize;
    let k1_bit = (k1[_max_limb] >> max_bit) & 1;
    let k2_bit = (k2[_max_limb] >> max_bit) & 1;
    assert!(k1_bit == 1 || k2_bit == 1); // At least one of the scalars should start with 1
    if (k1_bit == 0) && (k2_bit == 1) {
        // If res is 𝒪, set res = p; otherwise, double res and add p
        if res_is_infinity {
            res.x = p.x;
            res.y = p.y;
            res_is_infinity = false;
        } else {
            double_point_assign(&mut res, hints);
            add_points_complete_assign(&mut res, &mut res_is_infinity, p, hints);
        }

        // Update k2_rec
        k2_rec[_max_limb] |= 1 << max_bit;
    } else if (k1_bit == 1) && (k2_bit == 0) {
        // If res is 𝒪, set res = g; otherwise, double res and add g
        if res_is_infinity {
            res.x = G_X;
            res.y = G_Y;
            res_is_infinity = false;
        } else {
            double_point_assign(&mut res, hints);
            add_points_complete_assign(
                &mut res,
                &mut res_is_infinity,
                &SyscallPoint256 { x: G_X, y: G_Y },
                hints,
            );
        }

        // Update k1_rec
        k1_rec[_max_limb] |= 1 << max_bit;
    } else if (k1_bit == 1) && (k2_bit == 1) {
        if res_is_infinity {
            // If (g + p) is 𝒪, do nothing; otherwise set res = (g + p)
            if !gp_is_infinity {
                res.x = gp.x;
                res.y = gp.y;
                res_is_infinity = false;
            }
        } else {
            // If (g + p) is 𝒪, simply double res; otherwise double res and add (g + p)
            double_point_assign(&mut res, hints);
            if !gp_is_infinity {
                add_points_complete_assign(&mut res, &mut res_is_infinity, &gp, hints);
            }
        }

        // Update k1_rec and k2_rec
        k1_rec[_max_limb] |= 1 << max_bit;
        k2_rec[_max_limb] |= 1 << max_bit;
    }

    // Perform the rest of the loop
    for i in (0..=max_limb).rev() {
        let _i = i as usize;
        let bit_len = if i == max_limb { max_bit - 1 } else { 63 };
        for j in (0..=bit_len).rev() {
            let k1_bit = (k1[_i] >> j) & 1;
            let k2_bit = (k2[_i] >> j) & 1;

            if (k1_bit == 0) && (k2_bit == 0) {
                // If res is 𝒪, do nothing; otherwise, double
                if !res_is_infinity {
                    double_point_assign(&mut res, hints);
                }
            } else if (k1_bit == 0) && (k2_bit == 1) {
                // If res is 𝒪, set res = p; otherwise, double res and add p
                if res_is_infinity {
                    res.x = p.x;
                    res.y = p.y;
                    res_is_infinity = false;
                } else {
                    double_point_assign(&mut res, hints);
                    add_points_complete_assign(&mut res, &mut res_is_infinity, p, hints);
                }

                // Update k2_rec
                k2_rec[_i] |= 1 << j;
            } else if (k1_bit == 1) && (k2_bit == 0) {
                // If res is 𝒪, set res = g; otherwise, double res and add g
                if res_is_infinity {
                    res.x = G_X;
                    res.y = G_Y;
                    res_is_infinity = false;
                } else {
                    double_point_assign(&mut res, hints);
                    add_points_complete_assign(
                        &mut res,
                        &mut res_is_infinity,
                        &SyscallPoint256 { x: G_X, y: G_Y },
                        hints,
                    );
                }

                // Update k1_rec
                k1_rec[_i] |= 1 << j;
            } else if (k1_bit == 1) && (k2_bit == 1) {
                if res_is_infinity {
                    // If (g + p) is 𝒪, do nothing; otherwise set res = (g + p)
                    if !gp_is_infinity {
                        res.x = gp.x;
                        res.y = gp.y;
                        res_is_infinity = false;
                    }
                } else {
                    // If (g + p) is 𝒪, simply double res; otherwise double res and add (g + p)
                    double_point_assign(&mut res, hints);
                    if !gp_is_infinity {
                        add_points_complete_assign(&mut res, &mut res_is_infinity, &gp, hints);
                    }
                }

                // Update k1_rec and k2_rec
                k1_rec[_i] |= 1 << j;
                k2_rec[_i] |= 1 << j;
            }
        }
    }

    // Check that the recomposed scalars are the same as the received scalars
    assert_eq!(k1_rec, *k1);
    assert_eq!(k2_rec, *k2);

    (res_is_infinity, res)
}

pub fn secp256k1_ecdsa_verify(
    pk: &SyscallPoint256,
    z: &[u64; 4],
    r: &[u64; 4],
    s: &[u64; 4],
    hints: &mut Vec<u64>,
) -> bool {
    let s_inv: &mut [u64; 4] = &mut [0; 4];
    secp256k1_fn_inv(s, s_inv, hints);

    let u1: &mut [u64; 4] = &mut [0; 4];
    secp256k1_fn_mul(z, &s_inv, u1, hints);
    let u2: &mut [u64; 4] = &mut [0; 4];
    secp256k1_fn_mul(r, &s_inv, u2, hints);

    let (is_infinity, res) = secp256k1_double_scalar_mul_with_g(&u1, &u2, pk, hints);
    if is_infinity {
        return false;
    }

    eq(&secp256k1_fn_reduce(&res.x, hints), r)
}
