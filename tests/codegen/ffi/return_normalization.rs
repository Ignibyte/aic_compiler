//! Purpose:
//! Regression tests for extern return normalization against C callees that leave unspecified register bits dirty.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The C fixture returns full 64-bit words whose low bits encode the declared C return type, mirroring
//!   what the SysV x86_64 and AAPCS64 ABIs let a callee leave above a narrow return value.
//! - The fixture is assembled into the test program itself, so no shared library or search path is needed.

use super::*;

/// C fixture whose functions are declared in PHP with narrower return types than the word they return.
const DIRTY_RETURN_FIXTURE: &str = r#"
#include <stdint.h>

// Declared as `bool` in PHP: bits 0..7 say false, bit 8 is garbage the ABI allows.
uint64_t elephc_test_dirty_false(void) { return 0x100; }

// Declared as `bool` in PHP: the low byte says true, every other bit is garbage.
uint64_t elephc_test_dirty_true(void) { return 0xffffffffffffff01ull; }
"#;

/// Verifies a C `bool` return keeps only its low byte: an ABI-valid false with dirty upper bits reads as false
/// through `var_dump`, truthiness, identity comparison, and a stored local.
#[test]
fn test_ffi_extern_bool_return_uses_only_the_low_byte() {
    let out = compile_and_run_with_c_fixture(
        r#"<?php
extern function elephc_test_dirty_false(): bool;
extern function elephc_test_dirty_true(): bool;
var_dump(elephc_test_dirty_false());
var_dump(elephc_test_dirty_true());
echo elephc_test_dirty_false() ? "truthy" : "falsy", "\n";
$ok = elephc_test_dirty_false();
var_dump($ok === false);
var_dump(elephc_test_dirty_true() === true);
"#,
        DIRTY_RETURN_FIXTURE,
    );
    assert_eq!(
        out,
        "bool(false)\nbool(true)\nfalsy\nbool(true)\nbool(true)\n"
    );
}
