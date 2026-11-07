//! Purpose:
//! Heap-balance regressions for overwriting a local that only a conditional arm wrote, at a point
//! where the merge did not split the following statements per arm.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The overwrite skipped releasing the old object because the slot was not definitely
//!   initialized, leaking it whenever the arm had run.
//! - Each fixture runs both paths (arm taken and not taken) and requires a clean leak summary.

use crate::support::*;

/// Asserts the fixture prints `expected` and leaves no live heap blocks.
fn assert_heap_clean(source: &str, expected: &str) {
    let out = compile_and_run_with_heap_debug(source);
    assert_eq!(out.stdout, expected);
    assert!(
        out.stderr.contains("HEAP DEBUG: leak summary: clean"),
        "expected clean heap summary, got: {}",
        out.stderr
    );
}

/// Verifies an object an `if` arm stored is released when an `isset` read keeps the merge joined.
#[test]
fn test_overwrite_after_isset_releases_arm_object() {
    assert_heap_clean(
        r#"<?php
final class Box { public function __construct(public string $s) {} }
function f(bool $c): void {
    if ($c) {
        $e = new Box('first');
    }
    if (isset($e)) {
        echo "set\n";
    }
    $e = new Box('second');
    echo $e->s, "\n";
}
f(true);
f(false);
"#,
        "set\nsecond\nsecond\n",
    );
}

/// Verifies an object a loop's `if` arm stored is released by the overwrite after the loop.
#[test]
fn test_overwrite_after_loop_releases_arm_object() {
    assert_heap_clean(
        r#"<?php
function f(int $n): void {
    for ($i = 0; $i < $n; $i++) {
        if ($i % 2 === 0) {
            $e = new Exception('e' . $i);
        }
    }
    $e = new LogicException('l');
    echo $e->getMessage(), "\n";
}
f(3);
f(0);
"#,
        "l\nl\n",
    );
}

/// Verifies a catch variable bound inside an `if` is released when a later catch rebinds it.
#[test]
fn test_catch_rebind_releases_arm_exception() {
    assert_heap_clean(
        r#"<?php
function f(bool $c): void {
    if ($c) {
        try {
            throw new Exception('first');
        } catch (Exception $e) {
        }
    }
    try {
        throw new LogicException('second');
    } catch (LogicException $e) {
        echo $e->getMessage(), "\n";
    }
}
f(true);
f(false);
"#,
        "second\nsecond\n",
    );
}

/// Verifies the top-level form of the loop-arm overwrite is heap clean.
#[test]
fn test_top_level_overwrite_after_loop_arm_is_heap_clean() {
    assert_heap_clean(
        r#"<?php
for ($i = 0; $i < 3; $i++) {
    if ($i % 2 === 0) {
        try {
            throw new Exception('e');
        } catch (Exception $e) {
        }
    }
}
try {
    throw new LogicException('l');
} catch (LogicException $e) {
    echo $e->getMessage(), "\n";
}
"#,
        "l\n",
    );
}
