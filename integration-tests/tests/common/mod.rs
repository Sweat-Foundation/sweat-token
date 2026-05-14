// Shared setup, helpers, and traits used by the integration-test binaries.
//
// Each `tests/*.rs` is its own test binary; Rust's dead-code lint runs per
// binary and flags items that are only used by *other* binaries. Silence the
// noise at the module root rather than annotating every helper.
#![allow(dead_code)]

pub mod helpers;
pub mod panic;
pub mod prepare;
pub mod storage;
