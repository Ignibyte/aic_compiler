//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of indexed array array shape-transform builtins, including fill, pad, and splice.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Tests `array_fill(start_index, num, value)` — creates a 3-element array indexed from 0,
/// all initialized to 42, then accesses elements via integer index.
#[test]
fn test_array_fill() {
    let out = compile_and_run(
        r#"<?php
$a = array_fill(0, 3, 42);
echo $a[0] . " " . $a[1] . " " . $a[2];
"#,
    );
    assert_eq!(out, "42 42 42");
}

/// Regression: `array_fill` with a STRING value stores every element, not just the first.
/// Strings need 16-byte array slots (pointer + length); the fill previously allocated 8-byte
/// scalar slots, so the 16-byte string writes overflowed and only the first element survived.
/// Routed through the dedicated `__rt_array_fill_str` runtime.
#[test]
fn test_array_fill_string_value() {
    let out = compile_and_run(
        r#"<?php
$x = array_fill(0, 3, "ab");
echo implode(",", $x), "|", count($x), "|", $x[2];
"#,
    );
    assert_eq!(out, "ab,ab,ab|3|ab");
}

/// Tests `array_pad($array, length, value)` — pads `[1, 2]` to length 5 with trailing `0`
/// entries, then verifies the resulting array has exactly 5 elements via `count()`.
#[test]
fn test_array_pad() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2];
$b = array_pad($a, 5, 0);
echo count($b);
"#,
    );
    assert_eq!(out, "5");
}

/// Tests `array_splice(&$array, offset, length)` — removes 2 elements starting at index 1
/// from `[1, 2, 3, 4, 5]`, captures the removed portion, and verifies both counts.
#[test]
fn test_array_splice() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 3, 4, 5];
$removed = array_splice($a, 1, 2);
echo count($removed) . " " . count($a);
"#,
    );
    assert_eq!(out, "2 3");
}

/// Tests `array_combine($keys, $values)` — combines `["a", "b"]` keys with `[1, 2]` values
/// into an associative array, then verifies the resulting array has exactly 2 elements.
#[test]
fn test_array_combine() {
    let out = compile_and_run(
        r#"<?php
$keys = ["a", "b"];
$vals = [1, 2];
$m = array_combine($keys, $vals);
echo count($m);
"#,
    );
    assert_eq!(out, "2");
}

/// Tests that a key/value length mismatch raises a catchable `\ValueError`, even when
/// the combined result is discarded (a foreign-key node's cardinality-check shape).
/// Pre-fix `array_combine` was classified pure-non-throwing, so the optimizer DCE-d the
/// discarded call and pruned the enclosing try — the mismatch branch was unreachable and
/// this returned `"ok|ok"`. The runtime helper now compares lengths and throws before
/// allocating, and the effects change keeps the handler live.
#[test]
fn test_array_combine_length_mismatch_throws_value_error() {
    let out = compile_and_run(
        r#"<?php
function cardinality(array $keys, array $vals): string {
    try {
        array_combine($keys, $vals);
        return "ok";
    } catch (\ValueError $e) {
        return "mismatch";
    }
}
echo cardinality(["a", "b"], [1, 2]) . "|" . cardinality(["a", "b", "c"], [1, 2]);
"#,
    );
    assert_eq!(out, "ok|mismatch");
}

/// Tests `array_flip($array)` — inverts values-to-keys on `[10, 20, 30]`, producing a map
/// with 3 entries. Verifies count only.
#[test]
fn test_array_flip() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
$f = array_flip($a);
echo count($f);
"#,
    );
    assert_eq!(out, "3");
}

/// Tests `array_flip` integer-value key normalization — flips `[10, 20]`, then accesses
/// flipped keys using both integer (`$f[10]`) and string (`$f["20"]`) index forms, verifying
/// PHP's loose-key comparison for integer-like string keys.
#[test]
fn test_array_flip_integer_values_are_integer_keys() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20];
$f = array_flip($a);
echo $f[10] . "|" . $f["20"];
"#,
    );
    assert_eq!(out, "0|1");
}

/// Tests `array_flip` with string values that normalize to the same integer key — flips
/// `["1", "02", "2"]` where "02" and "2" collide under PHP integer-string key normalization,
/// then verifies the resulting count is 3 and each flipped entry is accessible by its
/// canonical integer key.
#[test]
fn test_array_flip_string_values_normalize_numeric_keys() {
    let out = compile_and_run(
        r#"<?php
$a = ["1", "02", "2"];
$f = array_flip($a);
echo count($f) . "|" . $f[1] . "|" . $f["02"] . "|" . $f["2"];
"#,
    );
    assert_eq!(out, "3|0|1|2");
}

/// Tests `array_chunk($array, size)` — splits `[1, 2, 3, 4, 5]` into chunks of size 2,
/// producing 3 chunks. Verifies chunk count via `count()`.
#[test]
fn test_array_chunk() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 3, 4, 5];
$c = array_chunk($a, 2);
echo count($c);
"#,
    );
    assert_eq!(out, "3");
}

/// Tests `array_fill_keys($keys, value)` — creates an array from `["x", "y"]` as keys,
/// both initialized to `0`, then verifies the resulting associative array has exactly 2 entries.
#[test]
fn test_array_fill_keys() {
    let out = compile_and_run(
        r#"<?php
$keys = ["x", "y"];
$m = array_fill_keys($keys, 0);
echo count($m);
"#,
    );
    assert_eq!(out, "2");
}

/// Regression: `array_fill_keys()` over a boxed-`Mixed` key array must apply PHP's own key cast
/// (stringify, then normalize), which is NOT the cast a direct `$a[$k] = $v` assignment uses.
///
/// A bare `array` parameter carries no element type, so real call sites reach `array_fill_keys()`
/// with `Array(Mixed)` key slots. Verified against PHP 8.5.8, the divergent cases are `false`
/// (key `""`, not `0`) and floats (key `"2.7"`, not `2`) — reusing `__rt_array_set_mixed_key`'s
/// tag dispatch, which is correct for direct assignment, would silently mis-key both. The
/// heterogeneous literals below are what give the key arrays their `Mixed` element type.
#[test]
fn test_array_fill_keys_over_boxed_mixed_keys_uses_php_key_cast() {
    let out = compile_and_run(
        r#"<?php
function dump(array $m): string {
    $out = "";
    foreach ($m as $k => $v) {
        $out .= gettype($k) . ":" . $k . ";";
    }
    return $out;
}
echo dump(array_fill_keys(["x", 5, "5", null, true], true)), "|";
echo dump(array_fill_keys([2.7, -2.7, false], true)), "|";
echo dump(array_fill_keys(["7", "008", "-3"], true)), "|";
$active = ["alpha" => 1, "beta" => 2];
echo dump(array_fill_keys(array_keys($active), true)), "|";
$set = array_fill_keys(["editor", "administrator", 7], true);
echo array_key_exists("administrator", $set) ? "y" : "n";
echo array_key_exists("missing", $set) ? "y" : "n";
echo array_key_exists(7, $set) ? "y" : "n";
"#,
    );
    assert_eq!(
        out,
        "string:x;integer:5;string:;integer:1;|string:2.7;string:-2.7;string:;|\
integer:7;string:008;integer:-3;|string:alpha;string:beta;|yny"
    );
}

/// Regression: a scalar fill payload must stay its own static type through the boxed-`Mixed` key
/// path — the helper forwards `(value_lo, value_hi, value_tag)` untouched rather than boxing the
/// value as a Mixed cell, so a strict comparison against the original literal still holds.
#[test]
fn test_array_fill_keys_over_boxed_mixed_keys_keeps_value_type() {
    let out = compile_and_run(
        r#"<?php
$filled = array_fill_keys(["a", 1, null], 42);
echo count($filled), var_export($filled["a"] === 42, true);
"#,
    );
    assert_eq!(out, "3true");
}

/// Regression: `array_fill`/`array_chunk`/`array_pad`/`array_splice` must unbox a `Mixed`/`Union`
/// integer argument (start index, chunk size, target size, offset, length) instead of using the
/// boxed heap pointer as a raw int. Each int arg here is read from a heterogeneous (Mixed-valued)
/// associative array. `array_fill` uses an integer fill value to sidestep an unrelated
/// refcounted-fill limitation with string values.
///
/// **Currently ignored** — pre-existing gap on origin/main: `array_fill($m["n"], 3, 7)` routes
/// through `__rt_array_fill_assoc` (because the start is a non-literal-zero int), which
/// stores every slot as a Mixed cell. `implode` over a Mixed-valued hash segfaults because
/// it does not unbox the per-slot Mixed tag. PHP returns a plain int-keyed array
/// (`[2=>7, 3=>7, 4=>7]`) without any boxing. A proper fix needs `__rt_array_fill_assoc`
/// to store scalar values directly (no Mixed box) and only box refcounted values, plus
/// `implode` to unbox Mixed when iterating over a hash. Tracked as a separate
/// `array-fill-assoc-implode` gap.
#[test]
#[ignore = "pre-existing gap: __rt_array_fill_assoc Mixed-boxing + implode Mixed unbox"]
fn test_shape_transforms_unbox_mixed_int_args() {
    let out = compile_and_run(
        r#"<?php
$m = ["n" => 2, "t" => "x"];
$sz = ["v" => 5, "t" => "y"];
$f = implode(",", array_fill($m["n"], 3, 7));
$c = implode(",", array_chunk([1, 2, 3, 4, 5], $m["n"])[2]);
$p = implode(",", array_pad([1, 2], $sz["v"], 0));
$b = [1, 2, 3, 4];
array_splice($b, $m["n"], $m["n"]);
echo $f, "|", $c, "|", $p, "|", implode(",", $b);
"#,
    );
    assert_eq!(out, "7,7,7|5|1,2,0,0,0|1,2");
}
