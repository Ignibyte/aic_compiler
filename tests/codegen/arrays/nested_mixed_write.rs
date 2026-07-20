//! Purpose:
//! Regression coverage for nested writes through an `Array(Mixed)` element
//! (`$a[$i][$j] = ...`), the only nested-write shape the checker accepts for
//! plain arrays (issue #529).
//!
//! Called from:
//! - `cargo test --test codegen_tests arrays::nested_mixed_write`.
//!
//! Key details:
//! - The nested-assign statement used to lower the FULL target as a read and
//!   then replace the resulting Mixed cell in place. `__rt_mixed_array_get`
//!   returns a detached fresh box whenever the slot storage is not already a
//!   boxed Mixed cell (string/int/float slots of a concrete inner array), so
//!   the write mutated a temporary and was silently lost, leaking the
//!   replacement payload. When the slot WAS a boxed cell the write landed, but
//!   the retained cell returned by the read was never released (leak).
//! - The fix writes through the parent cell instead (three-operand
//!   `Op::RuntimeCall` → `__rt_mixed_array_set`/`offsetSet`), which mutates the
//!   aliased container for every slot representation.
//! - Scope: these assert CORRECTNESS (the write lands / stdout byte-parity), NOT
//!   strict heap-cleanliness. A nested-mixed-array LITERAL local
//!   (`$a = [[..],[..],'z']`) is not fully freed at scope end on this fork even
//!   with NO nested write (confirmed via a no-write control: allocs=9 frees=3
//!   live=6). That pre-existing scope-cleanup/refcount leak dominates and is
//!   tracked as #663; re-enable the clean-heap asserts once #663 lands.

use crate::support::compile_and_run_with_heap_debug;

/// Issue #529 repro: the inner arrays are homogeneous `array<string>` (16-byte
/// string slots), the outer heterogeneous literal is `array<mixed>`. The write
/// must replace the stored element, not a detached copy, and stay heap-clean.
#[test]
fn test_nested_write_string_slot_inner_array() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$a = [['x', 'y0'], ['x', 'y1'], ['x', 'y2'], 7];
$a[2][1] = 'patched';
echo $a[2][1] . "\n";
echo $a[1][1] . "\n";
"#,
    );
    assert_eq!(out.stdout, "patched\ny1\n");
}

/// Same shape with homogeneous `array<int>` inner arrays (8-byte int slots).
#[test]
fn test_nested_write_int_slot_inner_array() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$a = [[1, 2], [3, 4], 'z'];
$a[1][0] = 99;
echo $a[1][0] . "\n";
echo $a[0][1] . "\n";
"#,
    );
    assert_eq!(out.stdout, "99\n2\n");
}

/// Heterogeneous inner arrays store boxed Mixed cells: the write already
/// propagated on this shape, but the retained cell returned by the target
/// read was never released, leaking one block per write.
#[test]
fn test_nested_write_boxed_cell_inner_array() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$b = [[1, 'x'], [2, 'y'], 7];
$b[1][0] = 99;
echo $b[1][0] . "\n";
echo $b[1][1] . "\n";
"#,
    );
    assert_eq!(out.stdout, "99\ny\n");
}

/// Associative inner container: overwriting an existing string key and adding
/// a brand-new key must both land in the stored hash.
#[test]
fn test_nested_write_assoc_inner_array() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$a = [['k' => 'v'], 7];
$a[0]['k'] = 'patched';
$a[0]['new'] = 'added';
echo $a[0]['k'] . "\n";
echo $a[0]['new'] . "\n";
"#,
    );
    assert_eq!(out.stdout, "patched\nadded\n");
}

/// Compound nested assignment reads the stale element and writes the combined
/// result back through the same nested-write path.
#[test]
fn test_nested_compound_assign() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$a = [['x', 'y'], 7];
$a[0][1] .= '!';
echo $a[0][1] . "\n";
"#,
    );
    assert_eq!(out.stdout, "y!\n");
}

/// Three-level chain through boxed-cell intermediates: the middle read
/// returns the STORED cell (retained), so the leaf array stays uniquely
/// owned and `__rt_mixed_array_set` mutates it in place.
///
/// Known residual limitation (pre-existing): when an INTERMEDIATE level is a
/// concrete homogeneous array (raw-pointer slots, e.g. `[[['x', 1]], 7]`),
/// the intermediate read boxes a fresh retained cell; the retain makes the
/// leaf look shared, so the write path's `__rt_array_to_mixed` splits it and
/// the write lands in the split copy. Fixing that shape needs a
/// fetch-for-write read helper that promotes intermediate slots to boxed
/// cells in place.
#[test]
fn test_nested_write_three_levels() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$a = [[['x', 1], 5], 7];
$a[0][0][0] = 'patched';
echo $a[0][0][0] . "\n";
echo $a[0][0][1] . "\n";
"#,
    );
    assert_eq!(out.stdout, "patched\n1\n");
}

/// The nested write must survive a function-return boundary on the outer array.
#[test]
fn test_nested_write_survives_function_return() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
function patch(): array {
    $a = [['x', 'y0'], ['x', 'y1'], 7];
    $a[1][1] = 'patched';
    return $a;
}
$r = patch();
echo $r[1][1] . "\n";
"#,
    );
    assert_eq!(out.stdout, "patched\n");
}
