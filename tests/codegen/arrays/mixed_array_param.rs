//! Purpose:
//! Regression tests (#650) for passing a value whose static type is `mixed` into a
//! parameter declared `array`. The argument-marshalling path did not unbox the boxed
//! `Mixed` cell, so the callee received the cell pointer and read the Mixed tag word as
//! the array header — `count()` returned the tag (always 4), `foreach` aborted on the
//! cell's heap kind, and `implode` produced empty elements. The fix unboxes the cell to
//! the underlying array/hash pointer at the call boundary, validating the runtime tag.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - A bare `array` hint is `Array(Mixed)` (elephc reads no phpdoc generic), so these
//!   arms fire for the common `function f(array $x)` shape when the argument's static
//!   type is `mixed`. `opaque(mixed $x): mixed` defeats call-site specialization so the
//!   value genuinely crosses the `mixed -> array` boundary.

use crate::support::*;

/// The canonical #650 proof: `count()` over a Mixed-passed `array` parameter must return
/// the element count, not the boxed Mixed tag. Pre-fix this returned `4,4,4,4` (mixed tag
/// 4 read from cell offset 0); it must return the real lengths.
#[test]
fn test_mixed_passed_to_array_param_count() {
    let out = compile_and_run(
        r#"<?php
function opaque(mixed $x): mixed { return $x; }
function n(array $xs): int { return count($xs); }
echo n(opaque([1])), ",", n(opaque([1, 2])), ",", n(opaque([1, 2, 3])), ",", n(opaque([1, 2, 3, 4, 5]));
"#,
    );
    assert_eq!(out, "1,2,3,5");
}

/// `foreach` and `implode` over a Mixed-passed `array` parameter must read the real
/// elements. Pre-fix foreach aborted with "unsupported kind" and implode emitted empties.
#[test]
fn test_mixed_passed_to_array_param_foreach_implode() {
    let out = compile_and_run(
        r#"<?php
function opaque(mixed $x): mixed { return $x; }
function j(array $xs): string { $o = ""; foreach ($xs as $v) { $o .= $v . "-"; } return $o; }
function im(array $xs): string { return implode(",", $xs); }
echo j(opaque([1, 3, 2, 5])), "|", im(opaque(["a", "b", "c"]));
"#,
    );
    assert_eq!(out, "1-3-2-5-|a,b,c");
}

/// An associative Mixed source (runtime hash, tag 5) passed to an `array` parameter must
/// unbox to the hash pointer, preserving key/value iteration and count.
#[test]
fn test_mixed_passed_to_array_param_assoc_source() {
    let out = compile_and_run(
        r#"<?php
function opaque(mixed $x): mixed { return $x; }
function ks(array $m): string { $o = ""; foreach ($m as $k => $v) { $o .= "$k=$v;"; } return $o . count($m); }
echo ks(opaque(["a" => 1, "b" => 2, "c" => 3]));
"#,
    );
    assert_eq!(out, "a=1;b=2;c=3;3");
}
