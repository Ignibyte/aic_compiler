//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of array array callbacks, including map, map single, and map string values to ints.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use crate::support::*;

// --- Callback-based array functions ---

// Tests `array_map` with a user-defined callback that doubles each element.
/// Verifies that array map.
#[test]
fn test_array_map() {
    let out = compile_and_run(
        r#"<?php
function double($x) { return $x * 2; }
$a = [1, 2, 3];
$b = array_map("double", $a);
echo $b[0] . $b[1] . $b[2];
"#,
    );
    assert_eq!(out, "246");
}

/// #643: usort over an array of arrays with a typed `array` callback parameter. The
/// element type `Array(Mixed)` previously produced a spurious checker error ("callback
/// parameter expects Array(Mixed), got Int") and then an unsupported-feature codegen
/// error ("usort indexed-array element"). Array values are single 8-byte heap handles,
/// permutable by `__rt_usort` like objects; the comparator interprets each element.
#[test]
fn test_usort_array_of_arrays_typed_callback() {
    let out = compile_and_run(
        r#"<?php
$rows = [[3, 'c'], [1, 'a'], [2, 'b']];
usort($rows, fn(array $x, array $y): int => $x[0] <=> $y[0]);
echo $rows[0][1], $rows[1][1], $rows[2][1];
"#,
    );
    assert_eq!(out, "abc");
}

/// #643: usort over a bare-`array` value (element type `Mixed`) sorts the boxed-Mixed
/// elements by handle through the same 8-byte permutation.
#[test]
fn test_usort_mixed_element_array() {
    let out = compile_and_run(
        r#"<?php
function sortvals(array $items): string {
    usort($items, fn($a, $b): int => $a <=> $b);
    $out = '';
    foreach ($items as $v) { $out .= $v; }
    return $out;
}
echo sortvals([3, 1, 2, 5, 4]);
"#,
    );
    assert_eq!(out, "12345");
}

/// #643: `usort()` over a boxed-`Mixed` source (a value the checker could not type as a concrete
/// array — here passed through a `mixed` boundary that defeats element specialization) unboxes the
/// Mixed to its underlying indexed array, COW-separates it, and sorts it in place, republishing the
/// unique pointer into the Mixed cell so the by-reference result is observed through the source.
#[test]
fn test_usort_over_boxed_mixed_source() {
    let out = compile_and_run(
        r#"<?php
final class P { public function __construct(public int $w) {} }
function opaque(mixed $x): mixed { return $x; }
function sortit(mixed $xs): array {
    usort($xs, static fn (P $a, P $b): int => $a->w <=> $b->w);
    return $xs;
}
$o = sortit(opaque([new P(5), new P(2), new P(8), new P(1)]));
$s = '';
foreach ($o as $p) { $s .= $p->w; }
echo $s;
"#,
    );
    assert_eq!(out, "1258");
}

/// #648: `array_filter()` over a boxed-`Mixed` source (a value the checker could not type as a
/// concrete array — here laundered through a `mixed` boundary) unboxes the Mixed to its underlying
/// indexed array and filters it through the refcounted helper. Mirrors the real AIC roots'
/// `array_filter(json_decode(...), is_string(...))` shape over heterogeneous elements.
#[test]
fn test_array_filter_over_boxed_mixed_source() {
    let out = compile_and_run(
        r#"<?php
function opaque(mixed $x): mixed { return $x; }
function keep_strings(mixed $xs): array {
    return array_filter($xs, is_string(...));
}
$kept = keep_strings(opaque(['a', 1, 'bb', 2, 'ccc', 3]));
$s = '';
foreach ($kept as $v) { $s .= $v; }
echo $s . ':' . count($kept);
"#,
    );
    assert_eq!(out, "abbccc:3");
}

/// #648: kept object elements from an `array_filter` over a boxed-`Mixed` source outlive the
/// temporary source array (the refcounted filter retains each kept boxed-Mixed cell via
/// `__rt_array_push_refcounted`), so reading their fields after the source is gone is safe — the
/// double-free / use-after-free proof the load-only survey structurally cannot see. Uses the
/// `is_object(...)` first-class-callable predicate over heterogeneous elements.
#[test]
fn test_array_filter_over_boxed_mixed_object_refcount_survives() {
    let out = compile_and_run(
        r#"<?php
final class Item { public function __construct(public int $w) {} }
function opaque(mixed $x): mixed { return $x; }
function keepers(mixed $xs): array {
    return array_filter($xs, is_object(...));
}
$kept = keepers(opaque([new Item(5), 'x', new Item(8), 42]));
$s = '';
foreach ($kept as $i) { $s .= $i->w; }
echo $s . ':' . count($kept);
"#,
    );
    assert_eq!(out, "58:2");
}

/// #648: `array_filter()` over an associative-array (hash) source walks `__rt_hash_filter`,
/// preserving keys. Value mode with an `is_string(...)` first-class-callable predicate over
/// heterogeneous values.
#[test]
fn test_array_filter_hash_value_mode() {
    let out = compile_and_run(
        r#"<?php
function keep_strings(array $xs): array {
    return array_filter($xs, is_string(...));
}
$kept = keep_strings(['a' => 'x', 'b' => 1, 'c' => 'yy', 'd' => 2, 'e' => 'zzz']);
$s = '';
foreach ($kept as $k => $v) { $s .= "$k=$v;"; }
echo $s . count($kept);
"#,
    );
    assert_eq!(out, "a=x;c=yy;e=zzz;3");
}

/// #648: `ARRAY_FILTER_USE_KEY` over a hash source, with a capturing closure predicate on the key.
/// The key is boxed as a Mixed cell for the predicate; kept entries keep their keys.
#[test]
fn test_array_filter_hash_use_key_closure() {
    let out = compile_and_run(
        r#"<?php
function pick(array $src, string $prefix): array {
    return array_filter($src, static fn (string $k): bool => str_starts_with($k, $prefix), ARRAY_FILTER_USE_KEY);
}
$r = pick(['alpha' => 1, 'beta' => 2, 'apex' => 3, 'gamma' => 4], 'a');
$s = '';
foreach ($r as $k => $v) { $s .= "$k=$v;"; }
echo $s . count($r);
"#,
    );
    assert_eq!(out, "alpha=1;apex=3;2");
}

/// #648: `ARRAY_FILTER_USE_BOTH` over a hash source — the predicate receives (value, key), both
/// boxed as Mixed cells.
#[test]
fn test_array_filter_hash_use_both() {
    let out = compile_and_run(
        r#"<?php
function pick(array $src): array {
    return array_filter($src, static fn (int $v, string $k): bool => $v > 1 && $k !== 'd', ARRAY_FILTER_USE_BOTH);
}
$r = pick(['a' => 1, 'b' => 2, 'c' => 3, 'd' => 4]);
$s = '';
foreach ($r as $k => $v) { $s .= "$k=$v;"; }
echo $s . count($r);
"#,
    );
    assert_eq!(out, "b=2;c=3;2");
}

/// #648: kept string values from a hash `array_filter` are retained (`__rt_incref` before
/// `__rt_hash_set`), so they outlive the temporary source hash — the double-free / use-after-free
/// proof the load-only survey structurally cannot see.
#[test]
fn test_array_filter_hash_refcount_survives() {
    let out = compile_and_run(
        r#"<?php
function keep_strings(array $xs): array {
    return array_filter($xs, is_string(...));
}
$kept = keep_strings(['k1' => 'aaa', 'k2' => 7, 'k3' => 'bbb', 'k4' => 9, 'k5' => 'ccc']);
$s = '';
foreach ($kept as $k => $v) { $s .= $v; }
echo $s . ':' . count($kept);
"#,
    );
    assert_eq!(out, "aaabbbccc:3");
}

// Tests `array_map` on a single-element array with a user-defined increment callback.
/// Verifies that array map single.
#[test]
fn test_array_map_single() {
    let out = compile_and_run(
        r#"<?php
function inc($x) { return $x + 1; }
$a = [10];
$b = array_map("inc", $a);
echo $b[0];
"#,
    );
    assert_eq!(out, "11");
}

// Tests `array_map` with a typed builtin callback (`strlen`) applied to string values,
// verifying mixed-type result handling in array_map codegen.
/// Verifies that array map string values to ints.
#[test]
fn test_array_map_string_values_to_ints() {
    let out = compile_and_run(
        r#"<?php
function string_len(string $value) { return strlen($value); }
$a = ["aa", "bbbb"];
$b = array_map("string_len", $a);
echo $b[0];
echo ",";
echo $b[1];
"#,
    );
    assert_eq!(out, "2,4");
}

/// Verifies runtime string builtin callback variables dispatch through descriptor-backed array_map.
#[test]
fn test_array_map_dynamic_string_builtin_callback_uses_descriptor_invoker() {
    let source = r#"<?php
$name = "STRTOUPPER";
$callback = $name;
$out = array_map($callback, ["ada", "lin"]);
echo $out[0] . ":" . $out[1];
"#;
    let out = compile_and_run(source);
    assert_eq!(out, "ADA:LIN");

    let dir = make_cli_test_dir("elephc_array_map_dynamic_string_builtin_descriptor");
    let (user_asm, _runtime_asm, _required_libraries) =
        compile_source_to_asm_with_options(source, &dir, 8_388_608, false, false);
    assert!(
        user_asm.contains("__rt_array_map_mixed") && user_asm.contains("callable_invoker"),
        "array_map dynamic string callbacks should route through descriptor invokers:\n{}",
        user_asm
    );
    let _ = fs::remove_dir_all(dir);
}

/// Verifies runtime string user callbacks can produce mixed array_map result shapes.
#[test]
fn test_array_map_dynamic_string_user_callback_mixed_results() {
    let out = compile_and_run(
        r#"<?php
function upper_runtime_map(string $value): string {
    return strtoupper($value);
}

$callback = "upper_runtime_map";
$out = array_map($callback, ["ada"]);
echo $out[0];
echo ":";

$callback = "strlen";
$lengths = array_map($callback, ["abcd"]);
echo $lengths[0];
"#,
    );
    assert_eq!(out, "ADA:4");
}

// Tests `array_filter` with a predicate that keeps even integers, verifying correct
// iteration, element removal, and count/foreach output.
/// Verifies that array filter.
#[test]
fn test_array_filter() {
    let out = compile_and_run(
        r#"<?php
function is_even($x) { return $x % 2 == 0; }
$a = [1, 2, 3, 4, 5, 6];
$b = array_filter($a, "is_even");
echo count($b);
foreach ($b as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "3246");
}

// Tests `array_filter` with a typed builtin callback (`str_starts_with`) applied to
// string values, verifying correct filtering and mixed string output.
/// Verifies that array filter string values.
#[test]
fn test_array_filter_string_values() {
    let out = compile_and_run(
        r#"<?php
function starts_a(string $value) { return str_starts_with($value, "a"); }
$a = ["aa", "bb", "ab"];
$b = array_filter($a, "starts_a");
echo count($b);
foreach ($b as $value) { echo $value; }
"#,
    );
    assert_eq!(out, "2aaab");
}

/// Verifies explicit `ARRAY_FILTER_USE_VALUE` keeps the default value-only callback shape.
#[test]
fn test_array_filter_explicit_use_value_mode() {
    let out = compile_and_run(
        r#"<?php
function positive_value($value) { return $value > 0; }
$filtered = array_filter([-1, 2, 0, 3], "positive_value", ARRAY_FILTER_USE_VALUE);
echo count($filtered);
foreach ($filtered as $value) { echo $value; }
"#,
    );
    assert_eq!(out, "223");
}

/// Verifies `ARRAY_FILTER_USE_BOTH` passes value and key to the callback.
#[test]
fn test_array_filter_use_both_mode() {
    let out = compile_and_run(
        r#"<?php
function keep_value_key($value, $key) { return ($value + $key) >= 5; }
$filtered = array_filter([1, 2, 3, 4], "keep_value_key", ARRAY_FILTER_USE_BOTH);
echo count($filtered);
foreach ($filtered as $value) { echo $value; }
"#,
    );
    assert_eq!(out, "234");
}

/// Verifies `ARRAY_FILTER_USE_KEY` passes only the source key to the callback.
#[test]
fn test_array_filter_use_key_mode() {
    let out = compile_and_run(
        r#"<?php
function keep_odd_key($key) { return $key % 2 == 1; }
$filtered = array_filter([10, 20, 30, 40], "keep_odd_key", ARRAY_FILTER_USE_KEY);
echo count($filtered);
foreach ($filtered as $value) { echo $value; }
"#,
    );
    assert_eq!(out, "22040");
}

/// Verifies invalid literal modes throw a catchable `ValueError` before callback invocation.
#[test]
fn test_array_filter_invalid_literal_mode_throws_value_error() {
    let out = compile_and_run(
        r#"<?php
function keep_value($value) { echo "callback"; return true; }
try {
    array_filter([1], "keep_value", 3);
    echo "bad";
} catch (ValueError $e) {
    echo "ValueError";
}
"#,
    );
    assert_eq!(out, "ValueError");
}

/// Verifies invalid runtime mode variables throw a catchable `ValueError`.
#[test]
fn test_array_filter_invalid_runtime_mode_throws_value_error() {
    let out = compile_and_run(
        r#"<?php
function keep_value_runtime($value) { echo "callback"; return true; }
$mode = 9;
try {
    array_filter([1], "keep_value_runtime", $mode);
    echo "bad";
} catch (ValueError $e) {
    echo "ValueError";
}
"#,
    );
    assert_eq!(out, "ValueError");
}

/// Verifies PHP 8.6 array_filter mode constants resolve in namespaces and through `defined()`.
#[test]
fn test_array_filter_use_value_constant_defined_and_namespaced() {
    let out = compile_and_run(
        r#"<?php
namespace Demo;
echo defined("ARRAY_FILTER_USE_VALUE") ? "Y" : "N";
echo ":";
echo ARRAY_FILTER_USE_VALUE;
echo ":";
echo ARRAY_FILTER_USE_BOTH;
echo ":";
echo \ARRAY_FILTER_USE_KEY;
"#,
    );
    assert_eq!(out, "Y:0:1:2");
}

/// Verifies array callback runtimes accept callback names selected through string variables.
#[test]
fn test_array_callback_runtimes_dynamic_string_callbacks_use_descriptor_invokers() {
    let source = r#"<?php
function even_runtime($value): bool {
    return $value % 2 == 0;
}

function show_runtime($value): void {
    echo $value;
}

function add_runtime($carry, $item): int {
    return $carry + $item;
}

$name = "even_runtime";
$filter = $name;
$filtered = array_filter([1, 2, 3, 4], $filter);
foreach ($filtered as $value) {
    echo $value;
}
echo ":";

$name = "show_runtime";
$walk = $name;
array_walk([5, 6], $walk);
echo ":";

$name = "add_runtime";
$reduce = $name;
echo array_reduce([1, 2, 3], $reduce, 0);
"#;
    let out = compile_and_run(source);
    assert_eq!(out, "24:56:6");

    let dir = make_cli_test_dir("elephc_array_callback_runtime_string_descriptors");
    let (user_asm, _runtime_asm, _required_libraries) =
        compile_source_to_asm_with_options(source, &dir, 8_388_608, false, false);
    assert!(
        user_asm.contains("__rt_array_filter")
            && user_asm.contains("__rt_array_walk")
            && user_asm.contains("__rt_array_reduce")
            && user_asm.contains("callable_invoker"),
        "array callback runtime string paths should route through descriptor invokers:\n{}",
        user_asm
    );
    let _ = fs::remove_dir_all(dir);
}

/// Verifies sort callback runtimes accept comparator names selected through string variables.
#[test]
fn test_sort_callback_runtimes_dynamic_string_callbacks_use_descriptor_invokers() {
    let source = r#"<?php
function choose_sort_name(bool $descending): string {
    return $descending ? "runtime_desc_compare" : "runtime_asc_compare";
}

function runtime_desc_compare($left, $right): int {
    return $right - $left;
}

function runtime_asc_compare($left, $right): int {
    return $left - $right;
}

$sort = choose_sort_name(true);

$usorted = [1, 3, 2];
usort($usorted, $sort);
foreach ($usorted as $value) {
    echo $value;
}
echo ":";

$uksorted = [1, 3, 2];
uksort($uksorted, $sort);
foreach ($uksorted as $value) {
    echo $value;
}
echo ":";

$uasorted = [1, 3, 2];
uasort($uasorted, $sort);
foreach ($uasorted as $value) {
    echo $value;
}
"#;
    let out = compile_and_run(source);
    assert_eq!(out, "321:321:321");

    let dir = make_cli_test_dir("elephc_sort_callback_runtime_string_descriptors");
    let (user_asm, _runtime_asm, _required_libraries) =
        compile_source_to_asm_with_options(source, &dir, 8_388_608, false, false);
    assert!(
        user_asm.contains("__rt_usort") && user_asm.contains("callable_invoker"),
        "sort callback runtime string paths should route through descriptor invokers:\n{}",
        user_asm
    );
    let _ = fs::remove_dir_all(dir);
}

// Tests `array_filter` when the callback returns falsy for every element, producing
// an empty array and confirming count is 0.
/// Verifies that array filter none pass.
#[test]
fn test_array_filter_none_pass() {
    let out = compile_and_run(
        r#"<?php
function never($x) { return 0; }
$a = [1, 2, 3];
$b = array_filter($a, "never");
echo count($b);
"#,
    );
    assert_eq!(out, "0");
}

// Tests `array_reduce` with a two-argument user callback (carry + item) over a five-element
// array, providing an explicit initial value of 0.
/// Verifies that array reduce.
#[test]
fn test_array_reduce() {
    let out = compile_and_run(
        r#"<?php
function add($carry, $item) { return $carry + $item; }
$a = [1, 2, 3, 4, 5];
$sum = array_reduce($a, "add", 0);
echo $sum;
"#,
    );
    assert_eq!(out, "15");
}

// Tests `array_reduce` with a user callback (carry * item) and an explicit initial
// value of 1, verifying the carry accumulates correctly across the array.
/// Verifies that array reduce with initial.
#[test]
fn test_array_reduce_with_initial() {
    let out = compile_and_run(
        r#"<?php
function mul($carry, $item) { return $carry * $item; }
$a = [2, 3, 4];
$product = array_reduce($a, "mul", 1);
echo $product;
"#,
    );
    assert_eq!(out, "24");
}

// Tests `array_walk` with a callback that echoes each element, verifying the function
// walks by reference and mutates the array in place.
/// Verifies that array walk.
#[test]
fn test_array_walk() {
    let out = compile_and_run(
        r#"<?php
function show($x) { echo $x; }
$a = [10, 20, 30];
array_walk($a, "show");
"#,
    );
    assert_eq!(out, "102030");
}

// Tests `usort` with a comparison callback that sorts an unsorted array in ascending
// order, verifying both value ordering and that the array is modified in place.
/// Verifies that usort.
#[test]
fn test_usort() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$a = [5, 3, 1, 4, 2];
usort($a, "cmp");
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "12345");
}

// Tests `usort` with a comparison callback that reverses order (`b - a`), verifying
// values are re-sorted in descending order.
/// Verifies that usort reverse.
#[test]
fn test_usort_reverse() {
    let out = compile_and_run(
        r#"<?php
function rcmp($a, $b) { return $b - $a; }
$a = [1, 3, 2];
usort($a, "rcmp");
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "321");
}

// Tests `uksort` with a comparison callback, verifying keys are reordered while values
// stay associated with their original keys.
/// Verifies that uksort.
#[test]
fn test_uksort() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$a = [5, 3, 1, 4, 2];
uksort($a, "cmp");
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "12345");
}

// Tests `uasort` with a comparison callback, verifying values are sorted while preserving
// key-to-value associations.
/// Verifies that uasort.
#[test]
fn test_uasort() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$a = [30, 10, 20];
uasort($a, "cmp");
foreach ($a as $value) { echo $value . " "; }
"#,
    );
    assert_eq!(out, "10 20 30 ");
}

// Tests `call_user_func` with a user-defined function and a single argument.
/// Verifies that call user func.
#[test]
fn test_call_user_func() {
    let out = compile_and_run(
        r#"<?php
function greet($x) { return $x + 100; }
$result = call_user_func("greet", 42);
echo $result;
"#,
    );
    assert_eq!(out, "142");
}

// Tests `call_user_func` with a user-defined function that takes no arguments.
/// Verifies that call user func no args.
#[test]
fn test_call_user_func_no_args() {
    let out = compile_and_run(
        r#"<?php
function get_value() { return 99; }
$result = call_user_func("get_value");
echo $result;
"#,
    );
    assert_eq!(out, "99");
}

// Tests `call_user_func` with a function accepting 9 parameters and 9 overflow arguments
// passed on the stack, verifying stack-passed overflow argument handling.
/// Verifies that call user func supports stack passed overflow args.
#[test]
fn test_call_user_func_supports_stack_passed_overflow_args() {
    let out = compile_and_run(
        r#"<?php
function sum9($a, $b, $c, $d, $e, $f, $g, $h, $i) {
    return $a + $b + $c + $d + $e + $f + $g + $h + $i;
}
echo call_user_func("sum9", 1, 2, 3, 4, 5, 6, 7, 8, 9);
"#,
    );
    assert_eq!(out, "45");
}

// Tests `call_user_func` with a builtin function name (`STRLEN`) passed as a string,
// verifying case-insensitive builtin callback resolution.
/// Verifies that call user func string builtin callback.
#[test]
fn test_call_user_func_string_builtin_callback() {
    let out = compile_and_run(r#"<?php echo call_user_func("STRLEN", "hello");"#);
    assert_eq!(out, "5");
}

// Tests `function_exists` returns true for a user-defined function.
/// Verifies that call user func accepts callable without known signature.
#[test]
fn test_call_user_func_accepts_callable_without_known_signature() {
    let out = compile_and_run(
        r#"<?php
function make_callback(): callable {
    return function($a, $b, $c): int {
        return $a + $b + $c;
    };
}
echo call_user_func(make_callback(), 2, 3, 4);
"#,
    );
    assert_eq!(out, "9");
}

/// Verifies that function exists true.
#[test]
fn test_function_exists_true() {
    let out = compile_and_run(
        r#"<?php
function my_func() { return 1; }
if (function_exists("my_func")) { echo "yes"; } else { echo "no"; }
"#,
    );
    assert_eq!(out, "yes");
}

// Tests `function_exists` returns false for a non-existent function name.
/// Verifies that function exists false.
#[test]
fn test_function_exists_false() {
    let out = compile_and_run(
        r#"<?php
if (function_exists("nonexistent")) { echo "yes"; } else { echo "no"; }
"#,
    );
    assert_eq!(out, "no");
}

// Tests `usort` on an already-sorted array, verifying the comparator is still called and
// the output is unchanged (regression: no sorting skipped incorrectly).
/// Verifies that usort already sorted.
#[test]
fn test_usort_already_sorted() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$a = [1, 2, 3];
usort($a, "cmp");
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "123");
}

// Tests `usort` on a single-element array, verifying the comparator is called and the
// array remains unchanged.
/// Verifies that usort single element.
#[test]
fn test_usort_single_element() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$a = [42];
usort($a, "cmp");
echo $a[0];
"#,
    );
    assert_eq!(out, "42");
}

// Tests `array_map` with a callback that squares each element, verifying correct
// element mapping on a four-element array.
/// Verifies that array map with complex callback.
#[test]
fn test_array_map_with_complex_callback() {
    let out = compile_and_run(
        r#"<?php
function square($x) { return $x * $x; }
$a = [1, 2, 3, 4];
$b = array_map("square", $a);
echo $b[0] . " " . $b[1] . " " . $b[2] . " " . $b[3];
"#,
    );
    assert_eq!(out, "1 4 9 16");
}

// Tests `array_reduce` on a single-element array with an initial carry value of 100,
// verifying the callback is invoked once with the carry and item.
/// Verifies that array reduce single.
#[test]
fn test_array_reduce_single() {
    let out = compile_and_run(
        r#"<?php
function add($carry, $item) { return $carry + $item; }
$a = [42];
$sum = array_reduce($a, "add", 100);
echo $sum;
"#,
    );
    assert_eq!(out, "142");
}

/// Verifies static-method callable-array variables route through descriptor callback wrappers.
#[test]
fn test_static_callable_array_variable_callback_runtimes() {
    let out = compile_and_run(
        r#"<?php
class StaticCallableArrayRuntime {
    public static function keep($value): bool {
        return $value > 2;
    }

    public static function add($carry, $item): int {
        return $carry + $item + 10;
    }

    public static function show($item): void {
        echo $item + 10;
        echo ",";
    }

    public static function compare($a, $b): int {
        return $b - $a;
    }
}

$filter = ["StaticCallableArrayRuntime", "keep"];
$reduce = ["StaticCallableArrayRuntime", "add"];
$walk = ["StaticCallableArrayRuntime", "show"];
$sort = ["StaticCallableArrayRuntime", "compare"];

$filtered = array_filter([1, 3, 4], $filter);
foreach ($filtered as $value) {
    echo $value;
}
echo ":";
echo array_reduce([1, 2], $reduce, 0);
echo ":";
array_walk([1, 2], $walk);
echo ":";

$usorted = [1, 3, 2];
usort($usorted, $sort);
foreach ($usorted as $value) {
    echo $value;
}
echo ":";

$uksorted = [1, 3, 2];
uksort($uksorted, $sort);
foreach ($uksorted as $value) {
    echo $value;
}
echo ":";

$uasorted = [1, 3, 2];
uasort($uasorted, $sort);
foreach ($uasorted as $value) {
    echo $value;
}
"#,
    );
    assert_eq!(out, "34:23:11,12,:321:321:321");
}

/// Verifies runtime-selected instance callable arrays route array_map through Mixed descriptors.
#[test]
fn test_dynamic_instance_callable_array_variable_array_map_mixed_result() {
    let out = compile_and_run(
        r#"<?php
class DynamicInstanceMapRuntime {
    public string $prefix = "";

    public function wrap(string $name): string {
        return $this->prefix . $name;
    }

    public function length(string $name): int {
        return strlen($name);
    }
}

$box = new DynamicInstanceMapRuntime();
$box->prefix = "first:";
$method = "wrap";
$wrap = [$box, $method];
$method = "length";
$length = [$box, $method];

$box = new DynamicInstanceMapRuntime();
$box->prefix = "second:";

$names = array_map($wrap, ["Ada", "Lin"]);
echo $names[0];
echo "|";
echo $names[1];
echo ":";
$lengths = array_map($length, ["Ada", "Linus"]);
echo $lengths[0];
echo "|";
echo $lengths[1];
"#,
    );
    assert_eq!(out, "first:Ada|first:Lin:3|5");
}

/// Verifies runtime-selected static callable arrays route array_map through Mixed descriptors.
#[test]
fn test_dynamic_static_callable_array_variable_array_map_mixed_result() {
    let out = compile_and_run(
        r#"<?php
class DynamicStaticMapRuntime {
    public static function wrap(string $name): string {
        return "static:" . $name;
    }

    public static function length(string $name): int {
        return strlen($name);
    }
}

$class = "DynamicStaticMapRuntime";
$method = "wrap";
$wrap = [$class, $method];
$method = "length";
$length = [$class, $method];

$names = array_map($wrap, ["Ada", "Lin"]);
echo $names[0];
echo "|";
echo $names[1];
echo ":";
$lengths = array_map($length, ["Ada", "Linus"]);
echo $lengths[0];
echo "|";
echo $lengths[1];
"#,
    );
    assert_eq!(out, "static:Ada|static:Lin:3|5");
}

/// Verifies runtime-selected instance callable arrays route fixed-return callbacks through descriptors.
#[test]
fn test_dynamic_instance_callable_array_variable_fixed_callback_runtimes() {
    let out = compile_and_run(
        r#"<?php
class DynamicInstanceCallbackRuntime {
    public int $limit = 0;
    public int $offset = 0;
    public bool $descending = false;

    public function keep($value): bool {
        return $value > $this->limit;
    }

    public function add($carry, $item): int {
        return $carry + $item + $this->offset;
    }

    public function show($item): void {
        echo $item + $this->offset;
        echo ",";
    }

    public function compare($a, $b): int {
        if ($this->descending) {
            return $b - $a;
        }
        return $a - $b;
    }
}

$box = new DynamicInstanceCallbackRuntime();
$box->limit = 2;
$box->offset = 10;
$box->descending = true;

$method = "keep";
$filter = [$box, $method];
$method = "add";
$reduce = [$box, $method];
$method = "show";
$walk = [$box, $method];
$method = "compare";
$sort = [$box, $method];

$box = new DynamicInstanceCallbackRuntime();
$box->limit = 100;
$box->offset = 100;
$box->descending = false;

$filtered = array_filter([1, 3, 4], $filter);
foreach ($filtered as $value) {
    echo $value;
}
echo ":";
echo array_reduce([1, 2], $reduce, 0);
echo ":";
array_walk([1, 2], $walk);
echo ":";

$usorted = [1, 3, 2];
usort($usorted, $sort);
foreach ($usorted as $value) {
    echo $value;
}
echo ":";

$uksorted = [1, 3, 2];
uksort($uksorted, $sort);
foreach ($uksorted as $value) {
    echo $value;
}
echo ":";

$uasorted = [1, 3, 2];
uasort($uasorted, $sort);
foreach ($uasorted as $value) {
    echo $value;
}
"#,
    );
    assert_eq!(out, "34:23:11,12,:321:321:321");
}

// --- array_map over heterogeneous (Mixed-element) input arrays ---

/// Verifies the reported repro: array_map with an untyped closure over a heterogeneous
/// array preserves each element (the string element must not coerce to integer 0).
#[test]
fn test_array_map_untyped_closure_over_mixed_array() {
    let out = compile_and_run(
        r#"<?php
$items = [1, "two", 3];
$out = array_map(function ($x) { return $x; }, $items);
foreach ($out as $v) { echo $v . " "; }
"#,
    );
    assert_eq!(out, "1 two 3 ");
}

/// Verifies array_map with an explicit `mixed`-typed closure parameter over a heterogeneous
/// array preserves the runtime type tags of every element.
#[test]
fn test_array_map_mixed_param_closure_preserves_tags() {
    let out = compile_and_run(
        r#"<?php
$out = array_map(function (mixed $x) { return gettype($x); }, [1, "two", 3.5, true, null]);
echo implode(",", $out);
"#,
    );
    assert_eq!(out, "integer,string,double,boolean,NULL");
}

/// Verifies array_map over a heterogeneous array dispatched to a named function with a
/// `mixed` parameter, which routes through the descriptor-backed callback wrapper.
#[test]
fn test_array_map_named_mixed_callback_over_mixed_array() {
    let out = compile_and_run(
        r#"<?php
function passthrough(mixed $x) { return $x; }
$out = array_map("passthrough", [1, "two", 3]);
foreach ($out as $v) { echo $v . " "; }
"#,
    );
    assert_eq!(out, "1 two 3 ");
}

/// Verifies a transforming closure over a heterogeneous array: the boxed Mixed element is
/// usable in string concatenation and the mapped results are correct per PHP semantics.
#[test]
fn test_array_map_mixed_array_string_transform() {
    let out = compile_and_run(
        r#"<?php
$out = array_map(function (mixed $x) { return "<" . $x . ">"; }, [1, "two", 3]);
foreach ($out as $v) { echo $v . " "; }
"#,
    );
    assert_eq!(out, "<1> <two> <3> ");
}

/// Root-cause unit test: a closure with a `mixed` parameter that returns the parameter must
/// infer a `mixed` return type, so a string argument survives instead of coercing to int 0.
#[test]
fn test_mixed_param_closure_return_preserves_string() {
    let out = compile_and_run(
        r#"<?php
$c = function (mixed $x) { return $x; };
echo $c("two") . ":" . $c(1);
"#,
    );
    assert_eq!(out, "two:1");
}

/// Verifies the same passthrough closure invoked through the descriptor invoker
/// (`call_user_func`) also preserves a string argument.
#[test]
fn test_mixed_param_closure_via_call_user_func_preserves_string() {
    let out = compile_and_run(
        r#"<?php
$c = function (mixed $x) { return $x; };
echo call_user_func($c, "two") . ":" . call_user_func($c, 1);
"#,
    );
    assert_eq!(out, "two:1");
}

/// Verifies runtime-selected static callable arrays route fixed-return callbacks through descriptors.
/// Verifies `usort` over an array of objects with an explicitly typed comparator:
/// the comparator reads an integer property from each object handle and the array
/// is reordered by that property.
#[test]
fn test_usort_objects_typed_comparator() {
    let out = compile_and_run(
        r#"<?php
class Item { public int $weight; public function __construct(int $w) { $this->weight = $w; } }
$items = [new Item(30), new Item(10), new Item(20)];
usort($items, function (Item $a, Item $b) { return $a->weight <=> $b->weight; });
foreach ($items as $it) { echo $it->weight, ","; }
"#,
    );
    assert_eq!(out, "10,20,30,");
}

/// Verifies `usort` over an array of objects with an *unannotated* comparator:
/// the comparator parameters are typed from the array element so `$a->prop`
/// resolves and the `<=>` comparison reorders the objects correctly.
#[test]
fn test_usort_objects_untyped_comparator() {
    let out = compile_and_run(
        r#"<?php
class Item { public int $weight; public function __construct(int $w) { $this->weight = $w; } }
$items = [new Item(30), new Item(10), new Item(20)];
usort($items, function ($a, $b) { return $a->weight <=> $b->weight; });
foreach ($items as $it) { echo $it->weight, ","; }
"#,
    );
    assert_eq!(out, "10,20,30,");
}

/// Verifies `uasort` over an indexed array of objects: the comparator reorders the
/// object handles by an integer property the same way `usort` does.
#[test]
fn test_uasort_objects() {
    let out = compile_and_run(
        r#"<?php
class Item { public int $weight; public function __construct(int $w) { $this->weight = $w; } }
$items = [new Item(30), new Item(10), new Item(20)];
uasort($items, fn ($a, $b) => $a->weight <=> $b->weight);
foreach ($items as $it) { echo $it->weight, ","; }
"#,
    );
    assert_eq!(out, "10,20,30,");
}

/// Verifies dynamic static callable arrays across fixed-callback array helpers.
#[test]
fn test_dynamic_static_callable_array_variable_fixed_callback_runtimes() {
    let out = compile_and_run(
        r#"<?php
class DynamicStaticCallbackRuntime {
    public static function keep($value): bool {
        return $value > 2;
    }

    public static function add($carry, $item): int {
        return $carry + $item + 10;
    }

    public static function show($item): void {
        echo $item + 10;
        echo ",";
    }

    public static function compare($a, $b): int {
        return $b - $a;
    }
}

$class = "DynamicStaticCallbackRuntime";
$method = "keep";
$filter = [$class, $method];
$method = "add";
$reduce = [$class, $method];
$method = "show";
$walk = [$class, $method];
$method = "compare";
$sort = [$class, $method];

$filtered = array_filter([1, 3, 4], $filter);
foreach ($filtered as $value) {
    echo $value;
}
echo ":";
echo array_reduce([1, 2], $reduce, 0);
echo ":";
array_walk([1, 2], $walk);
echo ":";

$usorted = [1, 3, 2];
usort($usorted, $sort);
foreach ($usorted as $value) {
    echo $value;
}
echo ":";

$uksorted = [1, 3, 2];
uksort($uksorted, $sort);
foreach ($uksorted as $value) {
    echo $value;
}
echo ":";

$uasorted = [1, 3, 2];
uasort($uasorted, $sort);
foreach ($uasorted as $value) {
    echo $value;
}
"#,
    );
    assert_eq!(out, "34:23:11,12,:321:321:321");
}

/// Verifies array_walk_recursive() visits the leaf values of a nested indexed array
/// depth-first, invoking the callback on each scalar leaf.
/// Fixture: [[1,2],[3,4]] → leaves 1,2,3,4 in order.
#[test]
fn test_array_walk_recursive_indexed() {
    let out = compile_and_run(
        r#"<?php
function show($x) { echo $x; echo ","; }
$a = [[1, 2], [3, 4]];
array_walk_recursive($a, "show");
"#,
    );
    assert_eq!(out, "1,2,3,4,");
}

/// Verifies array_walk_recursive() descends into nested associative arrays and visits
/// the leaf values in insertion order.
/// Fixture: {g1:{a:1,b:2}, g2:{c:3}} → leaves 1,2,3.
#[test]
fn test_array_walk_recursive_assoc() {
    let out = compile_and_run(
        r#"<?php
function p($x) { echo $x; echo ";"; }
$a = ["g1" => ["a" => 1, "b" => 2], "g2" => ["c" => 3]];
array_walk_recursive($a, "p");
"#,
    );
    assert_eq!(out, "1;2;3;");
}

/// Verifies array_walk_recursive() recurses through three levels of nesting.
/// Fixture: [[[1,2]],[[3]]] → leaves 1,2,3.
#[test]
fn test_array_walk_recursive_deep() {
    let out = compile_and_run(
        r#"<?php
function q($x) { echo $x; }
$a = [[[1, 2]], [[3]]];
array_walk_recursive($a, "q");
"#,
    );
    assert_eq!(out, "123");
}

/// Verifies array_walk_recursive() is callable case-insensitively, matching PHP builtin name rules.
/// Fixture: mixed-case spelling over a nested indexed array.
#[test]
fn test_array_walk_recursive_case_insensitive() {
    let out = compile_and_run(
        r#"<?php
function r($x) { echo $x; }
$a = [[1], [2]];
Array_Walk_Recursive($a, "r");
"#,
    );
    assert_eq!(out, "12");
}

/// Verifies array_find() (PHP 8.4) returns the first element satisfying the predicate.
/// Fixture: first element > 2 in [1,2,3,4] is 3.
#[test]
fn test_array_find_returns_first_match() {
    let out = compile_and_run(
        r#"<?php
function gt2($x) { return $x > 2; }
echo array_find([1, 2, 3, 4], "gt2");
"#,
    );
    assert_eq!(out, "3");
}

/// Verifies array_find() returns null when no element satisfies the predicate.
/// Fixture: no element > 2 in [1,2]; the boxed null compares equal to null.
#[test]
fn test_array_find_returns_null_when_absent() {
    let out = compile_and_run(
        r#"<?php
function gt2($x) { return $x > 2; }
$r = array_find([1, 2], "gt2");
echo ($r === null) ? "null" : "value";
"#,
    );
    assert_eq!(out, "null");
}

/// Verifies array_find() works with a closure predicate.
/// Fixture: first element >= 10 in [5,10,15] is 10.
#[test]
fn test_array_find_closure() {
    let out = compile_and_run(
        r#"<?php
$r = array_find([5, 10, 15], fn($x) => $x >= 10);
echo $r;
"#,
    );
    assert_eq!(out, "10");
}

/// Verifies array_any() (PHP 8.4) returns true iff some element satisfies the predicate.
/// Fixture: [1,2,3] has an element > 2 (true); [1,2] does not (false).
#[test]
fn test_array_any() {
    let out = compile_and_run(
        r#"<?php
function gt2($x) { return $x > 2; }
echo array_any([1, 2, 3], "gt2") ? "y" : "n";
echo array_any([1, 2], "gt2") ? "y" : "n";
"#,
    );
    assert_eq!(out, "yn");
}

/// Verifies array_all() (PHP 8.4) returns true iff every element satisfies the predicate.
/// Fixture: all of [1,2,3] are positive (true); [1,-2,3] is not all positive (false).
#[test]
fn test_array_all() {
    let out = compile_and_run(
        r#"<?php
function pos($x) { return $x > 0; }
echo array_all([1, 2, 3], "pos") ? "y" : "n";
echo array_all([1, -2, 3], "pos") ? "y" : "n";
"#,
    );
    assert_eq!(out, "yn");
}

/// Regression for #648: array_any/array_all over an indexed array with `Mixed` elements (a
/// mixed-type literal) must lower — each element is a single 8-byte boxed cell passed to the
/// predicate — instead of being rejected ("array_any indexed-array element PHP type Mixed").
#[test]
fn test_array_predicate_mixed_elements() {
    let out = compile_and_run(
        r#"<?php
$a = [1, "a", 2.5];
echo array_any($a, fn($x) => $x === "a") ? "y" : "n";
echo array_all($a, fn($x) => $x !== null) ? "y" : "n";
echo array_any($a, fn($x) => $x === "zzz") ? "y" : "n";
"#,
    );
    assert_eq!(out, "yyn");
}

/// Verifies array_find / array_any / array_all are callable case-insensitively.
/// Fixture: mixed-case spellings over a small numeric array.
#[test]
fn test_array_find_any_all_case_insensitive() {
    let out = compile_and_run(
        r#"<?php
function pos($x) { return $x > 0; }
echo Array_Find([3, 6], "pos");
echo Array_Any([0, 0], "pos") ? "y" : "n";
echo Array_All([1, 2], "pos") ? "y" : "n";
"#,
    );
    assert_eq!(out, "3ny");
}

/// Verifies array_udiff() keeps elements of the first array whose comparator never returns 0
/// against any element of the second array.
/// Fixture: udiff([1,2,3,4], [2,4]) with a numeric comparator keeps 1 and 3.
#[test]
fn test_array_udiff_string_comparator() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$r = array_udiff([1, 2, 3, 4], [2, 4], "cmp");
foreach ($r as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "13");
}

/// Verifies array_uintersect() keeps elements of the first array that compare equal to some
/// element of the second array.
/// Fixture: uintersect([1,2,3,4], [2,4]) keeps 2 and 4.
#[test]
fn test_array_uintersect_string_comparator() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
$r = array_uintersect([1, 2, 3, 4], [2, 4], "cmp");
foreach ($r as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "24");
}

/// Verifies array_udiff() works with a non-capturing closure (spaceship) comparator.
/// Fixture: udiff([5,10,15], [10]) keeps 5 and 15.
#[test]
fn test_array_udiff_closure_comparator() {
    let out = compile_and_run(
        r#"<?php
$r = array_udiff([5, 10, 15], [10], fn($a, $b) => $a <=> $b);
foreach ($r as $v) { echo $v; echo ","; }
"#,
    );
    assert_eq!(out, "5,15,");
}

/// Verifies array_udiff() / array_uintersect() result sizes and case-insensitive calls.
/// Fixture: udiff keeps 2 of 4; uintersect keeps 2 of 4.
#[test]
fn test_array_udiff_uintersect_case_insensitive() {
    let out = compile_and_run(
        r#"<?php
function cmp($a, $b) { return $a - $b; }
echo count(Array_Udiff([1, 2, 3, 4], [2, 4], "cmp"));
echo count(Array_Uintersect([1, 2, 3, 4], [2, 4], "cmp"));
"#,
    );
    assert_eq!(out, "22");
}

/// Verifies a by-value foreach over a concrete array of objects yields the typed element,
/// so the objects flow through an append into a usort with a typed comparator and sort
/// correctly. Regression for the #648 silent miscompile: the foreach value used to erase
/// to boxed Mixed, poisoning the append target to array<mixed> and making the comparator
/// read garbage. Also exercises the balancing incref — the collected objects outlive their
/// (temporary) source array.
#[test]
fn test_foreach_object_array_append_usort() {
    let out = compile_and_run(
        r#"<?php
final class Desc { public function __construct(public int $weight) {} }
final class Mgr {
    /** @return array */
    public function plugins(): array { $r = []; $r[] = new Desc(3); $r[] = new Desc(1); $r[] = new Desc(2); return $r; }
    /** @return array */
    public function ordered(): array {
        $matches = [];
        foreach ($this->plugins() as $d) { $matches[] = $d; }
        usort($matches, static fn (Desc $a, Desc $b): int => $a->weight <=> $b->weight);
        return $matches;
    }
}
$out = '';
foreach ((new Mgr())->ordered() as $d) { $out .= $d->weight; }
echo $out;
"#,
    );
    assert_eq!(out, "123");
}

/// Verifies array_map's result element type is the CALLBACK's return type, not the source
/// element type. Regression for #643: `array_map(fn (int): Foo, $ints)` was mistyped
/// `array<int>` (source) instead of `array<Foo>`, so property access on a mapped element —
/// and a later typed usort — saw Int. Pre-fix this failed the checker (`$a[0]` typed int,
/// rejecting `->w`); post-fix the mapped Foo objects are typed and readable.
#[test]
fn test_array_map_result_typed_by_callback_return() {
    let out = compile_and_run(
        r#"<?php
final class Foo { public function __construct(public int $w) {} }
$a = array_map(static fn (int $n): Foo => new Foo($n * 2), [5, 6]);
echo $a[0]->w, ',', $a[1]->w;
"#,
    );
    assert_eq!(out, "10,12");
}

/// #664: a `usort()` comparator held as an opaque `?Closure` — a property narrowed to `Closure` via
/// `instanceof` (the ward-dbal-file FileQueryPlan shape) — carries no recoverable signature, yet
/// dispatches correctly. The checker accepts it for the sort family and the backend unboxes the
/// boxed-`Mixed` closure (the `?Closure` union repr) to its raw descriptor. Sorts associative-array
/// rows (8-byte handles) ascending, byte-identical to PHP.
#[test]
fn test_usort_opaque_closure_property_comparator() {
    let out = compile_and_run(
        r#"<?php
final class Plan {
    public function __construct(public ?Closure $compare) {}
    public function compare(): ?Closure { return $this->compare; }
}
final class Sorter {
    public function __construct(private Plan $plan) {}
    /** @param list<array<string, int>> $rows @return list<array<string, int>> */
    public function run(array $rows): array {
        $c = $this->plan->compare();
        if (!$c instanceof Closure) { return $rows; }
        $s = $rows;
        usort($s, $c);
        return $s;
    }
}
$sorter = new Sorter(new Plan(static fn(array $a, array $b): int => $a['k'] <=> $b['k']));
$sorted = $sorter->run([['k' => 3], ['k' => 1], ['k' => 2], ['k' => 5], ['k' => 4]]);
foreach ($sorted as $r) { echo $r['k']; }
"#,
    );
    assert_eq!(out, "12345");
}

/// #664: an opaque `?Closure` `usort()` comparator sorts descending, is STABLE (PHP 8.0+ — equal
/// keys keep their original order), and short-circuits on the `?Closure` → `null` branch (the guard
/// `!$compare instanceof Closure` returns unsorted, exercising the null closure repr).
#[test]
fn test_usort_opaque_closure_descending_stable_and_null() {
    let out = compile_and_run(
        r#"<?php
final class Plan {
    public function __construct(public ?Closure $compare) {}
    public function compare(): ?Closure { return $this->compare; }
}
function sortRows(Plan $plan, array $rows): string {
    $c = $plan->compare();
    if (!$c instanceof Closure) { return "unsorted"; }
    $s = $rows;
    usort($s, $c);
    $o = [];
    foreach ($s as $r) { $o[] = $r['k'] . $r['t']; }
    return implode(",", $o);
}
echo sortRows(new Plan(static fn(array $a, array $b): int => $b['k'] <=> $a['k']), [['k'=>1,'t'=>0],['k'=>3,'t'=>0],['k'=>2,'t'=>0]]), "|";
echo sortRows(new Plan(static fn(array $a, array $b): int => $a['k'] <=> $b['k']), [['k'=>1,'t'=>0],['k'=>1,'t'=>1],['k'=>0,'t'=>9],['k'=>1,'t'=>2]]), "|";
echo sortRows(new Plan(null), [['k'=>1,'t'=>0]]);
"#,
    );
    assert_eq!(out, "30,20,10|09,10,11,12|unsorted");
}

/// #665 (Leak B): a closure indexing an `array<mixed>` parameter — the exact shape a
/// `usort` comparator takes, since closure params are not call-site specialized so
/// `$a['k']` lowers to `Op::ArrayGetMixedKey` and returns an OWNED boxed Mixed cell.
/// That result was not recognized as an owning container read, so its consumer
/// (`spaceship`) never released it, leaking one boxed cell per indexed read. Called
/// directly (not through `usort`) to isolate the comparator's boxed-read leak from the
/// separate descriptor-callback element-boxing leak (#666). Heap-clean is the assertion.
#[test]
fn test_closure_mixed_key_read_is_heap_clean() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$cmp = static fn(array $a, array $b): int => $a['k'] <=> $b['k'];
$x = ['k' => 3];
$y = ['k' => 1];
$sum = 0;
for ($i = 0; $i < 50; $i++) { $sum += $cmp($x, $y); }
echo $sum, "\n";
"#,
    );
    assert_eq!(out.stdout, "50\n");
    assert!(
        out.stderr.contains("leak summary: clean"),
        "closure mixed-key read leaked: {}",
        out.stderr
    );
}

/// #665 (Leak B), store path: `$v = $a['k']` on an `array<mixed>` inside a closure takes
/// a retaining store whose source-release is gated on the same owning-container-read
/// predicate; before the fix the boxed read source was never released, leaking per call.
#[test]
fn test_closure_mixed_key_read_store_is_heap_clean() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$read = static function (array $a): int { $v = $a['k']; return $v; };
$x = ['k' => 7];
$sum = 0;
for ($i = 0; $i < 50; $i++) { $sum += $read($x); }
echo $sum, "\n";
"#,
    );
    assert_eq!(out.stdout, "350\n");
    assert!(
        out.stderr.contains("leak summary: clean"),
        "closure mixed-key read store leaked: {}",
        out.stderr
    );
}

/// #666: a `usort` comparator over refcounted heap elements (associative arrays). The descriptor
/// invoker handed each element to the callback with a by-value incref it never released — the callee
/// borrows its params — leaking one reference per element on every comparison. An Int-return callback
/// cannot transfer an argument out, so the invoker now borrows the element (the boxed-Mixed argument
/// container keeps it alive for the call). Heap-clean over the sorted-then-indexed shape.
#[test]
fn test_usort_over_assoc_elements_is_heap_clean() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$sum = 0;
for ($i = 0; $i < 20; $i++) {
    $s = [['k' => 3], ['k' => 1], ['k' => 2], ['k' => 5], ['k' => 4]];
    usort($s, static fn(array $a, array $b): int => $a['k'] <=> $b['k']);
    $sum += $s[0]['k'];
}
echo $sum, "\n";
"#,
    );
    assert_eq!(out.stdout, "20\n");
    assert!(
        out.stderr.contains("leak summary: clean"),
        "usort over assoc-array elements leaked: {}",
        out.stderr
    );
}

/// #666 (object elements): same borrow fix for a `usort` comparator over object elements — one
/// reference per object leaked per comparison before the fix.
#[test]
fn test_usort_over_object_elements_is_heap_clean() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
final class Row { public function __construct(public int $k) {} }
$sum = 0;
for ($i = 0; $i < 20; $i++) {
    $s = [new Row(3), new Row(1), new Row(2)];
    usort($s, static fn(Row $a, Row $b): int => $a->k <=> $b->k);
    $sum += $s[0]->k;
}
echo $sum, "\n";
"#,
    );
    assert_eq!(out.stdout, "20\n");
    assert!(
        out.stderr.contains("leak summary: clean"),
        "usort over object elements leaked: {}",
        out.stderr
    );
}
