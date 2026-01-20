//! Hints for ZisK Precompiles stream processing
//!
//! This module provides functionality for parsing precompile hints
//! that are received as a stream of `u64` values. Hints are used to provide preprocessed
//! data to precompile operations in the ZisK zkVM.
//!
//! # Hint Format
//!
//! Each hint consists of:
//! - A **header** (`u64`): Contains the hint type (upper 32 bits) and data length (lower 32 bits)
//! - **Data** (`[u64; length]`): The hint payload, where `length` is specified in the header
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                         Header (u64)                        │
//! ├·····························································┤
//! │      Hint Code (32 bits)           Length (32 bits).        │
//! ├─────────────────────────────────────────────────────────────┤
//! │                        Data[0] (u64)                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │                        Data[1] (u64)                        │
//! ├─────────────────────────────────────────────────────────────┤
//! │                             ...                             │
//! ├─────────────────────────────────────────────────────────────┤
//! │                     Data[length-1] (u64)                    │
//! └─────────────────────────────────────────────────────────────┘
//!
//! - Hint Code — Control code or Data Hint Type
//! - Length — Number of following u64 data words
//!
//! ## Hint Type Layout
//!
//! ### Control codes
//!
//! The following control codes are defined:
//! - `0x00` (START): Reset processor state and global sequence.
//! - `0x01` (END): Wait until completion of all pending hints.
//! - `0x02` (CANCEL): Cancel current stream and stop processing further hints.
//! - `0x03` (ERROR): Indicate an error has occurred; stop processing further hints.
//!
//! Control codes are for control only and do not have any associated data (Length should be zero).
//!
//! ### Data Hint Types:
//! - `0x04` (`Noop`): Pass-through data
//! - `0x05` (`EcRecover`): ECRECOVER inputs (currently returns empty)
//! ```

use std::fmt::Display;

use anyhow::Result;

// === CONTROL CODES ===
const CTRL_START: u32 = 0x00;
const CTRL_END: u32 = 0x01;
const CTRL_CANCEL: u32 = 0x02;
const CTRL_ERROR: u32 = 0x03;

// === BUILT-IN HINT CODES ===
// Noop hint code
const HINT_NOOP: u32 = 0x04;

// Secp256k1 Scalar hint codes
const HINT_SECP256K1_FN_REDUCE: u32 = 0x02000;
const HINT_SECP256K1_FN_ADD: u32 = 0x02001;
const HINT_SECP256K1_FN_NEG: u32 = 0x02002;
const HINT_SECP256K1_FN_SUB: u32 = 0x02003;
const HINT_SECP256K1_FN_MUL: u32 = 0x02004;
const HINT_SECP256K1_FN_INV: u32 = 0x02005;
// Secp256k1 Field hint codes
const HINT_SECP256K1_FP_REDUCE: u32 = 0x02010;
const HINT_SECP256K1_FP_ADD: u32 = 0x02011;
const HINT_SECP256K1_FP_NEGATE: u32 = 0x02012;
const HINT_SECP256K1_FP_MUL: u32 = 0x02013;
const HINT_SECP256K1_FP_MUL_SCALAR: u32 = 0x02014;
// Secp256k1 Curve hint codes
const HINT_SECP256K1_TO_AFFINE: u32 = 0x02020;
const HINT_SECP256K1_DECOMPRESS: u32 = 0x02021;
const HINT_SECP256K1_DOUBLE_SCALAR_MUL_WITH_G: u32 = 0x02022;
const HINT_SECP256K1_ECDSA_VERIFY: u32 = 0x02023;

// Big integer arithmetic hint codes
const HINT_REDMOD256: u32 = 0x06;
const HINT_ADDMOD256: u32 = 0x07;
const HINT_MULMOD256: u32 = 0x08;
const HINT_DIVREM256: u32 = 0x09;
const HINT_WPOW256: u32 = 0x0A;
const HINT_OMUL256: u32 = 0x0B;
const HINT_WMUL256: u32 = 0x0C;

// Modular exponentiation hint code
const HINT_MODEXP: u32 = 0x0D;

// BN254 hint codes
const HINT_BN254_IS_ON_CURVE: u32 = 0x0E;
const HINT_BN254_TO_AFFINE: u32 = 0x0F;
const HINT_BN254_ADD: u32 = 0x10;
const HINT_BN254_MUL: u32 = 0x11;
const HINT_BN254_TO_AFFINE_TWIST: u32 = 0x12;
const HINT_BN254_IS_ON_CURVE_TWIST: u32 = 0x13;
const HINT_BN254_IS_ON_SUBGROUP_TWIST: u32 = 0x14;
const HINT_BN254_PAIRING_BATCH: u32 = 0x15;

// BLS12-381 hint codes
const HINT_BLS12_381_MUL_FP12: u32 = 0x16;
const HINT_BLS12_381_DECOMPRESS: u32 = 0x17;
const HINT_BLS12_381_IS_ON_CURVE: u32 = 0x18;
const HINT_BLS12_381_IS_ON_SUBGROUP: u32 = 0x19;
const HINT_BLS12_381_ADD: u32 = 0x1A;
const HINT_BLS12_381_SCALAR_MUL: u32 = 0x1B;
const HINT_BLS12_381_DECOMPRESS_TWIST: u32 = 0x1C;
const HINT_BLS12_381_IS_ON_CURVE_TWIST: u32 = 0x1D;
const HINT_BLS12_381_IS_ON_SUBGROUP_TWIST: u32 = 0x1E;
const HINT_BLS12_381_ADD_TWIST: u32 = 0x1F;
const HINT_BLS12_381_SCALAR_MUL_TWIST: u32 = 0x20;
const HINT_BLS12_381_MILLER_LOOP: u32 = 0x21;
const HINT_BLS12_381_FINAL_EXP: u32 = 0x22;

/// Control code variants for stream control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CtrlHint {
    /// Reset processor state and global sequence.
    Start = CTRL_START,
    /// Wait until completion of all pending hints.
    End = CTRL_END,
    /// Cancel current stream and stop processing.
    Cancel = CTRL_CANCEL,
    /// Signal error and stop processing.
    Error = CTRL_ERROR,
}

impl Display for CtrlHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            CtrlHint::Start => "CTRL_START",
            CtrlHint::End => "CTRL_END",
            CtrlHint::Cancel => "CTRL_CANCEL",
            CtrlHint::Error => "CTRL_ERROR",
        };
        write!(f, "{} ({:#x})", name, *self as u32)
    }
}

impl TryFrom<u32> for CtrlHint {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            CTRL_START => Ok(Self::Start),
            CTRL_END => Ok(Self::End),
            CTRL_CANCEL => Ok(Self::Cancel),
            CTRL_ERROR => Ok(Self::Error),
            _ => Err(anyhow::anyhow!("Invalid control code: {:#x}", value)),
        }
    }
}

/// Built-in hint type variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum BuiltInHint {
    /// Pass-through hint type.
    /// When a hint has this type, the processor simply passes through the data
    /// without any additional computation.
    Noop = HINT_NOOP,

    // Secp256k1 hint types.
    /// Secp256k1 scalar field reduction hint type.
    Secp256K1FnReduce = HINT_SECP256K1_FN_REDUCE,
    /// Secp256k1 scalar field addition hint type.
    Secp256K1FnAdd = HINT_SECP256K1_FN_ADD,
    /// Secp256k1 scalar field negation hint type.
    Secp256K1FnNeg = HINT_SECP256K1_FN_NEG,
    /// Secp256k1 scalar field subtraction hint type.
    Secp256K1FnSub = HINT_SECP256K1_FN_SUB,
    /// Secp256k1 scalar field multiplication hint type.
    Secp256K1FnMul = HINT_SECP256K1_FN_MUL,
    /// Secp256k1 scalar field inversion hint type.
    Secp256K1FnInv = HINT_SECP256K1_FN_INV,
    /// Secp256k1 base field reduction hint type.
    Secp256K1FpReduce = HINT_SECP256K1_FP_REDUCE,
    /// Secp256k1 base field addition hint type.
    Secp256K1FpAdd = HINT_SECP256K1_FP_ADD,
    /// Secp256k1 base field negation hint type.
    Secp256K1FpNegate = HINT_SECP256K1_FP_NEGATE,
    /// Secp256k1 base field multiplication hint type.
    Secp256K1FpMul = HINT_SECP256K1_FP_MUL,
    /// Secp256k1 base field scalar multiplication hint type.
    Secp256K1FpMulScalar = HINT_SECP256K1_FP_MUL_SCALAR,
    /// Secp256k1 to affine coordinates hint type.
    Secp256K1ToAffine = HINT_SECP256K1_TO_AFFINE,
    /// Secp256k1 point decompression hint type.
    Secp256K1Decompress = HINT_SECP256K1_DECOMPRESS,
    /// Secp256k1 double scalar multiplication with G hint type.
    Secp256K1DoubleScalarMulWithG = HINT_SECP256K1_DOUBLE_SCALAR_MUL_WITH_G,
    /// Secp256k1 ECDSA verification hint type.
    Secp256K1EcdsaVerify = HINT_SECP256K1_ECDSA_VERIFY,

    // Big Integer Arithmetic Hints
    ///  Modular reduction of a 256-bit integer hint type.
    RedMod256 = HINT_REDMOD256,
    /// Modular addition of 256-bit integers hint type.
    AddMod256 = HINT_ADDMOD256,
    /// Modular multiplication of 256-bit integers hint type.
    MulMod256 = HINT_MULMOD256,
    /// Division and remainder of 256-bit integers hint type.
    DivRem256 = HINT_DIVREM256,
    /// Wrapping exponentiation of 256-bit integers hint type.
    WPow256 = HINT_WPOW256,
    /// Overflowing multiplication of 256-bit integers hint type.
    OMul256 = HINT_OMUL256,
    /// Wrapping multiplication of 256-bit integers hint type.
    WMul256 = HINT_WMUL256,

    /// Modular exponentiation hint type.
    ModExp = HINT_MODEXP,

    // BN254 Precompile Hints
    /// Check if point is on curve hint type for BN254 curve.
    Bn254IsOnCurve = HINT_BN254_IS_ON_CURVE,
    /// Convert to affine coordinates hint type for BN254 curve.
    Bn254ToAffine = HINT_BN254_TO_AFFINE,
    /// Point addition hint type for BN254 curve.
    Bn254Add = HINT_BN254_ADD,
    /// Scalar multiplication hint type for BN254 curve.
    Bn254Mul = HINT_BN254_MUL,
    /// Convert to affine coordinates hint type for BN254 twist.
    Bn254ToAffineTwist = HINT_BN254_TO_AFFINE_TWIST,
    /// Check if point is on curve hint type for BN254 twist.
    Bn254IsOnCurveTwist = HINT_BN254_IS_ON_CURVE_TWIST,
    /// Check if point is in subgroup hint type for BN254 twist.
    Bn254IsOnSubgroupTwist = HINT_BN254_IS_ON_SUBGROUP_TWIST,
    /// Pairing batch computation hint type for BN254 curve.
    Bn254PairingBatch = HINT_BN254_PAIRING_BATCH,

    // BLS12-381 Precompile Hints
    /// Multiplication in Fp12 hint type for BLS12-381 curve.
    Bls12_381MulFp12 = HINT_BLS12_381_MUL_FP12,
    /// Point decompression hint type for BLS12-381 curve.
    Bls12_381Decompress = HINT_BLS12_381_DECOMPRESS,
    /// Check if point is on curve hint type for BLS12-381 curve.
    Bls12_381IsOnCurve = HINT_BLS12_381_IS_ON_CURVE,
    /// Check if point is in subgroup hint type for BLS12-381 curve.
    Bls12_381IsOnSubgroup = HINT_BLS12_381_IS_ON_SUBGROUP,
    /// Point addition hint type for BLS12-381 curve.
    Bls12_381Add = HINT_BLS12_381_ADD,
    /// Scalar multiplication hint type for BLS12-381 curve.
    Bls12_381ScalarMul = HINT_BLS12_381_SCALAR_MUL,
    /// Point decompression hint type for BLS12-381 twist.
    Bls12_381DecompressTwist = HINT_BLS12_381_DECOMPRESS_TWIST,
    /// Check if point is on curve hint type for BLS12-381 twist.
    Bls12_381IsOnCurveTwist = HINT_BLS12_381_IS_ON_CURVE_TWIST,
    /// Check if point is in subgroup hint type for BLS12-381 twist.
    Bls12_381IsOnSubgroupTwist = HINT_BLS12_381_IS_ON_SUBGROUP_TWIST,
    /// Point addition hint type for BLS12-381 twist.
    Bls12_381AddTwist = HINT_BLS12_381_ADD_TWIST,
    /// Scalar multiplication hint type for BLS12-381 twist.
    Bls12_381ScalarMulTwist = HINT_BLS12_381_SCALAR_MUL_TWIST,
    /// Miller loop computation hint type for BLS12-381 curve.
    Bls12_381MillerLoop = HINT_BLS12_381_MILLER_LOOP,
    /// Final exponentiation computation hint type for BLS12-381 curve.
    Bls12_381FinalExp = HINT_BLS12_381_FINAL_EXP,
}

impl Display for BuiltInHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            // Noop Hint
            BuiltInHint::Noop => "NOOP",

            // Secp256k1 Scalar Hints
            BuiltInHint::Secp256K1FnReduce => "SECP256K1_FN_REDUCE",
            BuiltInHint::Secp256K1FnAdd => "SECP256K1_FN_ADD",
            BuiltInHint::Secp256K1FnNeg => "SECP256K1_FN_NEG",
            BuiltInHint::Secp256K1FnSub => "SECP256K1_FN_SUB",
            BuiltInHint::Secp256K1FnMul => "SECP256K1_FN_MUL",
            BuiltInHint::Secp256K1FnInv => "SECP256K1_FN_INV",
            // Secp256k1 Field Hints
            BuiltInHint::Secp256K1FpReduce => "SECP256K1_FP_REDUCE",
            BuiltInHint::Secp256K1FpAdd => "SECP256K1_FP_ADD",
            BuiltInHint::Secp256K1FpNegate => "SECP256K1_FP_NEGATE",
            BuiltInHint::Secp256K1FpMul => "SECP256K1_FP_MUL",
            BuiltInHint::Secp256K1FpMulScalar => "SECP256K1_FP_MUL_SCALAR",
            // Secp256k1 Curve Hints
            BuiltInHint::Secp256K1ToAffine => "SECP256K1_TO_AFFINE",
            BuiltInHint::Secp256K1Decompress => "SECP256K1_DECOMPRESS",
            BuiltInHint::Secp256K1DoubleScalarMulWithG => "SECP256K1_DOUBLE_SCALAR_MUL_WITH_G",
            BuiltInHint::Secp256K1EcdsaVerify => "SECP256K1_ECDSA_VERIFY",

            // Big Integer Arithmetic Hints
            BuiltInHint::RedMod256 => "REDMOD256",
            BuiltInHint::AddMod256 => "ADDMOD256",
            BuiltInHint::MulMod256 => "MULMOD256",
            BuiltInHint::DivRem256 => "DIVREM256",
            BuiltInHint::WPow256 => "WPOW256",
            BuiltInHint::OMul256 => "OMUL256",
            BuiltInHint::WMul256 => "WMUL256",

            // Modular Exponentiation Hint
            BuiltInHint::ModExp => "MODEXP",

            // BN254 Hints
            BuiltInHint::Bn254IsOnCurve => "BN254_IS_ON_CURVE",
            BuiltInHint::Bn254ToAffine => "BN254_TO_AFFINE",
            BuiltInHint::Bn254Add => "BN254_ADD",
            BuiltInHint::Bn254Mul => "BN254_MUL",
            BuiltInHint::Bn254ToAffineTwist => "BN254_TO_AFFINE_TWIST",
            BuiltInHint::Bn254IsOnCurveTwist => "BN254_IS_ON_CURVE_TWIST",
            BuiltInHint::Bn254IsOnSubgroupTwist => "BN254_IS_ON_SUBGROUP_TWIST",
            BuiltInHint::Bn254PairingBatch => "BN254_PAIRING_BATCH",

            // BLS12-381 Hints
            BuiltInHint::Bls12_381MulFp12 => "BLS12_381_MUL_FP12",
            BuiltInHint::Bls12_381Decompress => "BLS12_381_DECOMPRESS",
            BuiltInHint::Bls12_381IsOnCurve => "BLS12_381_IS_ON_CURVE",
            BuiltInHint::Bls12_381IsOnSubgroup => "BLS12_381_IS_ON_SUBGROUP",
            BuiltInHint::Bls12_381Add => "BLS12_381_ADD",
            BuiltInHint::Bls12_381ScalarMul => "BLS12_381_SCALAR_MUL",
            BuiltInHint::Bls12_381DecompressTwist => "BLS12_381_DECOMPRESS_TWIST",
            BuiltInHint::Bls12_381IsOnCurveTwist => "BLS12_381_IS_ON_CURVE_TWIST",
            BuiltInHint::Bls12_381IsOnSubgroupTwist => "BLS12_381_IS_ON_SUBGROUP_TWIST",
            BuiltInHint::Bls12_381AddTwist => "BLS12_381_ADD_TWIST",
            BuiltInHint::Bls12_381ScalarMulTwist => "BLS12_381_SCALAR_MUL_TWIST",
            BuiltInHint::Bls12_381MillerLoop => "BLS12_381_MILLER_LOOP",
            BuiltInHint::Bls12_381FinalExp => "BLS12_381_FINAL_EXP",
        };
        write!(f, "{} ({:#x})", name, *self as u32)
    }
}

impl TryFrom<u32> for BuiltInHint {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        match value {
            // Noop Hint
            HINT_NOOP => Ok(Self::Noop),

            // Secp256K1 Scalar Hints
            HINT_SECP256K1_FN_REDUCE => Ok(Self::Secp256K1FnReduce),
            HINT_SECP256K1_FN_ADD => Ok(Self::Secp256K1FnAdd),
            HINT_SECP256K1_FN_NEG => Ok(Self::Secp256K1FnNeg),
            HINT_SECP256K1_FN_SUB => Ok(Self::Secp256K1FnSub),
            HINT_SECP256K1_FN_MUL => Ok(Self::Secp256K1FnMul),
            HINT_SECP256K1_FN_INV => Ok(Self::Secp256K1FnInv),
            // Secp256k1 Field Hints
            HINT_SECP256K1_FP_REDUCE => Ok(Self::Secp256K1FpReduce),
            HINT_SECP256K1_FP_ADD => Ok(Self::Secp256K1FpAdd),
            HINT_SECP256K1_FP_NEGATE => Ok(Self::Secp256K1FpNegate),
            HINT_SECP256K1_FP_MUL => Ok(Self::Secp256K1FpMul),
            HINT_SECP256K1_FP_MUL_SCALAR => Ok(Self::Secp256K1FpMulScalar),
            // Secp256k1 Curve Hints
            HINT_SECP256K1_TO_AFFINE => Ok(Self::Secp256K1ToAffine),
            HINT_SECP256K1_DECOMPRESS => Ok(Self::Secp256K1Decompress),
            HINT_SECP256K1_DOUBLE_SCALAR_MUL_WITH_G => Ok(Self::Secp256K1DoubleScalarMulWithG),
            HINT_SECP256K1_ECDSA_VERIFY => Ok(Self::Secp256K1EcdsaVerify),

            // Big Integer Arithmetic Hints
            HINT_REDMOD256 => Ok(Self::RedMod256),
            HINT_ADDMOD256 => Ok(Self::AddMod256),
            HINT_MULMOD256 => Ok(Self::MulMod256),
            HINT_DIVREM256 => Ok(Self::DivRem256),
            HINT_WPOW256 => Ok(Self::WPow256),
            HINT_OMUL256 => Ok(Self::OMul256),
            HINT_WMUL256 => Ok(Self::WMul256),

            // Modular Exponentiation Hint
            HINT_MODEXP => Ok(Self::ModExp),

            // BN254 Hints
            HINT_BN254_IS_ON_CURVE => Ok(Self::Bn254IsOnCurve),
            HINT_BN254_TO_AFFINE => Ok(Self::Bn254ToAffine),
            HINT_BN254_ADD => Ok(Self::Bn254Add),
            HINT_BN254_MUL => Ok(Self::Bn254Mul),
            HINT_BN254_TO_AFFINE_TWIST => Ok(Self::Bn254ToAffineTwist),
            HINT_BN254_IS_ON_CURVE_TWIST => Ok(Self::Bn254IsOnCurveTwist),
            HINT_BN254_IS_ON_SUBGROUP_TWIST => Ok(Self::Bn254IsOnSubgroupTwist),
            HINT_BN254_PAIRING_BATCH => Ok(Self::Bn254PairingBatch),

            // BLS12-381 Hints
            HINT_BLS12_381_MUL_FP12 => Ok(Self::Bls12_381MulFp12),
            HINT_BLS12_381_DECOMPRESS => Ok(Self::Bls12_381Decompress),
            HINT_BLS12_381_IS_ON_CURVE => Ok(Self::Bls12_381IsOnCurve),
            HINT_BLS12_381_IS_ON_SUBGROUP => Ok(Self::Bls12_381IsOnSubgroup),
            HINT_BLS12_381_ADD => Ok(Self::Bls12_381Add),
            HINT_BLS12_381_SCALAR_MUL => Ok(Self::Bls12_381ScalarMul),
            HINT_BLS12_381_DECOMPRESS_TWIST => Ok(Self::Bls12_381DecompressTwist),
            HINT_BLS12_381_IS_ON_CURVE_TWIST => Ok(Self::Bls12_381IsOnCurveTwist),
            HINT_BLS12_381_IS_ON_SUBGROUP_TWIST => Ok(Self::Bls12_381IsOnSubgroupTwist),
            HINT_BLS12_381_ADD_TWIST => Ok(Self::Bls12_381AddTwist),
            HINT_BLS12_381_SCALAR_MUL_TWIST => Ok(Self::Bls12_381ScalarMulTwist),
            HINT_BLS12_381_MILLER_LOOP => Ok(Self::Bls12_381MillerLoop),
            HINT_BLS12_381_FINAL_EXP => Ok(Self::Bls12_381FinalExp),

            _ => Err(anyhow::anyhow!("Invalid built-in hint code: {:#x}", value)),
        }
    }
}

/// Hint code representing either a control code or built-in hint type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum HintCode {
    /// Control code for stream management.
    Ctrl(CtrlHint),
    /// Built-in hint type.
    BuiltIn(BuiltInHint),
    /// Custom hint type
    Custom(u32),
}

impl Display for HintCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HintCode::Ctrl(ctrl) => write!(f, "{}", ctrl),
            HintCode::BuiltIn(builtin) => write!(f, "{}", builtin),
            HintCode::Custom(code) => write!(f, "CUSTOM_HINT_{:#x}", code),
        }
    }
}

impl TryFrom<u32> for HintCode {
    type Error = anyhow::Error;

    fn try_from(value: u32) -> Result<Self> {
        // Try CtrlCode first
        if let Ok(ctrl) = CtrlHint::try_from(value) {
            return Ok(HintCode::Ctrl(ctrl));
        }
        // Try BuiltInHint next
        if let Ok(builtin) = BuiltInHint::try_from(value) {
            return Ok(HintCode::BuiltIn(builtin));
        }
        // Unknown codes return error - custom codes handled separately
        Err(anyhow::anyhow!("Unknown hint code: {:#x}", value))
    }
}

impl HintCode {
    /// Convert HintCode to its u32 discriminant value.
    #[inline]
    pub const fn to_u32(self) -> u32 {
        match self {
            // Control Codes
            HintCode::Ctrl(CtrlHint::Start) => CTRL_START,
            HintCode::Ctrl(CtrlHint::End) => CTRL_END,
            HintCode::Ctrl(CtrlHint::Cancel) => CTRL_CANCEL,
            HintCode::Ctrl(CtrlHint::Error) => CTRL_ERROR,

            // Built-In Hint Codes
            // Noop Hint
            HintCode::BuiltIn(BuiltInHint::Noop) => HINT_NOOP,

            // Secp256K1 Scalar Hint Codes
            HintCode::BuiltIn(BuiltInHint::Secp256K1FnReduce) => HINT_SECP256K1_FN_REDUCE,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FnAdd) => HINT_SECP256K1_FN_ADD,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FnNeg) => HINT_SECP256K1_FN_NEG,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FnSub) => HINT_SECP256K1_FN_SUB,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FnMul) => HINT_SECP256K1_FN_MUL,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FnInv) => HINT_SECP256K1_FN_INV,
            // Secp256k1 Field Hint Codes
            HintCode::BuiltIn(BuiltInHint::Secp256K1FpReduce) => HINT_SECP256K1_FP_REDUCE,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FpAdd) => HINT_SECP256K1_FP_ADD,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FpNegate) => HINT_SECP256K1_FP_NEGATE,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FpMul) => HINT_SECP256K1_FP_MUL,
            HintCode::BuiltIn(BuiltInHint::Secp256K1FpMulScalar) => HINT_SECP256K1_FP_MUL_SCALAR,
            // Secp256k1 Curve Hint Codes
            HintCode::BuiltIn(BuiltInHint::Secp256K1ToAffine) => HINT_SECP256K1_TO_AFFINE,
            HintCode::BuiltIn(BuiltInHint::Secp256K1Decompress) => HINT_SECP256K1_DECOMPRESS,
            HintCode::BuiltIn(BuiltInHint::Secp256K1DoubleScalarMulWithG) => {
                HINT_SECP256K1_DOUBLE_SCALAR_MUL_WITH_G
            }
            HintCode::BuiltIn(BuiltInHint::Secp256K1EcdsaVerify) => HINT_SECP256K1_ECDSA_VERIFY,

            // Big Integer Arithmetic Hints
            HintCode::BuiltIn(BuiltInHint::RedMod256) => HINT_REDMOD256,
            HintCode::BuiltIn(BuiltInHint::AddMod256) => HINT_ADDMOD256,
            HintCode::BuiltIn(BuiltInHint::MulMod256) => HINT_MULMOD256,
            HintCode::BuiltIn(BuiltInHint::DivRem256) => HINT_DIVREM256,
            HintCode::BuiltIn(BuiltInHint::WPow256) => HINT_WPOW256,
            HintCode::BuiltIn(BuiltInHint::OMul256) => HINT_OMUL256,
            HintCode::BuiltIn(BuiltInHint::WMul256) => HINT_WMUL256,

            // Modular Exponentiation Hint
            HintCode::BuiltIn(BuiltInHint::ModExp) => HINT_MODEXP,

            // BN254 Hints
            HintCode::BuiltIn(BuiltInHint::Bn254IsOnCurve) => HINT_BN254_IS_ON_CURVE,
            HintCode::BuiltIn(BuiltInHint::Bn254ToAffine) => HINT_BN254_TO_AFFINE,
            HintCode::BuiltIn(BuiltInHint::Bn254Add) => HINT_BN254_ADD,
            HintCode::BuiltIn(BuiltInHint::Bn254Mul) => HINT_BN254_MUL,
            HintCode::BuiltIn(BuiltInHint::Bn254ToAffineTwist) => HINT_BN254_TO_AFFINE_TWIST,
            HintCode::BuiltIn(BuiltInHint::Bn254IsOnCurveTwist) => HINT_BN254_IS_ON_CURVE_TWIST,
            HintCode::BuiltIn(BuiltInHint::Bn254IsOnSubgroupTwist) => {
                HINT_BN254_IS_ON_SUBGROUP_TWIST
            }
            HintCode::BuiltIn(BuiltInHint::Bn254PairingBatch) => HINT_BN254_PAIRING_BATCH,

            // BLS12-381 Hints
            HintCode::BuiltIn(BuiltInHint::Bls12_381MulFp12) => HINT_BLS12_381_MUL_FP12,
            HintCode::BuiltIn(BuiltInHint::Bls12_381Decompress) => HINT_BLS12_381_DECOMPRESS,
            HintCode::BuiltIn(BuiltInHint::Bls12_381IsOnCurve) => HINT_BLS12_381_IS_ON_CURVE,
            HintCode::BuiltIn(BuiltInHint::Bls12_381IsOnSubgroup) => HINT_BLS12_381_IS_ON_SUBGROUP,
            HintCode::BuiltIn(BuiltInHint::Bls12_381Add) => HINT_BLS12_381_ADD,
            HintCode::BuiltIn(BuiltInHint::Bls12_381ScalarMul) => HINT_BLS12_381_SCALAR_MUL,
            HintCode::BuiltIn(BuiltInHint::Bls12_381DecompressTwist) => {
                HINT_BLS12_381_DECOMPRESS_TWIST
            }
            HintCode::BuiltIn(BuiltInHint::Bls12_381IsOnCurveTwist) => {
                HINT_BLS12_381_IS_ON_CURVE_TWIST
            }
            HintCode::BuiltIn(BuiltInHint::Bls12_381IsOnSubgroupTwist) => {
                HINT_BLS12_381_IS_ON_SUBGROUP_TWIST
            }
            HintCode::BuiltIn(BuiltInHint::Bls12_381AddTwist) => HINT_BLS12_381_ADD_TWIST,
            HintCode::BuiltIn(BuiltInHint::Bls12_381ScalarMulTwist) => {
                HINT_BLS12_381_SCALAR_MUL_TWIST
            }
            HintCode::BuiltIn(BuiltInHint::Bls12_381MillerLoop) => HINT_BLS12_381_MILLER_LOOP,
            HintCode::BuiltIn(BuiltInHint::Bls12_381FinalExp) => HINT_BLS12_381_FINAL_EXP,

            // Custom Hints
            HintCode::Custom(code) => code,
        }
    }
}

/// Represents a single precompile hint parsed from a `u64` slice.
///
/// A hint consists of a type identifier and associated data. The hint type
/// determines how the data should be processed by the [`PrecompileHintsProcessor`].
pub struct PrecompileHint {
    /// The type of hint, determining how the data should be processed.
    pub hint_code: HintCode,
    /// The hint payload data.
    pub data: Vec<u64>,
}

impl std::fmt::Debug for PrecompileHint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data_display = if self.data.len() <= 10 {
            format!("{:?}", self.data)
        } else {
            format!("{:?}... ({} more)", &self.data[..10], self.data.len() - 10)
        };
        f.debug_struct("PrecompileHint")
            .field("hint_type", &self.hint_code)
            .field("data", &data_display)
            .finish()
    }
}

impl PrecompileHint {
    /// Parses a [`PrecompileHint`] from a slice of `u64` values at the given index.
    ///
    /// # Arguments
    ///
    /// * `slice` - The source slice containing concatenated hints
    /// * `idx` - The index where the hint header starts
    /// * `allow_custom` - If true, unknown codes create Custom variant; if false, return error
    ///
    /// # Returns
    ///
    /// * `Ok(PrecompileHint)` - Successfully parsed hint
    /// * `Err` - If the slice is too short or the index is out of bounds
    #[inline(always)]
    pub fn from_u64_slice(slice: &[u64], idx: usize, allow_custom: bool) -> Result<Self> {
        if slice.is_empty() || idx >= slice.len() {
            return Err(anyhow::anyhow!("Slice too short or index out of bounds"));
        }

        let header = slice[idx];
        let length = (header & 0xFFFFFFFF) as u32;

        if slice.len() < idx + length as usize + 1 {
            return Err(anyhow::anyhow!(
                "Slice too short for hint data: expected {}, got {}",
                length,
                slice.len() - idx - 1
            ));
        }

        let hint_code_32 = (header >> 32) as u32;
        let hint_code = if allow_custom {
            HintCode::try_from(hint_code_32).unwrap_or(HintCode::Custom(hint_code_32))
        } else {
            HintCode::try_from(hint_code_32)?
        };

        // Create a new Vec with the hint data.
        let data = slice[idx + 1..idx + length as usize + 1].to_vec();

        Ok(PrecompileHint { hint_code, data })
    }
}
