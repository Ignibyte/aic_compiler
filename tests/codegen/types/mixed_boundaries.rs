//! Purpose:
//! Integration tests for a boxed `mixed` value at a declared `array`, `?array` or class-typed
//! parameter or return boundary: accepted values pass, and every other runtime type throws the
//! `TypeError` php-src throws.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The source values come from `json_decode()` and `mixed` parameters, so the checker only
//!   knows them as `mixed` and the run-time guard decides.
//! - php appends `, called in <file> on line <n>` to an argument `TypeError`; the fixtures cut
//!   it off, as elephc's other parameter guards leave it out.

use crate::support::*;

/// Verifies `mixed` values pass declared `array`, `?array`, interface, `?interface` and class
/// parameters and returns, by value and by reference, through functions, constructors and
/// static methods.
#[test]
fn test_mixed_value_passes_array_and_class_boundaries() {
    let out = compile_and_run(
        r#"<?php
interface Shape { public function area(): int; }
final class Square implements Shape { public function __construct(private int $s) {} public function area(): int { return $this->s * $this->s; } }
final class Bag { public function __construct(public array $items) {} public function size(): int { return count($this->items); } public static function sum(array $xs): int { return array_sum($xs); } }
function count2(array $a): int { return count($a); }
function maybe(?array $a): string { return $a === null ? "none" : implode(',', $a); }
function area(Shape $s): int { return $s->area(); }
function areaOrZero(?Shape $s): int { return $s === null ? 0 : $s->area(); }
function names(mixed $raw): array { return $raw; }
function shape(mixed $raw): Shape { return $raw; }
function grow(array &$a): void { $a[] = 9; }
function relayArea(mixed $m): int { return area($m) + areaOrZero($m) + shape($m)->area(); }
function relayNull(mixed $m): int { return areaOrZero($m); }
$data = json_decode('{"tags":["a","b"],"nums":[1,2,3],"map":{"k":"v"},"none":null}', true);
echo count2($data['tags']), "|", count2($data['map']), "|", maybe($data['none']), "|", maybe($data['tags']), "\n";
echo implode(',', names($data['nums'])), "|", count(names($data['map'])), "\n";
echo (new Bag($data['nums']))->size(), "|", Bag::sum($data['nums']), "\n";
echo relayArea(new Square(2)), "|", relayNull(null), "\n";
$box = $data['nums'];
grow($box);
echo count($box), "\n";
"#,
    );
    assert_eq!(out, "2|1|none|a,b\n1,2,3|1\n3|6\n12|0\n4\n");
}

/// Verifies a `mixed` value of the wrong runtime type throws the boundary's catchable
/// `TypeError`, worded as php-src words it.
#[test]
fn test_mixed_value_of_the_wrong_type_throws_the_boundary_type_error() {
    let out = compile_and_run(
        r#"<?php
final class Square { }
final class Circle { }
function count2(array $a): int { return count($a); }
function maybe(?array $a): int { return 0; }
function take(Square $s): int { return 1; }
function names(mixed $raw): array { return $raw; }
function square(mixed $raw): Square { return $raw; }
function why(TypeError $e): string { return explode(", called in", $e->getMessage())[0]; }
function viaCount(mixed $v): string { try { count2($v); return "ok"; } catch (TypeError $e) { return why($e); } }
function viaMaybe(mixed $v): string { try { maybe($v); return "ok"; } catch (TypeError $e) { return why($e); } }
function viaTake(mixed $v): string { try { take($v); return "ok"; } catch (TypeError $e) { return why($e); } }
function viaNames(mixed $v): string { try { names($v); return "ok"; } catch (TypeError $e) { return why($e); } }
function viaSquare(mixed $v): string { try { square($v); return "ok"; } catch (TypeError $e) { return why($e); } }
echo viaCount(5), "\n", viaCount("s"), "\n", viaCount(1.5), "\n", viaCount(true), "\n", viaCount(false), "\n";
echo viaCount(null), "\n", viaCount(new Circle()), "\n", viaCount([1]), "\n";
echo viaMaybe(5), "\n", viaMaybe(null), "\n";
echo viaTake(new Circle()), "\n", viaTake([1]), "\n", viaTake(new Square()), "\n";
echo viaNames(7), "\n", viaSquare(new Circle()), "\n", viaSquare(null), "\n", viaSquare(new Square()), "\n";
"#,
    );
    assert_eq!(out, "count2(): Argument #1 ($a) must be of type array, int given\ncount2(): Argument #1 ($a) must be of type array, string given\ncount2(): Argument #1 ($a) must be of type array, float given\ncount2(): Argument #1 ($a) must be of type array, true given\ncount2(): Argument #1 ($a) must be of type array, false given\ncount2(): Argument #1 ($a) must be of type array, null given\ncount2(): Argument #1 ($a) must be of type array, Circle given\nok\nmaybe(): Argument #1 ($a) must be of type ?array, int given\nok\ntake(): Argument #1 ($s) must be of type Square, Circle given\ntake(): Argument #1 ($s) must be of type Square, array given\nok\nnames(): Return value must be of type array, int returned\nsquare(): Return value must be of type Square, Circle returned\nsquare(): Return value must be of type Square, null returned\nok\n");
}

/// Verifies guarded `mixed` arguments and returns leave the heap clean, owned temporaries
/// included.
#[test]
fn test_mixed_boundary_guards_leave_the_heap_clean() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
final class Square { public function __construct(public int $s) {} }
function total(array $a): int { return array_sum($a); }
function pick(mixed $m): Square { return $m; }
function keep(mixed $m): array { return $m; }
function wrap(mixed $m): int { return pick($m)->s; }
$sum = 0;
for ($i = 0; $i < 50; $i++) {
    $sum += total(json_decode('[1,2,3]', true));
    $sum += count(keep(json_decode('{"a":1,"b":2}', true)));
    $sum += wrap(new Square($i));
}
echo $sum;
"#,
    );
    assert!(out.success, "program failed: {}", out.stderr);
    assert_eq!(out.stdout, "1625");
    assert!(
        out.stderr.contains("HEAP DEBUG: leak summary: clean"),
        "expected guarded mixed boundaries to leave a clean heap, got: {}",
        out.stderr
    );
}

/// Verifies the bare `object` type accepts every object from a `mixed` value and refuses the
/// rest, as a parameter and as a return.
#[test]
fn test_mixed_value_at_a_bare_object_boundary() {
    let out = compile_and_run(
        r#"<?php
function f(object $o): object { return $o; }
function r(mixed $m): object { return $m; }
function g(mixed $m): string { try { f($m); return "ok"; } catch (TypeError $e) { return explode(", called in", $e->getMessage())[0]; } }
function h(mixed $m): string { try { r($m); return "ok"; } catch (TypeError $e) { return $e->getMessage(); } }
echo g(new stdClass()), "|", g(5), "|", h(new stdClass()), "|", h("s"), "\n";
"#,
    );
    assert_eq!(
        out,
        "ok|f(): Argument #1 ($o) must be of type object, int given|ok|r(): Return value must be of type object, string returned\n"
    );
}
