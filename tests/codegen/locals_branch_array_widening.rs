//! Purpose:
//! End-to-end coverage for an array local reassigned inside a branch from an array of another
//! element type, which `merge_local_assignment_type` widens instead of rejecting.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - A store at conditional depth 0 re-binds the local (the straight-line retype); a store in a
//!   branch cannot, and used to fail with `cannot reassign $x from array<T> to array`.
//! - Every fixture branches on `$argc` and runs once per argument vector, so both arms execute,
//!   with heap debugging on, and each run must leave a clean heap.

use crate::support::*;

/// Runs `source` with no arguments and with one, and checks each run's output and heap.
fn assert_both_arms(source: &str, without_argument: &str, with_argument: &str) {
    let runs = compile_and_run_with_heap_debug_per_argv(source, &[&[], &["x"]]);
    for (run, expected) in runs.iter().zip([without_argument, with_argument]) {
        assert!(run.success, "program failed: {}", run.stderr);
        assert_eq!(run.stdout, expected);
        assert!(
            run.stderr.contains("HEAP DEBUG: leak summary: clean"),
            "expected a clean heap, got: {}",
            run.stderr
        );
    }
}

/// Widens a call declared `array` stored in an `if` over an `array<Link>` local.
#[test]
fn test_branch_array_widening_a_call_result_stored_in_an_if() {
    assert_both_arms(
        r#"<?php
final class Link { public function __construct(public string $key) {} }
function staff(): array { return [new Link("a"), new Link("b")]; }
function build(bool $more): array {
    $links = [new Link("dash")];
    if ($more) {
        $links = staff();
    }
    $links[] = new Link("theme");
    return $links;
}
$out = [];
foreach (build($argc > 1) as $link) { $out[] = $link->key; }
echo implode(",", $out);"#,
        r#"dash,theme"#,
        r#"a,b,theme"#,
    );
}

/// Widens a spread of a call's result stored in an `if` over an `array<Link>` local: the shape AIC #248 reported.
#[test]
fn test_branch_array_widening_a_spread_of_a_call_stored_in_an_if() {
    assert_both_arms(
        r#"<?php
final readonly class Link { public function __construct(public string $key) {} }
/** @return list<Link> */
function staff(): array { return [new Link("a"), new Link("b")]; }
/** @return list<Link> */
function build(bool $more): array {
    $links = [new Link("dash")];
    if ($more) {
        $links = [...$links, ...staff()];
    }
    $links[] = new Link("theme");
    return $links;
}
$out = [];
foreach (build($argc > 1) as $link) { $out[] = $link->key; }
echo implode(",", $out);"#,
        r#"dash,theme"#,
        r#"dash,a,b,theme"#,
    );
}

/// Widens a static method's spread result and an instance method's result, each stored in an `if` inside a readonly class.
#[test]
fn test_branch_array_widening_static_and_instance_method_results_stored_in_an_if() {
    assert_both_arms(
        r#"<?php
final readonly class Link { public function __construct(public string $key) {} }
final class Repo { public function all(): array { return [new Link("r1"), new Link("r2")]; } }
final readonly class Navigation {
    private static function staff(): array { return [new Link("a"), new Link("b")]; }
    public static function build(bool $more): array {
        $links = [new Link("dash")];
        if ($more) {
            $links = [...$links, ...self::staff()];
        }
        $links[] = new Link("theme");
        return $links;
    }
    public function fromRepo(bool $more, Repo $repo): array {
        $links = [new Link("own")];
        if ($more) {
            $links = $repo->all();
        }
        return $links;
    }
}
$out = [];
foreach (Navigation::build($argc > 1) as $link) { $out[] = $link->key; }
foreach ((new Navigation())->fromRepo($argc > 1, new Repo()) as $link) { $out[] = $link->key; }
echo implode(",", $out);"#,
        r#"dash,theme,own"#,
        r#"dash,a,b,theme,r1,r2"#,
    );
}

/// Widens stores in an `elseif`, an `if` nested in it and an `else`, each a different array shape.
#[test]
fn test_branch_array_widening_stores_in_elseif_else_and_nested_branches() {
    assert_both_arms(
        r#"<?php
final class Link { public function __construct(public string $key) {} }
function staff(): array { return [new Link("a"), new Link("b")]; }
function pick(int $n): array {
    $links = [new Link("d")];
    if ($n === 1) {
        $links = [...staff(), ...$links];
    } elseif ($n === 2) {
        if ($n > 1) {
            $links = staff();
        }
    } else {
        $links = [new Link("x"), ...$links];
    }
    return $links;
}
foreach ([1, 2, 3, $argc] as $n) {
    $out = [];
    foreach (pick($n) as $link) { $out[] = $link->key; }
    echo implode(",", $out), ";";
}"#,
        r#"a,b,d;a,b;x,d;a,b,d;"#,
        r#"a,b,d;a,b;x,d;a,b;"#,
    );
}

/// Widens `switch` arms storing other element types, and a top-level `if` storing a spread.
#[test]
fn test_branch_array_widening_switch_arms_and_top_level_branches() {
    assert_both_arms(
        r#"<?php
function names(): array { return ["x" => 1, "y" => "two"]; }
function row(int $n): array {
    $row = [1, 2];
    switch ($n) {
        case 1:
            $row = ["a", "b"];
            break;
        case 2:
            $row = names();
            break;
    }
    $row[] = 9;
    return $row;
}
$m = $argc > 1;
$top = [1.5];
if ($m) {
    $top = [...$top, ...row(1)];
}
foreach ([1, 2, 3] as $n) { echo json_encode(row($n)), ";"; }
echo json_encode($top);"#,
        r#"["a","b",9];{"x":1,"y":"two","0":9};[1,2,9];[1.5]"#,
        r#"["a","b",9];{"x":1,"y":"two","0":9};[1,2,9];[1.5,"a","b",9]"#,
    );
}

/// The widening warns once per site, names both types, and `--strict-locals` keeps the error.
#[test]
fn test_branch_array_widening_warns_and_strict_locals_rejects_it() {
    let source = "<?php function s(): array { return [\"q\"]; } $a = [$argc]; if ($argc > 1) { $a = s(); } echo count($a);";
    let warnings = check_files_diagnostics(&[("main.php", source)], "main.php", false)
        .expect("a branch store of another array type must type-check");
    assert_eq!(warnings.len(), 1, "one warning expected, got: {warnings:?}");
    assert!(
        warnings[0].contains("$a is assigned incompatible array types (array<int> and array)"),
        "unexpected warning: {}",
        warnings[0]
    );
    let error = check_files_diagnostics(&[("main.php", source)], "main.php", true)
        .expect_err("--strict-locals must keep rejecting the store");
    assert!(
        error.contains("cannot reassign $a from array<int> to array"),
        "unexpected error: {error}"
    );
}

/// A local a closure captures by reference is reachable through that reference, so its binding is
/// not widened and the store stays an error.
#[test]
fn test_branch_array_widening_skips_a_reference_captured_local() {
    let source = "<?php function s(): array { return [\"q\"]; } $a = [$argc]; $f = function () use (&$a) { return count($a); }; if ($argc > 1) { $a = s(); } echo $f();";
    let error = check_files_diagnostics(&[("main.php", source)], "main.php", false)
        .expect_err("a reference-captured local must keep the error");
    assert!(
        error.contains("cannot reassign $a from array<int> to array"),
        "unexpected error: {error}"
    );
}
