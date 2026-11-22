//! Purpose:
//! Integration tests for a value returned through a declared union or nullable scalar type:
//! php-src checks it at the return boundary, binds it by PHP's union coercion order in a weak
//! file, accepts only exact members (and int widened to float) in a strict file, and otherwise
//! throws `TypeError`.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The values come from `json_decode()` and `mixed` parameters, so only the run-time check
//!   decides them.
//! - Every expected output is verbatim php 8.5 stdout; fractional float-to-int bindings are
//!   left out because php prints their deprecation on stdout.

use crate::support::*;

/// Verifies a weak file binds each value by PHP's union coercion order: an exact member
/// first, then int, float, string and bool, with numeric strings choosing int or float.
#[test]
fn test_weak_union_return_binds_by_php_coercion_order() {
    let out = compile_and_run(
        r#"<?php
function is_(mixed $v): int|string { return $v; }
function if_(mixed $v): int|float { return $v; }
function fs(mixed $v): float|string { return $v; }
function ib(mixed $v): int|bool { return $v; }
function fb(mixed $v): float|bool { return $v; }
function in_(mixed $v): ?int { return $v; }
$values = json_decode('[1, 2.0, "7", " 7", "1e2", "7abc", "", true, false]');
foreach (['is_', 'if_', 'fs', 'ib', 'fb', 'in_'] as $f) {
    foreach ($values as $v) {
        try { echo json_encode($f($v)), gettype($f($v))[0], " "; } catch (TypeError $e) { echo "E "; }
    }
    echo "\n";
}
"#,
    );
    assert_eq!(
        out,
        "1i 2i \"7\"s \" 7\"s \"1e2\"s \"7abc\"s \"\"s 1i 0i \n1i 2d 7i 7i 100d E E 1i 0i \n1d 2d \"7\"s \" 7\"s \"1e2\"s \"7abc\"s \"\"s 1d 0d \n1i 2i 7i 7i 100i trueb falseb trueb falseb \n1d 2d 7d 7d 100d trueb falseb trueb falseb \n1i 2i 7i 7i 100i E E 1i 0i \n"
    );
}

/// Verifies a strict file accepts exact members and the int-to-float widening, and refuses
/// every other value.
#[test]
fn test_strict_union_return_accepts_exact_members_and_int_to_float() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function is_(mixed $v): int|string { return $v; }
function fs(mixed $v): float|string { return $v; }
function fb(mixed $v): float|bool|null { return $v; }
$values = json_decode('[1, 2.5, "7", true, null, [1]]');
foreach (['is_', 'fs', 'fb'] as $f) {
    foreach ($values as $v) {
        try { echo json_encode($f($v)), gettype($f($v))[0], " "; } catch (TypeError $e) { echo "E "; }
    }
    echo "\n";
}
"#,
    );
    assert_eq!(out, "1i E \"7\"s E E E \n1d 2.5d \"7\"s E E E \n1d 2.5d E trueb nullN E \n");
}

/// Verifies the refusal names the union as php-src prints it, a bool by its value, an object
/// by its class, and a closure by its `{closure:...}` label.
#[test]
fn test_union_return_type_error_matches_php_wording() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
final class Square {}
final class Circle {}
function shape(mixed $v): Square|int|null { return $v; }
function maybe(mixed $v): ?int { return $v; }
function words(mixed $v): string|int|null { return $v; }
$label = function (mixed $v): int|bool { return $v; };
foreach ([fn () => shape(new Circle), fn () => shape("1"), fn () => maybe(true), fn () => words(1.5),
    fn () => words([]), fn () => $label(false ?: "x")] as $case) {
    try { var_dump($case()); } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
}
"#,
    );
    let (closure, rest): (Vec<&str>, Vec<&str>) =
        out.lines().partition(|line| line.starts_with("{closure:"));
    assert_eq!(rest.iter().map(|line| format!("{line}\n")).collect::<String>(), "shape(): Return value must be of type Square|int|null, Circle returned\nshape(): Return value must be of type Square|int|null, string returned\nmaybe(): Return value must be of type ?int, true returned\nwords(): Return value must be of type string|int|null, float returned\nwords(): Return value must be of type string|int|null, array returned\n");
    assert_eq!(closure.len(), 1);
    assert!(closure[0].ends_with("}(): Return value must be of type int|bool, string returned"), "{}", closure[0]);
}

/// Verifies the optimizer keeps a call whose return check can throw: an unused call and an
/// assignment inside `try` both reach the caller's `catch`.
#[test]
fn test_union_return_type_error_reaches_the_callers_catch() {
    let out = compile_and_run(
        r#"<?php
function pick(mixed $v): int|string { return $v; }
$values = json_decode('[null, [1]]');
try { pick($values[0]); echo "unreached\n"; } catch (TypeError $e) { echo "statement: ", $e->getMessage(), "\n"; }
try { $kept = pick($values[1]); echo "unreached\n"; } catch (TypeError $e) { echo "assignment: ", $e->getMessage(), "\n"; }
"#,
    );
    assert_eq!(out, "statement: pick(): Return value must be of type string|int, null returned\nassignment: pick(): Return value must be of type string|int, array returned\n");
}

/// Verifies accepted strings, arrays and refused values release every staged value, round
/// after round.
#[test]
fn test_union_return_releases_accepted_and_refused_values() {
    let output = compile_and_run_with_heap_debug(
        r#"<?php
function label(mixed $v): string|int|null { return $v; }
function sized(mixed $v): array|int { return $v; }
$parts = json_decode('["a", 3, null, 2.0, {"k": "v"}, [1, 2]]', true);
$out = [];
for ($round = 0; $round < 3; $round++) {
    foreach ($parts as $part) {
        try { $out[] = label($part) ?? "null"; } catch (TypeError $e) { $out[] = "E"; }
        try { $sized = sized($part); $out[] = is_array($sized) ? count($sized) : $sized; } catch (TypeError $e) { $out[] = "F"; }
    }
}
echo implode(",", $out), "\n";
"#,
    );
    assert!(output.success, "stdout={:?}\nstderr={}", output.stdout, output.stderr);
    assert_eq!(output.stdout, "a,F,3,3,null,F,2,2,E,1,E,2,a,F,3,3,null,F,2,2,E,1,E,2,a,F,3,3,null,F,2,2,E,1,E,2\n");
    assert!(output.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", output.stderr);
}

/// Verifies a `false` member never coerces: `array|false` and `Box|false` accept `false` and
/// their other member, and refuse `true`, `0`, `""` and `null`.
#[test]
fn test_false_member_union_return_accepts_only_exact_members() {
    let out = compile_and_run(
        r#"<?php
class Box {}
function af(mixed $v): array|false { return $v; }
function bf(mixed $v): Box|false { return $v; }
foreach ([false, true, 0, "", null, [1], new Box()] as $v) {
    foreach (['af', 'bf'] as $f) {
        try { $r = $f($v); echo is_object($r) ? 'Box' : json_encode($r), " "; }
        catch (TypeError $e) { echo substr($e->getMessage(), strrpos($e->getMessage(), ', ') + 2), " | "; }
    }
    echo "\n";
}
"#,
    );
    assert_eq!(
        out,
        "false false \ntrue returned | true returned | \nint returned | int returned | \nstring returned | string returned | \nnull returned | null returned | \n[1] array returned | \nBox returned | Box \n"
    );
}
