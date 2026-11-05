//! Purpose:
//! Integration or regression tests for local types joined after a `try`/`catch`.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The try body and every catch reach the post-try code on different paths. A local each of
//!   them writes must be read below at a type both paths stored, not at the last catch's type.
//! - Every expectation here is the host PHP 8.5.12 output for the same fixture.

use super::*;

/// A ternary with a `[]` arm assigned in `try`, and `[]` again in `catch`, iterates its keys.
///
/// The ternary is `mixed` (an empty array against a keyed array of `string|list<string>`), so
/// the try path stores a boxed value. The catch was lowered after it and left `array<never>`
/// as the local's type, so `foreach` read the boxed value as an indexed list: keys 0 and 1 with
/// NULL values, while `count()` still said 2.
#[test]
fn test_try_ternary_empty_array_then_catch_empty_array_keeps_keys() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
/** @return array<string, string|list<string>> */
function fields(bool $lists): array {
    $f = ['q' => 'a', 'sort' => 'b'];
    if ($lists) { $f['tags'] = ['x']; }
    return $f;
}
function viaTry(bool $none, bool $lists): string {
    try { $fields = $none ? [] : fields($lists); } catch (RuntimeException) { $fields = []; }
    $out = [];
    foreach ($fields as $name => $value) { $out[] = $name . '=' . gettype($value); }
    return count($fields) . ':' . implode(',', $out);
}
echo viaTry(false, false), "|", viaTry(false, true), "|", viaTry(true, false), "\n";
"#,
    );
    assert_eq!(out, "2:q=string,sort=string|3:q=string,sort=string,tags=array|0:\n");
}

/// A `mixed` call result in `try` and an array literal in `catch` read back on both paths.
#[test]
fn test_try_mixed_result_then_catch_array_reads_both_paths() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
declare(strict_types=1);
function m(bool $t): mixed {
    if ($t) { throw new RuntimeException('x'); }
    return ['q' => 'a', 'sort' => 'b'];
}
function keys(bool $t): string {
    try { $f = m($t); } catch (RuntimeException) { $f = ['z' => 1]; }
    $out = [];
    foreach ($f as $k => $v) { $out[] = $k . '=' . gettype($v); }
    return implode(',', $out);
}
echo keys(false), "|", keys(true), "\n";
"#,
    );
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "q=string,sort=string|z=integer\n", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A `mixed` call result in `try` and an integer in `catch`: the try path is not read as an int.
#[test]
fn test_try_mixed_result_then_catch_int_reads_boxed_value() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function n(bool $t): mixed {
    if ($t) { throw new RuntimeException('x'); }
    return 'str';
}
function scalar(bool $t): string {
    try { $f = n($t); } catch (RuntimeException) { $f = 5; }
    return gettype($f) . ':' . var_export($f, true);
}
echo scalar(false), "|", scalar(true), "\n";
"#,
    );
    assert_eq!(out, "string:'str'|integer:5\n");
}

/// The join also covers a `try` with a `finally`, and every catch of a multi-catch `try`.
#[test]
fn test_try_catch_finally_and_two_catches_join_local_types() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function m(bool $t): mixed {
    if ($t) { throw new RuntimeException('x'); }
    return ['q' => 'a', 'sort' => 'b'];
}
function withFinally(bool $t): string {
    $log = '';
    try { $f = m($t); } catch (RuntimeException) { $f = []; } finally { $log = 'f'; }
    $out = [];
    foreach ($f as $k => $v) { $out[] = $k . '=' . gettype($v); }
    return $log . ':' . implode(',', $out);
}
function twoCatches(bool $t): string {
    try { $f = m($t); } catch (LogicException) { $f = [9]; } catch (RuntimeException) { $f = []; }
    $out = [];
    foreach ($f as $k => $v) { $out[] = $k . '=' . gettype($v); }
    return implode(',', $out);
}
echo withFinally(false), "|", withFinally(true), "\n";
echo twoCatches(false), "|", twoCatches(true), "\n";
"#,
    );
    assert_eq!(out, "f:q=string,sort=string|f:\nq=string,sort=string|\n");
}

/// A top-level catch variable bound in a loop and rebound by a later catch keeps its object.
///
/// The loop leaves `$e` boxed on the try path and bare on the catch path, and the join types it
/// `mixed`. Global storage keeps whatever an arm stored, so the catch arm boxes its object
/// before the merge; reading the bare pointer as a cell lost the later `catch`'s exception
/// (`get_class()` printed nothing) and crashed under heap debugging. An `if` arm that binds an
/// object beside a `mixed` one joins the same way.
#[test]
fn test_top_level_catch_variable_rebound_after_a_loop_keeps_its_object() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
class Foo {}
$f = new SplFixedArray(3);
foreach (["x", new Foo()] as $k) { try { $v = $f[$k]; } catch (TypeError $e) { echo "t\n"; } }
try { $f["-1"] = 0; } catch (OutOfBoundsException $e) { echo get_class($e), ": ", $e->getMessage(), "\n"; }
foreach (["x", new Foo()] as $k) { if (is_string($k)) { $o = new TypeError("a"); } }
try { $f["-1"] = 0; } catch (OutOfBoundsException $o) { echo get_class($o), "\n"; }
"#,
    );
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "t\nt\nOutOfBoundsException: Index invalid or out of range\nOutOfBoundsException\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}
