use crate::zisklib::{eq, fcall_secp256r1_ecdsa_verify, gt, is_zero};

use super::{
    constants::{IDENTITY, N_MINUS_ONE, P_MINUS_ONE},
    curve::{secp256r1_is_on_curve, secp256r1_triple_scalar_mul_with_g},
    scalar::{secp256r1_fn_neg, secp256r1_fn_reduce},
};

/// Verifies the signature (r, s) over the message hash z using the public key pk
/// Returns true if the signature is valid, false otherwise
pub fn secp256r1_ecdsa_verify(
    pk: &[u64; 8],
    z: &[u64; 4],
    r: &[u64; 4],
    s: &[u64; 4],
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> bool {
    // r and s must be in the range [1, n-1]
    if is_zero(r) || gt(r, &N_MINUS_ONE) {
        return false;
    }
    if is_zero(s) || gt(s, &N_MINUS_ONE) {
        return false;
    }

    // pk must not be the identity point
    if eq(pk, &IDENTITY) {
        return false;
    }

    // pk must be a valid curve point
    let pk_x: [u64; 4] = [pk[0], pk[1], pk[2], pk[3]];
    let pk_y: [u64; 4] = [pk[4], pk[5], pk[6], pk[7]];
    if gt(&pk_x, &P_MINUS_ONE) || gt(&pk_y, &P_MINUS_ONE) {
        return false;
    }
    if !secp256r1_is_on_curve(
        pk,
        #[cfg(feature = "hints")]
        hints,
    ) {
        return false;
    }

    // Ecdsa verification computes (x, y) = [s⁻¹·z (mod n)]G + [s⁻¹·r (mod n)]PK
    // and checks that x ≡ r (mod n)
    // We can equivalently hint (x,y), verify that
    //   [z]G + [r]PK + [-s](x,y) == 𝒪,
    // and ensure that x ≡ r (mod n), saving us from expensive fn arithmetic

    // Hint the result
    let point = fcall_secp256r1_ecdsa_verify(
        pk,
        z,
        r,
        s,
        #[cfg(feature = "hints")]
        hints,
    );

    // Check the recovered point is valid
    // Note: Identity point would be raised here
    if !secp256r1_is_on_curve(
        &point,
        #[cfg(feature = "hints")]
        hints,
    ) {
        return false;
    }

    // Check that [z]G + [r]PK + [-s](x,y) == 𝒪
    let neg_s = secp256r1_fn_neg(
        s,
        #[cfg(feature = "hints")]
        hints,
    );
    if secp256r1_triple_scalar_mul_with_g(
        z,
        r,
        &neg_s,
        pk,
        &point,
        #[cfg(feature = "hints")]
        hints,
    )
    .is_some()
    {
        return false;
    }

    // Check that x ≡ r (mod n)
    let point_x: [u64; 4] = [point[0], point[1], point[2], point[3]];
    eq(
        &secp256r1_fn_reduce(
            &point_x,
            #[cfg(feature = "hints")]
            hints,
        ),
        r,
    )
}

// ==================== C FFI Functions ====================

/// # Safety
/// - `msg_ptr` must point to 4 u64s
/// - `sig_ptr` must point to 8 u64s
/// - `pk_ptr` must point to 8 u64s
///
/// Returns true if signature is valid
#[cfg_attr(not(feature = "hints"), no_mangle)]
#[cfg_attr(feature = "hints", export_name = "hints_secp256r1_ecdsa_verify_c")]
pub unsafe extern "C" fn secp256r1_ecdsa_verify_c(
    msg_ptr: *const u64,
    sig_ptr: *const u64,
    pk_ptr: *const u64,
    #[cfg(feature = "hints")] hints: &mut Vec<u64>,
) -> bool {
    let msg: &[u64; 4] = &*(msg_ptr as *const [u64; 4]);
    let sig: &[u64; 8] = &*(sig_ptr as *const [u64; 8]);
    let pk: &[u64; 8] = &*(pk_ptr as *const [u64; 8]);
    let r: &[u64; 4] = &sig[0..4].try_into().unwrap();
    let s: &[u64; 4] = &sig[4..8].try_into().unwrap();
    secp256r1_ecdsa_verify(
        pk,
        msg,
        r,
        s,
        #[cfg(feature = "hints")]
        hints,
    )
}
