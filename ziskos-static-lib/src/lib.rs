//! Static C library exposing the standard zkvm accelerator interface.
//!
//! This crate builds [`ziskos`] as a C static library, exporting the 19
//! `zkvm_*` functions defined in `zkvm_accelerators.h`.
//!
//! The exported functions are:
//! - `zkvm_keccak256`
//! - `zkvm_sha256`
//! - `zkvm_ripemd160`
//! - `zkvm_modexp`
//! - `zkvm_bn254_g1_add`
//! - `zkvm_bn254_g1_mul`
//! - `zkvm_bn254_pairing`
//! - `zkvm_blake2f`
//! - `zkvm_kzg_point_eval`
//! - `zkvm_bls12_g1_add`
//! - `zkvm_bls12_g1_msm`
//! - `zkvm_bls12_g2_add`
//! - `zkvm_bls12_g2_msm`
//! - `zkvm_bls12_pairing`
//! - `zkvm_bls12_map_fp_to_g1`
//! - `zkvm_bls12_map_fp2_to_g2`
//! - `zkvm_secp256r1_verify`
//! - `zkvm_secp256k1_verify`
//! - `zkvm_secp256k1_ecrecover`

pub use ziskos::zisklib::lib::zkvm_accelerators::*;
