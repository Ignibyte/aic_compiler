//! Purpose:
//! Regression tests for a local read after a loop that another scalar type reaches on a path the
//! end of the loop body does not show: a `break` or `continue` out of an `if` arm, a `try` arm, or a
//! loop that runs zero times.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Lowering typed the read from the body's last flow-sensitive type, so a Mixed slot holding a
//!   float unboxed as `int(2)`, and a zero-iteration seed of `1` read as `float(1)`.
//! - Expected output is what PHP 8.5 prints for each fixture.

use super::*;

/// Verifies a float stored by an `if` arm that breaks out of `for` and `foreach` loops keeps its type.
#[test]
fn test_break_arm_float_reaches_read_after_loop() {
    let out = compile_and_run(
        r#"<?php
function viaFor(int $hit): void {
    $x = 1;
    for ($i = 0; $i < 3; $i++) {
        if ($i === $hit) {
            $x = 2.5;
            break;
        }
    }
    var_dump($x);
}
function viaForeach(): void {
    $x = 1;
    foreach ([1, 2] as $i) {
        if ($i === 1) {
            $x = 2.5;
            break;
        }
        $x = 3;
    }
    var_dump($x);
}
viaFor(1);
viaFor(9);
viaForeach();
$top = 1;
foreach ([1, 2, 3] as $i) {
    if ($i === 2) {
        $top = 2.5;
        break;
    }
}
var_dump($top);
"#,
    );
    assert_eq!(out, "float(2.5)\nint(1)\nfloat(2.5)\nfloat(2.5)\n");
}

/// Verifies an integer stored by a breaking arm is not read as the float the loop entered with.
#[test]
fn test_break_arm_int_reaches_float_local_after_loop() {
    let out = compile_and_run(
        r#"<?php
function f(): void {
    $x = 1.0;
    foreach ([1, 2] as $i) {
        if ($i === 1) {
            $x = 7;
            break;
        }
    }
    var_dump($x);
}
f();
"#,
    );
    assert_eq!(out, "int(7)\n");
}

/// Verifies a `continue` arm's float survives to the read after the loop.
#[test]
fn test_continue_arm_float_reaches_read_after_loop() {
    let out = compile_and_run(
        r#"<?php
function f(int $hit): void {
    $x = 1;
    for ($i = 0; $i < 3; $i++) {
        if ($i === $hit) {
            $x = 2.5;
            continue;
        }
        $x = 4;
    }
    var_dump($x);
}
f(2);
f(9);
"#,
    );
    assert_eq!(out, "float(2.5)\nint(4)\n");
}

/// Verifies a loop that runs zero times leaves the integer seed an integer.
#[test]
fn test_zero_iteration_loops_keep_integer_seed() {
    let out = compile_and_run(
        r#"<?php
function viaForeach(array $items): void {
    $x = 1;
    foreach ($items as $item) {
        $x = 2.5;
    }
    var_dump($x);
}
function viaFor(int $n): void {
    $x = 1;
    for ($i = 0; $i < $n; $i++) {
        $x = 2.5;
    }
    var_dump($x);
}
function viaWhile(int $n): void {
    $x = 1;
    $i = 0;
    while ($i < $n) {
        $x = 2.5;
        $i++;
    }
    var_dump($x);
}
viaForeach([]);
viaForeach([1]);
viaFor(0);
viaFor(1);
viaWhile(0);
viaWhile(1);
"#,
    );
    assert_eq!(out, "int(1)\nfloat(2.5)\nint(1)\nfloat(2.5)\nint(1)\nfloat(2.5)\n");
}

/// Verifies an integer stored in a `try` arm that breaks is not read as the float seed.
#[test]
fn test_try_break_arm_int_reaches_read_after_loop() {
    let out = compile_and_run(
        r#"<?php
$x = 1.5;
foreach ([1, 2] as $i) {
    try {
        if ($i === 1) {
            $x = 3;
            break;
        }
    } catch (Exception $e) {
        $x = 2.5;
    }
}
var_dump($x);
"#,
    );
    assert_eq!(out, "int(3)\n");
}

/// Verifies the Mixed reads the fix selects leave the heap clean across repeated calls.
#[test]
fn test_loop_exit_scalar_reads_heap_clean() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
function f(int $hit): float|int {
    $x = 1;
    foreach ([1, 2, 3] as $i) {
        if ($i === $hit) {
            $x = $i + 0.5;
            break;
        }
    }
    return $x;
}
$sum = 0;
for ($n = 0; $n < 6; $n++) {
    $sum += f($n);
}
echo $sum, "\n";
"#,
    );
    assert_eq!(out.stdout, "10.5\n");
    assert!(
        out.stderr.contains("HEAP DEBUG: leak summary: clean"),
        "expected clean heap summary, got: {}",
        out.stderr
    );
}
