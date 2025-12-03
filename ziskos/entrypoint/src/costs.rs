#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
use core::arch::asm;

const MAX_TAG_ID: u16 = 256;

/// Marks the start of a cost measurement region
///
/// # Arguments
/// * `TAG_ID` - Unique identifier for the cost region (must fit in 12-bit immediate)
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_start<const TAG_ID: u16>() {
    const { assert!(TAG_ID < MAX_TAG_ID, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, x1, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_start<const TAG_ID: u16>() {
    const { assert!(TAG_ID < MAX_TAG_ID, "TAG_ID must be less than 4096 (12-bit immediate)") };
}

/// Marks the end of a cost measurement region
///
/// # Arguments
/// * `TAG_ID` - Unique identifier for the cost region (must match the start tag)
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_end<const TAG_ID: u16>() {
    const { assert!(TAG_ID < MAX_TAG_ID, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, x2, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_end<const TAG_ID: u16>() {
    const { assert!(TAG_ID < MAX_TAG_ID, "TAG_ID must be less than 4096 (12-bit immediate)") };
}

/// Records an absolute cost measurement
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_absolute<const TAG_ID: u16>() {
    const { assert!(TAG_ID < MAX_TAG_ID, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, x3, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_absolute<const TAG_ID: u16>() {
    const { assert!(TAG_ID < MAX_TAG_ID, "TAG_ID must be less than MAX_TAG_ID (12-bit immediate)") };
}

/// Records a relative cost measurement
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_relative<const TAG_ID: u16>() {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, x4, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_relative<const TAG_ID: u16>() {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
}

/// Reset relative cost measurement
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_reset_relative<const TAG_ID: u16>() {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, x5, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_reset_relative<const TAG_ID: u16>() {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
}
/*

/// Cost arguments
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(never)]
pub fn ziskos_cost_argument<const TAG_ID: u16>(a: u64) {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, a0, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_argument<const TAG_ID: u16>(a: u64) {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
}

/// Cost arguments
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_2_arguments<const TAG_ID: u16>(a: u64, b: u64) {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, {}, {}", in(reg) a, const TAG_ID, options(nomem, nostack));
        asm!("addi x0, {}, {}", in(reg) b, const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_2_arguments<const TAG_ID: u16>(a: u64, b: u64) {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
}

/// Cost arguments
#[cfg(all(target_os = "zkvm", target_vendor = "zisk"))]
#[inline(always)]
pub fn ziskos_cost_3_arguments<const TAG_ID: u16>(a: u64, b: u64, c: u64) {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
    unsafe {
        asm!("addi x0, x5, {}", const TAG_ID, options(nomem, nostack));
    }
}

#[cfg(not(all(target_os = "zkvm", target_vendor = "zisk")))]
#[inline(always)]
pub fn ziskos_cost_3_arguments<const TAG_ID: u16>(_a: u64, _b: u64, _c: u64) {
    const { assert!(TAG_ID < 4096, "TAG_ID must be less than 4096 (12-bit immediate)") };
}
*/
