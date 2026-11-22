//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of indexed array array set-operation builtins, including unique, diff, and intersect.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Verifies `array_diff()` and `array_intersect()` take string lists and keep the original key
/// of every survivor, as PHP does (AIC #245).
///
/// A string list has 16-byte slots, which the native raw-slot helpers refused at compile time
/// (`array_diff indexed-array element PHP type Str`), and on an int list they renumbered the
/// result: `array_diff([10, 20, 30], [20])` answered keys `0, 1` where PHP keeps `0, 2`.
#[test]
fn test_value_set_builtins_take_string_lists_and_keep_keys() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
function show(array $r): void { foreach ($r as $k => $v) { echo $k, "=", $v, ","; } echo "\n"; }
show(array_diff(["a", "b", "c", "b"], ["b"]));
show(array_intersect(["a", "b", "c", "b"], ["c", "b"]));
show(array_diff([10, 20, 30, 40], [20]));
show(array_intersect([10, 20, 30, 40], [40, 20]));
show(array_diff(["k" => "v", "j" => "w", 7 => "v"], ["w"]));
$d = array_diff([10, 20, 30], [20]);
var_dump(isset($d[1]), $d[2] ?? "missing");
"#,
    );
    assert!(out.success, "stdout={:?}\nstderr={}", out.stdout, out.stderr);
    assert_eq!(
        out.stdout,
        "0=a,2=c,\n1=b,2=c,3=b,\n0=10,2=30,3=40,\n1=20,3=40,\nk=v,7=v,\nbool(false)\nint(30)\n"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Verifies the value-set builtins compare boxed values by their string rendering, as PHP's
/// `(string) $a === (string) $b` does (AIC #245).
///
/// Mixed lists used to be refused (`compares boxed elements by identity`): the native helpers
/// compared cell addresses. `1`, `"1"`, `1.0` and `true` render alike while `"01"` and `"1.0"`
/// do not, `null`, `false` and `""` render alike, a `Stringable` object renders through
/// `__toString()`, and an object without one throws PHP's `Error`.
#[test]
fn test_value_set_builtins_compare_mixed_values_by_string() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
final class Tag {
    public function __construct(private string $name) {}
    public function __toString(): string { return $this->name; }
}
echo json_encode(array_diff([1, "1", "01", "1.0", 1.0, 1.5, "1.5", true, false, null, ""], ["1", ""])), "\n";
echo json_encode(array_intersect([1, "1", "01", "1.0", 1.0, 1.5, "1.5", true, false, null, ""], [1])), "\n";
echo json_encode(array_diff([1, "b", 3, 4], [3, "z"])), "\n";
echo json_encode(array_intersect([1, "b", 3, 4], [3, "z", "1"])), "\n";
echo json_encode(array_unique([1, "1", 2, 2.0, "x", "x"])), "\n";
echo json_encode(array_unique([0.1 + 0.2, 0.3, -0.0, 0.0, 1.0, "1"])), "\n";
echo json_encode(array_diff([new Tag("a"), new Tag("b"), "c"], ["a", new Tag("c")])), "\n";
echo json_encode(array_diff_assoc(["x", "y", "z"], [1 => "y", 2 => "Z"])), "\n";
echo json_encode(array_intersect_assoc(["a" => "1", "b" => 2, 0 => "z"], ["a" => 1, "b" => "3", 0 => "z"])), "\n";
try {
    array_diff([new ArrayIterator([])], ["x"]);
} catch (Error $e) {
    echo get_class($e), ": ", $e->getMessage(), "\n";
}
"#,
    );
    assert!(out.success, "stdout={:?}\nstderr={}", out.stdout, out.stderr);
    assert_eq!(
        out.stdout,
        concat!(
            "{\"2\":\"01\",\"3\":\"1.0\",\"5\":1.5,\"6\":\"1.5\"}\n",
            "{\"0\":1,\"1\":\"1\",\"4\":1,\"7\":true}\n",
            "{\"0\":1,\"1\":\"b\",\"3\":4}\n",
            "{\"0\":1,\"2\":3}\n",
            "{\"0\":1,\"2\":2,\"4\":\"x\"}\n",
            "{\"0\":0.30000000000000004,\"2\":-0,\"3\":0,\"4\":1}\n",
            "{\"1\":{}}\n",
            "{\"0\":\"x\",\"2\":\"z\"}\n",
            "{\"a\":\"1\",\"0\":\"z\"}\n",
            "Error: Object of class ArrayIterator could not be converted to string\n",
        )
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Verifies the set builtins take operands declared only `array`: parameters, a promoted
/// property, a class constant and a static property (AIC #245).
///
/// A declared `array` is neither a list nor a hash to the checker, so `array_diff()` and
/// `array_intersect()` answered `first argument must be array`, `array_unique()` `argument must
/// be array` and the `_assoc` pair `must be associative arrays`. `array_diff_key()` and
/// `array_intersect_key()` already had a prelude for it, which a parameter missed: the
/// checker resolves signatures lazily, so the prelude function had none yet when the caller's
/// body was checked.
#[test]
fn test_set_builtins_take_operands_declared_array() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
final class Fleet {
    private const array ALL = ["V1", "V2", "V3"];
    public static array $sold = ["V2"];
    public function __construct(public array $vins) {}
    public function missing(array $seen): array { return array_diff($this->vins, $seen); }
    public function unsold(): array { return array_diff(self::ALL, self::$sold); }
}
function diff(array $a, array $b): array { return array_diff($a, $b); }
function both(array $a, array $b): array { return array_intersect($a, $b); }
function uniq(array $a): array { return array_unique($a); }
function dkey(array $a, array $b): array { return array_diff_key($a, $b); }
function ikey(array $a, array $b): array { return array_intersect_key($a, $b); }
function dassoc(array $a, array $b): array { return array_diff_assoc($a, $b); }
function iassoc(array $a, array $b): array { return array_intersect_assoc($a, $b); }
$fleet = new Fleet(["V1", "V2", "V3"]);
echo json_encode($fleet->missing(["V2"])), json_encode($fleet->unsold()), "\n";
echo json_encode(diff([1, 2, 3], [2])), json_encode(both(["a", "b"], ["b"])), json_encode(uniq([1, "1", 2])), "\n";
echo json_encode(dkey(["a" => 1, "b" => 2], ["a" => 0])), json_encode(ikey(["a" => 1, "b" => 2], ["a" => 0])), "\n";
echo json_encode(dassoc(["a" => "1", "b" => 2], ["a" => 1])), json_encode(iassoc(["a" => "1", "b" => 2], ["a" => 1])), "\n";
"#,
    );
    assert!(out.success, "stdout={:?}\nstderr={}", out.stdout, out.stderr);
    assert_eq!(
        out.stdout,
        concat!(
            "{\"0\":\"V1\",\"2\":\"V3\"}{\"0\":\"V1\",\"2\":\"V3\"}\n",
            "{\"0\":1,\"2\":3}{\"1\":\"b\"}{\"0\":1,\"2\":2}\n",
            "{\"b\":2}{\"a\":1}\n",
            "{\"b\":2}{\"a\":\"1\"}\n",
        )
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Verifies repeated value-set calls over strings, boxed values and objects release every
/// rendering and result (AIC #245). The native `_assoc` helper leaked about a dozen blocks a
/// call on x86_64; the prelude it now goes through leaks none.
#[test]
fn test_value_set_builtins_release_renderings_in_a_loop() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
final class Tag {
    public function __construct(private string $name) {}
    public function __toString(): string { return $this->name; }
}
function diff(array $a, array $b): array { return array_diff($a, $b); }
$total = 0;
for ($n = 0; $n < 20; $n++) {
    $s = ["a" . $n, "b", "c" . $n];
    $total += count(array_diff($s, ["b"])) + count(array_intersect($s, ["b", "c" . $n]));
    $total += count(array_unique([$n, (string) $n, "z", "z"])) + count(diff($s, ["c" . $n]));
    $total += count(array_diff([new Tag("t" . $n), "x"], ["x"]));
    $total += count(array_diff_assoc(["a", "b" . $n], [1 => "b"]));
    $total += count(array_intersect_assoc(["k" => "v" . $n, "j" => 1], ["k" => "v" . $n, "j" => "1"]));
}
echo $total, "\n";
"#,
    );
    assert!(out.success, "stdout={:?}\nstderr={}", out.stdout, out.stderr);
    assert_eq!(out.stdout, "260\n");
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Verifies typed lists still answer PHP's values through `array_diff`, `array_intersect` and
/// `array_unique`, and that `array_reverse` and `array_merge`, which share the 8-byte element
/// gate but never compare, still accept a boxed array.
#[test]
fn test_value_comparing_builtins_still_accept_typed_elements() {
    let out = compile_and_run(
        r#"<?php
echo implode(",", array_diff([1, 2, 3], [2])), "|";
echo implode(",", array_intersect([1, 2, 3], [2, 3])), "|";
echo implode(",", array_unique([1, 2, 2, 3])), "|";
$boxed = [1, "b", 3];
$more = [9, "z"];
echo implode(",", array_reverse($boxed)), "|";
echo implode(",", array_merge($boxed, $more));
"#,
    );
    assert_eq!(out, "1,3|2,3|1,2,3|3,b,1|1,b,3,9,z");
}

/// Verifies `array_unique()` removes duplicate values; count of `[1,2,2,3,3,3]` is 3.
#[test]
fn test_array_unique() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 2, 3, 3, 3];
$b = array_unique($a);
echo count($b);
"#,
    );
    assert_eq!(out, "3");
}

/// Verifies `array_diff()` returns values from `$a` not present in `$b`; count of `[1,2,3,4]` vs `[2,4]` is 2.
#[test]
fn test_array_diff() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 3, 4];
$b = [2, 4];
$c = array_diff($a, $b);
echo count($c);
"#,
    );
    assert_eq!(out, "2");
}

/// Verifies `array_intersect()` returns values present in both `$a` and `$b`; count of `[1,2,3,4]` vs `[2,4,6]` is 2.
#[test]
fn test_array_intersect() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 3, 4];
$b = [2, 4, 6];
$c = array_intersect($a, $b);
echo count($c);
"#,
    );
    assert_eq!(out, "2");
}

/// Verifies `array_rand()` returns a valid key/index within the array bounds `[0, 3)`.
#[test]
fn test_array_rand() {
    let out = compile_and_run(
        r#"<?php
$a = [10, 20, 30];
$i = array_rand($a);
if ($i >= 0 && $i < 3) { echo "ok"; }
"#,
    );
    assert_eq!(out, "ok");
}

/// Verifies `shuffle()` permutes all elements without losing any; count stays 5, sum stays 15.
#[test]
fn test_shuffle() {
    let out = compile_and_run(
        r#"<?php
$a = [1, 2, 3, 4, 5];
shuffle($a);
echo count($a);
echo array_sum($a);
"#,
    );
    assert_eq!(out, "515");
}

/// Verifies `array_diff_key()` removes entries by key; count of `["a"=>"1","b"=>"2"]` minus key "a" is 1.
#[test]
fn test_array_diff_key() {
    let out = compile_and_run(
        r#"<?php
$a = ["a" => "1", "b" => "2"];
$b = ["a" => "9"];
$c = array_diff_key($a, $b);
echo count($c);
"#,
    );
    assert_eq!(out, "1");
}

/// Regression: verifies borrowed arrays inside `$src` are not freed when `$src` is unset after `array_diff_key()`.
#[test]
fn test_gc_array_diff_key_borrowed_array_survives_source_unset() {
    let out = compile_and_run(
        r#"<?php
$src = ["keep" => [1, 2], "drop" => [3, 4]];
$mask = ["drop" => 1];
$filtered = array_diff_key($src, $mask);
unset($src);
$saved = $filtered["keep"];
echo $saved[1];
"#,
    );
    assert_eq!(out, "2");
}

/// Verifies `array_intersect_key()` keeps only entries with matching keys; count of `["a"=>"1","b"=>"2"]` intersecting key "a" is 1.
#[test]
fn test_array_intersect_key() {
    let out = compile_and_run(
        r#"<?php
$a = ["a" => "1", "b" => "2"];
$b = ["a" => "9"];
$c = array_intersect_key($a, $b);
echo count($c);
"#,
    );
    assert_eq!(out, "1");
}

/// Regression: verifies borrowed arrays inside `$src` are not freed when `$src` is unset after `array_intersect_key()`.
#[test]
fn test_gc_array_intersect_key_borrowed_array_survives_source_unset() {
    let out = compile_and_run(
        r#"<?php
$src = ["keep" => [5, 6], "drop" => [7, 8]];
$mask = ["keep" => 1];
$filtered = array_intersect_key($src, $mask);
unset($src);
$saved = $filtered["keep"];
echo $saved[0] . "|" . $saved[1];
"#,
    );
    assert_eq!(out, "5|6");
}
