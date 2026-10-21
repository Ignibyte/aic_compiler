//! Purpose:
//! Executes frame-local checked arithmetic and conservative escape fallbacks.
//!
//! Called from:
//! - The focused optimizer codegen test suite.
//!
//! Key details:
//! - Both IR modes run with heap validation, actual PHP oracles and all target emitters.

use super::*;

/// Compiles and runs an explicit mode with heap validation and allocation counters.
fn run(source: &str, optimized: bool) -> (String, String) {
    let dir = make_cli_test_dir("elephc_stack_numeric");
    let path = dir.join("main.php");
    fs::write(&path, source).unwrap();
    let mut command = elephc_cli_command(&dir);
    command.args(["--heap-debug", "--gc-stats"]);
    if !optimized {
        command.arg("--no-ir-opt");
    }
    let compile = command.arg(&path).output().unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let result = run_binary(&path.with_extension(""), &dir);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(
        stderr.contains("HEAP DEBUG: leak summary: clean"),
        "{stderr}"
    );
    if std::env::var_os("ELEPHC_PHP_CHECK").is_some() {
        let php = Command::new("php")
            .arg("-n")
            .arg(&path)
            .output()
            .expect("PHP oracle");
        assert!(
            php.status.success(),
            "{}",
            String::from_utf8_lossy(&php.stderr)
        );
        assert_eq!(stdout.as_bytes(), php.stdout, "actual PHP oracle");
    }
    fs::remove_dir_all(dir).unwrap();
    (stdout, stderr)
}

/// An accumulator has constant heap allocations rather than one numeric cell per iteration.
#[test]
fn test_stack_numeric_reduction_is_heap_clean_and_allocation_bounded() {
    let source = include_str!("../../../examples/numeric-local/main.php");
    let plain = run(source, false);
    let optimized = run(source, true);
    assert_eq!(plain.0, optimized.0);
    assert_eq!(optimized.0, "499500");
    let (plain_allocs, _) = parse_gc_stats(&plain.1);
    let (optimized_allocs, optimized_frees) = parse_gc_stats(&optimized.1);
    assert_eq!(
        optimized_allocs, optimized_frees,
        "every numeric owner is retired"
    );
    let small = run(&source.replace("* 1000", "* 10"), true);
    assert_eq!(small.0, "45");
    let (small_allocs, _) = parse_gc_stats(&small.1);
    assert!(optimized_allocs.abs_diff(small_allocs) <= 8,
        "frame numeric allocation count must stay bounded: small={small_allocs}, large={optimized_allocs}");
    assert!(
        optimized_allocs + 900 < plain_allocs,
        "numeric allocations: optimized={optimized_allocs}, plain={plain_allocs}"
    );
}

/// Escaping boxes, aliases, captures and a delayed snapshot keep independently owned values.
#[test]
fn test_stack_numeric_escaping_values_preserve_php_snapshots() {
    let source = r#"<?php
function snapshots(int $n): void {
    $sum = 0; $items = [];
    for ($i = 0; $i < $n; $i++) { $sum += $i; $items[] = $sum; }
    echo $items[2], ':', $items[4], '|';
}
function reference_snapshot(int $n): void {
    $value = 0; $ref =& $value;
    for ($i = 0; $i < $n; $i++) { $value += $i; }
    echo $ref, '|';
}
function capture_snapshot(int $n): void {
    $s = 0; $seen = function() use (&$s) { return $s; };
    for ($i = 0; $i < $n; $i++) { $s += $i; }
    echo $seen(), '|';
}
function value_snapshot(int $n): void {
    $v = 0; $copy = 0;
    for ($i = 0; $i < $n; $i++) { $v += $i; if ($i == 2) { $copy = $v; } }
    echo (int)$copy, ':', (int)$v, '|';
}
function scope_snapshot(int $n): void {
    $g = 0;
    for ($i = 0; $i < $n; $i++) { $g += $i; }
    $vars = get_defined_vars();
    for ($i = 0; $i < $n; $i++) { $g += 1; }
    echo (int)$vars["g"], ':', (int)$g, '|';
}
snapshots($argc + 4);
reference_snapshot($argc + 4);
capture_snapshot($argc + 4);
value_snapshot($argc + 4);
scope_snapshot($argc + 4);
"#;
    for optimized in [false, true] {
        assert_eq!(run(source, optimized).0, "3:10|10|10|3:10|10:15|");
    }
}

/// Overflow branches use the original operands for all three arithmetic operations.
#[test]
fn test_stack_numeric_checked_overflow_matches_php_in_both_modes() {
    let source = r#"<?php
function casts(int $n, int $seed, int $step): void {
    $a = 0; $b = 0; $c = 0;
    for ($i = 0; $i < $n; $i++) {
        $a = $seed + $step; $b = $seed - $step; $c = $seed * $step;
    }
    echo @(int)$a, ':', @(int)$b, ':', @(int)$c, '|';
}
casts($argc + 1, PHP_INT_MAX, 2);
casts($argc + 1, PHP_INT_MIN, 2);
casts($argc + 1, PHP_INT_MIN, -1);
"#;
    let expected = "-9223372036854775808:9223372036854775805:0|-9223372036854775806:-9223372036854775808:0|-9223372036854775808:-9223372036854775807:-9223372036854775808|";
    assert_eq!(run(source, false).0, expected);
    assert_eq!(run(source, true).0, expected);
}

/// Every target emits checked integer and float paths while rejected escapes stay heap-backed.
#[test]
fn test_stack_numeric_checked_emission_all_supported_targets() {
    let source = format!(
        "{}\n{}",
        include_str!("../../../examples/numeric-local/main.php"),
        r#"
#[Export]
function numeric_triple(int $n, int $seed, int $step): void {
    $a = 0; $b = 0; $c = 0;
    for ($i = 0; $i < $n; $i++) {
        $a = $seed + $step; $b = $seed - $step; $c = $seed * $step;
    }
    echo (int)$a, ':', (int)$b, ':', (int)$c;
}
"#
    );
    for target in [
        "macos-aarch64",
        "ios-arm64",
        "ios-sim-arm64",
        "linux-aarch64",
        "linux-x86_64",
    ] {
        for optimized in [false, true] {
            let dir = make_cli_test_dir("elephc_stack_numeric_target");
            let path = dir.join("main.php");
            fs::write(&path, &source).unwrap();
            let mut command = elephc_cli_command(&dir);
            command.args(["--emit-asm", "--target", target]);
            if target.starts_with("ios-") {
                command.args(["--emit", "staticlib"]);
            }
            if !optimized {
                command.arg("--no-ir-opt");
            }
            let output = command.arg(&path).output().unwrap();
            assert!(
                output.status.success(),
                "{target}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let asm = fs::read_to_string(path.with_extension("s")).unwrap();
            assert!(!asm.is_empty(), "{target}");
            if optimized {
                assert!(asm.contains("stack_numeric_overflow"), "{target}");
                assert!(asm.contains(if target == "linux-x86_64" {
                    "cvtsi2sd"
                } else {
                    "scvtf"
                }));
                assert!(asm.contains(if target == "linux-x86_64" {
                    "imul r11, r10"
                } else {
                    "smulh x9, x0, x10"
                }));
            }
            fs::remove_dir_all(dir).unwrap();
        }
    }
}

/// Normal and exceptional exits release heap seeds without treating stack cells as allocations.
#[test]
fn test_stack_numeric_throw_and_finally_keep_scalar_state() {
    let source = r#"<?php
function unwind_numeric(int $n): int {
    $sum = 0;
    try {
        for ($i = 0; $i < $n; $i++) { $sum += $i; }
        throw new Exception("done");
    } catch (Exception $e) {
        return (int)$sum;
    } finally { echo "finally|"; }
}
echo unwind_numeric($argc + 4);
"#;
    for optimized in [false, true] {
        assert_eq!(run(source, optimized).0, "finally|10");
    }
}
