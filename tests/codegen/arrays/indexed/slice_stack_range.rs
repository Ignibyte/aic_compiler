//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of indexed array array slicing, stack, and range builtins, including slice, shift, and shift empty.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Tests `array_slice($a, $offset, $length)` with a 5-element array, offset 1, length 3.
/// Verifies correct sub-sequence extraction (20 30 40) and that indices map correctly.
#[test]
fn test_array_slice() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30, 40, 50];
$b = array_slice($a, 1, 3);
echo $b[0] . " " . $b[1] . " " . $b[2];
"#,
    );
    assert_eq!(out, "20 30 40");
}

/// Regression for #648: `array_slice` over a `string`-element array must copy the 16-byte string
/// descriptors AND persist each into an owned buffer — a raw descriptor copy would share the
/// source's array-owned strings and double-free them when the slice is released, corrupting the
/// source. Covers a positive slice, a to-end slice, a negative offset, and — critically — that the
/// SOURCE array remains intact after the slice temporaries are gone.
#[test]
fn test_array_slice_string_elements() {
    let out = compile_and_run(
        r#"<?php
function j(array $a): string { return implode(",", $a); }
$a = ["a", "b", "c", "d", "e"];
echo j(array_slice($a, 1, 2)), "|";   // b,c
echo j(array_slice($a, 2)), "|";      // c,d,e  (to end)
echo j(array_slice($a, -2)), "|";     // d,e    (negative offset)
echo j($a);                            // a,b,c,d,e  (source intact — no double-free)
"#,
    );
    assert_eq!(out, "b,c|c,d,e|d,e|a,b,c,d,e");
}

/// Tests `array_shift` removes and returns the first element from a 3-element array.
/// Verifies the popped value (10) and that remaining array length is reduced to 2.
#[test]
fn test_array_shift() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
$first = array_shift($a);
echo $first . " " . count($a);
"#,
    );
    assert_eq!(out, "10 2");
}

/// Tests `array_shift` on a single-element array, then on an already-empty array.
/// Verifies that calling shift on empty returns empty string (no output).
#[test]
fn test_array_shift_empty() {
    let out = compile_and_run("<?php $a = [1]; array_shift($a); echo array_shift($a);");
    assert_eq!(out, "");
}

/// Tests `array_unshift` prepends a value to an array and returns the new count.
/// Verifies new length is returned (3) and that the prepended element is at index 0.
#[test]
fn test_array_unshift() {
    let out = compile_and_run(
        r#"<?php
$a = [2, 3];
$n = array_unshift($a, 1);
echo $n . " " . $a[0];
"#,
    );
    assert_eq!(out, "3 1");
}

/// Tests `range($start, $end)` with ascending values (1 to 5).
/// Verifies correct count (5) and iteration order (12345).
#[test]
fn test_range() {
    let out = compile_and_run(
        r#"<?php
$a = range(1, 5);
echo count($a) . ":";
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "5:12345");
}

/// Tests `range` with start greater than end (5 down to 1), verifying descending order.
/// Verifies correct count (5) and iteration order (54321).
#[test]
fn test_range_descending() {
    let out = compile_and_run(
        r#"<?php
$a = range(5, 1);
echo count($a) . ":";
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "5:54321");
}

/// Tests `range($start, $end)` when start equals end (3 to 3).
/// Verifies a single-element array is produced with count 1 and value 3.
#[test]
fn test_range_single_element() {
    let out = compile_and_run(
        r#"<?php
$a = range(3, 3);
echo count($a) . ":";
foreach ($a as $v) { echo $v; }
"#,
    );
    assert_eq!(out, "1:3");
}

/// Regression: `range()` and `array_slice()` must unbox a `Mixed`/`Union` integer argument
/// (range end, slice offset) instead of using the boxed heap pointer as a raw int. The int args
/// here are read from a heterogeneous (Mixed-valued) associative array. Before the fix these
/// produced empty results or "heap memory exhausted" (a pointer used as a count).
#[test]
fn test_range_and_slice_unbox_mixed_int_args() {
    let out = compile_and_run(
        r#"<?php
$m = ["n" => 2, "t" => "x"];
echo implode(",", range(1, $m["n"])), "|", implode(",", array_slice([10, 20, 30, 40], $m["n"]));
"#,
    );
    assert_eq!(out, "1,2|30,40");
}

/// Regression: the shared slice/splice/range argument marshaling must unbox a `Mixed` length and a
/// `Mixed` offset on `array_splice` (which mutates its source), and unbox both endpoints of
/// `range()`. The integers are read from a heterogeneous (`Mixed`-valued) associative array, so the
/// boxed-pointer-as-int bug would corrupt the offset, length, removed slice, and remaining array.
#[test]
fn test_slice_splice_range_unbox_mixed_offset_and_length() {
    let out = compile_and_run(
        r#"<?php
$m = ["off" => 1, "len" => 2, "t" => "x"];
echo implode(",", array_slice([10, 20, 30, 40, 50], $m["off"], $m["len"])), "|";
$a = [1, 2, 3, 4, 5];
$removed = array_splice($a, $m["off"], $m["len"]);
echo implode(",", $removed), "|", implode(",", $a), "|";
echo implode(",", range($m["off"], $m["len"]));
"#,
    );
    assert_eq!(out, "20,30|2,3|1,4,5|1,2");
}

/// Regression: when the *array itself* is a boxed `Mixed` cell (read from a heterogeneous associative
/// array), `array_slice`/`array_splice` must still unbox a `Mixed` offset and a `Mixed` length instead
/// of passing the boxed heap pointer as a raw integer. A `Mixed` length previously hard-errored at
/// codegen ("array_slice length PHP type Mixed") and a `Mixed` offset silently corrupted the result.
/// Covers offset+length both Mixed (slice), offset Mixed with length absent (slice), and offset+length
/// both Mixed with source mutation (splice).
#[test]
fn test_mixed_array_slice_splice_unbox_mixed_offset_and_length() {
    let out = compile_and_run(
        r#"<?php
$d = ["arr" => [10, 20, 30, 40, 50], "off" => 1, "len" => 2];
$a = $d["arr"];
echo implode(",", array_slice($a, $d["off"], $d["len"])), "|";
$d2 = ["arr" => [10, 20, 30, 40, 50], "off" => 2];
$a2 = $d2["arr"];
echo implode(",", array_slice($a2, $d2["off"])), "|";
$d3 = ["arr" => [1, 2, 3, 4, 5], "off" => 1, "len" => 2];
$a3 = $d3["arr"];
$removed = array_splice($a3, $d3["off"], $d3["len"]);
echo implode(",", $removed), "|", implode(",", $a3);
"#,
    );
    assert_eq!(out, "20,30|30,40,50|2,3|1,4,5");
}

/// Regression for #644: a `?int` (tagged-scalar) `$length` must pick between "run to the end"
/// (null) and its payload word (int) at runtime. This is `FileStatement::applyLimitOffset`'s shape
/// — `$plan->limit()` returns `?int` — which previously aborted codegen with
/// "array_slice length for PHP type TaggedScalar". The last case pins that a NEGATIVE `?int`
/// length still counts back from the end rather than being mistaken for the to-end marker.
#[test]
fn test_array_slice_nullable_int_length() {
    let out = compile_and_run(
        r#"<?php
final class Plan {
    public function __construct(private ?int $lim) {}
    public function limit(): ?int { return $this->lim; }
}
$rows = [1, 2, 3, 4, 5];
$none = new Plan(null);
$two = new Plan(2);
$zero = new Plan(0);
$back = new Plan(-1);
echo implode(",", array_slice($rows, 1, $none->limit())), "|";
echo implode(",", array_slice($rows, 1, $two->limit())), "|";
echo implode(",", array_slice($rows, 1, $zero->limit())), "|";
echo implode(",", array_slice($rows, 1, $back->limit()));
"#,
    );
    assert_eq!(out, "2,3,4,5|2,3||2,3,4");
}

/// Regression for #644: a NEGATIVE `array_slice` length stops that many elements short of the end
/// (`array_slice([1..5], 0, -1) === [1,2,3,4]`). The helpers previously used `-1` as their
/// "run to the end" marker, so a legal `-1` silently returned the whole remainder and `-2` built a
/// negative-capacity array (heap exhaustion). The marker is now `i64::MIN`, which no PHP length
/// can collide with. The final case pins that the to-end path still works.
#[test]
fn test_array_slice_negative_length() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 3, 4, 5];
echo implode(",", array_slice($a, 0, -1)), "|";
echo implode(",", array_slice($a, 0, -2)), "|";
echo implode(",", array_slice($a, 1, -1)), "|";
echo implode(",", array_slice($a, 0, -9)), "|";
echo implode(",", array_slice($a, 1));
"#,
    );
    assert_eq!(out, "1,2,3,4|1,2,3|2,3,4||2,3,4,5");
}

/// Regression for #644: the same negative-length rule over `string` elements and through
/// `array_splice`, whose removal length shares the normalization. Pins that a negative removal
/// length leaves the correct remainder in the source array.
#[test]
fn test_array_slice_splice_negative_length_string_and_source() {
    let out = compile_and_run(
        r#"<?php
$s = ["a", "b", "c", "d", "e"];
echo implode(",", array_slice($s, 0, -2)), "|";
$n = [1, 2, 3, 4, 5];
$removed = array_splice($n, 1, -2);
echo implode(",", $removed), "|", implode(",", $n);
"#,
    );
    assert_eq!(out, "a,b,c|2,3|1,4,5");
}
