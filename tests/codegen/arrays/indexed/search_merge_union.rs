//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of indexed array array search, merge, and union builtins, including search, search not found is strict false, and search assigned not found is strict false.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Verifies `array_search` returns the 0-based integer index of the first match.
#[test]
fn test_array_search() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
echo array_search(20, $a);
"#,
    );
    assert_eq!(out, "1");
}

/// Verifies `array_search` returns strict `false` (===) when the value is absent.
#[test]
fn test_array_search_not_found_is_strict_false() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
echo array_search(99, $a) === false ? "miss" : "hit";
"#,
    );
    assert_eq!(out, "miss");
}

/// Regression: assigning the result of `array_search` to a variable before comparing
/// must still yield strict `false`, not a falsy zero or empty string.
#[test]
fn test_array_search_assigned_not_found_is_strict_false() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
$result = array_search(99, $a);
echo $result === false ? "miss" : "hit";
"#,
    );
    assert_eq!(out, "miss");
}

/// Verifies that `array_search` returns index `0` (not `false`) when the target is
/// at the first position, and that `=== false` correctly distinguishes the two.
#[test]
fn test_array_search_zero_index_is_not_false() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
echo array_search(10, $a) === false ? "miss" : "zero";
"#,
    );
    assert_eq!(out, "zero");
}

/// Verifies `array_key_exists` returns true for an existing integer key and false for a missing key.
#[test]
fn test_array_key_exists() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
if (array_key_exists(1, $a)) { echo "yes"; }
if (!array_key_exists(5, $a)) { echo "no"; }
"#,
    );
    assert_eq!(out, "yesno");
}

/// Verifies `array_merge` concatenates two indexed arrays and preserves all elements.
#[test]
fn test_array_merge() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2];
$b = [3, 4];
$c = array_merge($a, $b);
echo count($c);
echo $c[0] . $c[1] . $c[2] . $c[3];
"#,
    );
    assert_eq!(out, "41234");
}

/// Regression for #648: `array_merge` over `string`-element arrays must copy the 16-byte string
/// descriptors AND persist each into an owned buffer — a raw copy would share the sources'
/// array-owned strings and double-free them. Also exercises a `Mixed` operand (an erased
/// `array<...>` from an array-element access) coerced to the sibling's `Array(Str)` type. Asserts
/// both source arrays remain intact after the merge (the double-free proof).
#[test]
fn test_array_merge_string_elements() {
    let out = compile_and_run(
        r#"<?php
function j(array $a): string { return implode(",", $a); }
$a = ["a", "b"];
$b = ["c", "d", "e"];
echo j(array_merge($a, $b)), "|";   // a,b,c,d,e
echo j($a), "|", j($b), "|";        // a,b|c,d,e  (sources intact)
$h = ["k" => ["x", "y"]];
$mixed = $h["k"];                    // Mixed via array-element access
echo j(array_merge($mixed, ["z"]));  // x,y,z
"#,
    );
    assert_eq!(out, "a,b,c,d,e|a,b|c,d,e|x,y,z");
}

/// Verifies `array_merge` uses the right operand element type when the left array is empty.
#[test]
fn test_array_merge_empty_left_uses_right_element_type() {
    let out = compile_and_run(
        r#"<?php
$a = [];
$b = [3, 4];
$c = array_merge($a, $b);
echo count($c);
echo ":";
echo $c[0] . $c[1];
"#,
    );
    assert_eq!(out, "2:34");
}

/// Verifies the `+` operator keeps the left operand's values when both arrays
/// have the same numeric key (left wins semantics).
#[test]
fn test_indexed_array_union_keeps_left_duplicate_numeric_keys() {
    let out = compile_and_run(
        r#"<?php
$left = [10, 20];
$right = [99, 88, 77];
$result = $left + $right;
echo count($result) . ":" . $result[0] . "," . $result[1] . "," . $result[2];
"#,
    );
    assert_eq!(out, "3:10,20,77");
}

/// Verifies the `+` operator appends right-side string-keyed values that do not
/// exist in the left array (right-side keys are preserved for non-conflicting entries).
#[test]
fn test_indexed_array_union_string_values_append_missing_suffix() {
    let out = compile_and_run(
        r#"<?php
$left = ["left"];
$right = ["ignored", "added"];
$result = $left + $right;
echo count($result) . ":" . $result[0] . "," . $result[1];
"#,
    );
    assert_eq!(out, "2:left,added");
}

/// Verifies that an empty left array combined with `+` produces a result whose
/// indices and count mirror the right operand.
#[test]
fn test_indexed_array_union_empty_left_copies_right_layout() {
    let out = compile_and_run(
        r#"<?php
$result = [] + ["first", "second"];
echo count($result) . ":" . $result[0] . "," . $result[1];
"#,
    );
    assert_eq!(out, "2:first,second");
}

/// Regression (#679): the three-argument strict form of `array_search` is rewritten
/// to a shared injected shim declared `int|string|false`, which used to erase the
/// haystack's key type at every call site. Over a list the result must satisfy a
/// declared `int` return — this is the exact `ParentChain::depthOf` shape that
/// blocked three native-survey roots.
#[test]
fn test_strict_array_search_over_a_list_satisfies_a_declared_int_return() {
    let out = compile_and_run(
        r#"<?php
function depthOf(string $x): int {
    $items = ["a", "b", "c"];
    $d = array_search($x, $items, true);
    if ($d === false) { return -1; }
    return $d;
}
echo depthOf("b") . "," . depthOf("zz");
"#,
    );
    assert_eq!(out, "1,-1");
}

/// Regression (#679): over a STRING-keyed map the strict form must yield the string
/// key, satisfying a declared `string` return. This is the load-bearing case — the
/// shim is one shared compiled function, so narrowing its call-site type is only
/// sound if the returned value really carries the key's representation.
#[test]
fn test_strict_array_search_over_a_string_keyed_map_returns_the_string_key() {
    let out = compile_and_run(
        r#"<?php
function findKey(string $needle): string {
    $map = ["alpha" => "a-value", "beta" => "b-value"];
    $k = array_search($needle, $map, true);
    if ($k === false) { return "MISS"; }
    return $k;
}
echo findKey("b-value") . "," . findKey("nope");
"#,
    );
    assert_eq!(out, "beta,MISS");
}

/// Regression (#679): over a sparse INT-keyed map the strict form must yield the
/// declared key, not the positional offset.
#[test]
fn test_strict_array_search_over_a_sparse_int_keyed_map_returns_the_key() {
    let out = compile_and_run(
        r#"<?php
function findIdx(string $needle): int {
    $map = [5 => "x", 10 => "y", 3 => "z"];
    $k = array_search($needle, $map, true);
    if ($k === false) { return -1; }
    return $k;
}
echo findIdx("y") . "," . findIdx("nope");
"#,
    );
    assert_eq!(out, "10,-1");
}

/// Regression (#679): a haystack that is not statically array-shaped must KEEP the
/// shim's full `int|string|false` union. Narrowing on a guess would silently
/// truncate a real string key, which is a miscompile rather than a compile error.
#[test]
fn test_strict_array_search_over_an_indeterminate_haystack_keeps_the_wide_union() {
    let out = compile_and_run(
        r#"<?php
function opaque(mixed $x): mixed { return $x; }
$arr = opaque(["p", "q", "r"]);
$k = array_search("r", $arr, true);
var_dump($k);
"#,
    );
    assert_eq!(out, "int(2)\n");
}
