//! Purpose:
//! Integration tests for a `mixed` argument passed to a declared scalar or scalar-union
//! parameter of a user function, method, constructor or closure: php-src checks it when the
//! callee receives it, binds it by PHP's coercion rules in a weak file, accepts only exact types
//! (and int widened to float) in a strict file, and otherwise throws `TypeError`.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The values come from `json_decode()`, so only the run-time check decides them.
//! - Every expected output is verbatim php 8.5 stdout; messages are cut before php's
//!   "called in" clause, which elephc does not print.

use crate::support::*;

/// Verifies a strict file accepts exact types and the int-to-float widening at a declared
/// parameter, and refuses every other `mixed` value.
#[test]
fn test_strict_scalar_arguments_accept_exact_types_and_int_to_float() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function i_(int $v): string { return var_export($v, true); }
function f_(float $v): string { return var_export($v, true); }
function s_(string $v): string { return var_export($v, true); }
function b_(bool $v): string { return var_export($v, true); }
function n_(?int $v): string { return var_export($v, true); }
function u_(int|string $v): string { return var_export($v, true); }
$values = json_decode('[5, "5", 2.5, true, null, [1]]');
foreach ($values as $v) { try { echo i_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo f_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo s_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo b_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo n_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo u_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
"#,
    );
    assert_eq!(
        out,
        "5 E E E E E \n5.0 E 2.5 E E E \nE '5' E E E E \nE E E true E E \n5 E E E NULL E \n5 '5' E E E E \n"
    );
}

/// Verifies a weak file binds numeric strings, bools and leading-whitespace numbers as php does,
/// and refuses null, arrays, non-numeric and leading-numeric strings.
#[test]
fn test_weak_scalar_arguments_bind_by_php_coercion() {
    let out = compile_and_run(
        r#"<?php
function i_(int $v): string { return var_export($v, true); }
function f_(float $v): string { return var_export($v, true); }
function s_(string $v): string { return var_export($v, true); }
function b_(bool $v): string { return var_export($v, true); }
function n_(?int $v): string { return var_export($v, true); }
function u_(int|string $v): string { return var_export($v, true); }
$values = json_decode('[5, "5", " 5", "1e2", "5abc", "", true, null, [1]]');
foreach ($values as $v) { try { echo i_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo f_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo s_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo b_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo n_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
foreach ($values as $v) { try { echo u_($v), " "; } catch (TypeError $e) { echo "E "; } }
echo "\n";
"#,
    );
    assert_eq!(
        out,
        "5 5 5 100 E E 1 E E \n5.0 5.0 5.0 100.0 E E 1.0 E E \n'5' '5' ' 5' '1e2' '5abc' '' '1' E E \ntrue true true true true false true E E \n5 5 5 100 E E 1 NULL E \n5 '5' ' 5' '1e2' '5abc' '' 1 E E \n"
    );
}

/// Verifies the TypeError names the declaring class of an inherited method, a constructor, a
/// static method and a function, with the declared and the given type.
#[test]
fn test_scalar_argument_type_error_names_the_callee_like_php() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
class Base { public function m(int $i): int { return $i; } }
final class Child extends Base {}
final class Maker {
    public function __construct(public float $x) {}
    public static function make(?int $n): string { return var_export($n, true); }
}
function joinIt(string $a, bool $b): string { return $a; }
$checks = [
    fn() => (new Child())->m(json_decode('"5"')),
    fn() => new Maker(json_decode('"1.5"')),
    fn() => Maker::make(json_decode('2.5')),
    fn() => joinIt("ok", json_decode('[]')),
    fn() => joinIt(json_decode('null'), true),
    fn() => joinIt(json_decode('{"a":1}'), true),
];
foreach ($checks as $check) {
    try { $check(); echo "no error\n"; } catch (TypeError $e) { echo $e::class, ": ", explode(', called in', $e->getMessage())[0], "\n"; }
}
"#,
    );
    assert_eq!(
        out,
        "TypeError: Base::m(): Argument #1 ($i) must be of type int, string given\nTypeError: Maker::__construct(): Argument #1 ($x) must be of type float, string given\nTypeError: Maker::make(): Argument #1 ($n) must be of type ?int, float given\nTypeError: joinIt(): Argument #2 ($b) must be of type bool, array given\nTypeError: joinIt(): Argument #1 ($a) must be of type string, null given\nTypeError: joinIt(): Argument #1 ($a) must be of type string, stdClass given\n"
    );
}

/// Verifies the check runs after every argument is evaluated, reaches the caller's catch
/// through a finally, and survives an unused or assigned pure call.
#[test]
fn test_scalar_argument_check_runs_after_every_argument_is_evaluated() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function trace(string $label, mixed $value): mixed { echo $label, " "; return $value; }
function pair(int $a, int $b): int { return $a + $b; }
function outer(mixed $v): int {
    try { return pair(trace("a", $v), trace("b", 2)); }
    finally { echo "finally "; }
}
try { echo outer(json_decode('1')), "\n"; echo outer(json_decode('"1"')), "\n"; }
catch (TypeError $e) { echo "caught ", explode(', called in', $e->getMessage())[0], "\n"; }
function same(int $i): int { return $i; }
try { same(json_decode('"x"')); echo "unchecked\n"; } catch (TypeError $e) { echo "unused call checked\n"; }
try { $kept = same(json_decode('"y"')); echo "unchecked\n"; } catch (TypeError $e) { echo "assigned call checked\n"; }
"#,
    );
    assert_eq!(
        out,
        "a b finally 3\na b finally caught pair(): Argument #1 ($a) must be of type int, string given\nunused call checked\nassigned call checked\n"
    );
}

/// Verifies a string callable naming a function, a closure and an arrow function check their
/// declared parameters, the closures named by php's `{closure:…}` label.
#[test]
fn test_scalar_argument_check_covers_resolved_callables() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function twice(int $i): int { return $i * 2; }
function report(TypeError $e): void {
    $message = explode(', called in', $e->getMessage())[0];
    echo preg_replace('/\{closure:[^}]*\}/', '{closure}', $message), "\n";
}
$byName = 'twice';
$closure = function (int $i): int { return $i + 1; };
$arrow = fn(string $s): string => $s . "!";
try { echo $byName(json_decode('3')), "\n"; echo $byName(json_decode('"3"')), "\n"; } catch (TypeError $e) { report($e); }
try { echo $closure(json_decode('3')), "\n"; echo $closure(json_decode('"3"')), "\n"; } catch (TypeError $e) { report($e); }
try { echo $arrow(json_decode('"x"')), "\n"; echo $arrow(json_decode('3')), "\n"; } catch (TypeError $e) { report($e); }
"#,
    );
    assert_eq!(
        out,
        "6\ntwice(): Argument #1 ($i) must be of type int, string given\n4\n{closure}(): Argument #1 ($i) must be of type int, string given\nx!\n{closure}(): Argument #1 ($s) must be of type string, int given\n"
    );
}

/// Verifies accepted, converted and refused arguments release every value they staged.
#[test]
fn test_scalar_argument_check_releases_accepted_and_refused_values() {
    let output = compile_and_run_with_heap_debug(
        r#"<?php
declare(strict_types=1);
function len(string $s): int { return strlen($s); }
function num(?int $n): int { return $n ?? -1; }
$values = json_decode('["abc", "def", 7, null, [1, 2], {"k": "v"}]');
for ($round = 0; $round < 3; $round++) {
    foreach ($values as $v) {
        try { echo len($v), ","; } catch (TypeError $e) { echo "E,"; }
        try { echo num($v), ","; } catch (TypeError $e) { echo "E,"; }
    }
}
echo "\n";
"#,
    );
    assert!(output.success, "stdout={:?}\nstderr={}", output.stdout, output.stderr);
    assert_eq!(output.stdout, "3,E,3,E,E,7,E,-1,E,E,E,E,3,E,3,E,E,7,E,-1,E,E,E,E,3,E,3,E,E,7,E,-1,E,E,E,E,\n");
    assert!(output.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", output.stderr);
}

/// Verifies a strict file checks a `mixed` argument at a declared `string` with the read-only
/// guard: a string taken from a destructured list passes through a static call intact, every
/// other type is refused by php-src's name for it, and nothing leaks (AIC #223).
#[test]
fn test_strict_string_argument_is_guarded_and_kept_intact() {
    let output = compile_and_run_with_heap_debug(
        r#"<?php
declare(strict_types=1);
final class Span { public function __construct(public string $from, public string $to) {} }
final class Ranges {
    public static function parse(string $line): Span {
        [$from, $to] = self::ends($line);
        return self::span($from, $to);
    }
    private static function ends(string $line): array { return explode('-', $line, 2) + ['', '']; }
    private static function span(string $from, string $to): Span { return new Span($from, $to === '' ? 'end' : $to); }
}
function f(string $s): string { return "<$s>"; }
$spans = [];
foreach (['0-3', '999-', '5-77'] as $line) { $spans[] = Ranges::parse($line); }
$noise = [];
for ($i = 0; $i < 40; $i++) { $noise[] = str_repeat('x', 24) . $i; }
foreach ($spans as $span) { echo $span->from, '..', $span->to, ','; }
echo "\n";
foreach (json_decode('["s", 1, 1.5, true, null, [1], {"k": 1}]') as $value) {
    try { echo f($value), ','; } catch (TypeError $e) { echo explode(', called in', $e->getMessage())[0], ','; }
}
echo "\n";
"#,
    );
    assert!(output.success, "stdout={:?}\nstderr={}", output.stdout, output.stderr);
    assert_eq!(
        output.stdout,
        concat!(
            "0..3,999..end,5..77,\n",
            "<s>,f(): Argument #1 ($s) must be of type string, int given,",
            "f(): Argument #1 ($s) must be of type string, float given,",
            "f(): Argument #1 ($s) must be of type string, true given,",
            "f(): Argument #1 ($s) must be of type string, null given,",
            "f(): Argument #1 ($s) must be of type string, array given,",
            "f(): Argument #1 ($s) must be of type string, stdClass given,\n",
        )
    );
    assert!(output.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", output.stderr);
}
