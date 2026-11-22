//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of string-key writes that
//! give a function-local associative array a second value type, covering scalar, null, and
//! array payloads, loop writes, compound updates, and same-type writes that must stay typed.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.
//! - Heap-debug fixtures also assert a clean leak summary, since the widening converts the hash in place.

use crate::support::*;

/// An integer written after a string must not be stored under the string tag.
#[test]
fn test_function_local_hash_string_then_int() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function build(): void {
    $m = [];
    $m["k"] = "v";
    $m["j"] = 5;
    echo count($m), ":", $m["k"], ":", $m["j"], "\n";
    var_dump($m);
}
build();
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "2:v:5\narray(2) {\n  [\"k\"]=>\n  string(1) \"v\"\n  [\"j\"]=>\n  int(5)\n}\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A string written after an integer must not be read back as the integer's slot.
#[test]
fn test_function_local_hash_int_then_string() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function build(): void {
    $m = [];
    $m["j"] = 5;
    $m["k"] = "v";
    foreach ($m as $key => $value) {
        echo $key, "=", $value, ";";
    }
}
build();
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "j=5;k=v;", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Bool, null, float, and array payloads keep their own PHP types after the hash widens.
#[test]
fn test_function_local_hash_widens_for_each_payload_kind() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function boolIntoInt(): void { $m = ["a" => 1]; $m["b"] = true; var_dump($m["b"]); }
function nullIntoString(): void { $m = ["a" => "x"]; $m["b"] = null; var_dump($m["b"]); }
function floatIntoInt(): void { $m = ["a" => 1]; $m["b"] = 2.5; echo json_encode($m), "\n"; }
function arrayIntoString(): void { $m = ["a" => "x"]; $m["b"] = [1, 2]; echo count($m["b"]), "\n"; }
boolIntoInt();
nullIntoString();
floatIntoInt();
arrayIntoString();
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "bool(true)\nNULL\n{\"a\":1,\"b\":2.5}\n2\n", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A write inside a loop, whose local the loop pass already widens, still round-trips.
#[test]
fn test_function_local_hash_widens_inside_loop() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function build(): string {
    $m = ["a" => "x"];
    for ($i = 0; $i < 3; $i++) {
        $m["n" . $i] = $i;
    }
    return json_encode($m);
}
echo build();
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "{\"a\":\"x\",\"n0\":0,\"n1\":1,\"n2\":2}", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A compound update reads and writes the widened entry.
#[test]
fn test_function_local_hash_compound_update_after_widening() {
    let out = compile_and_run(r#"<?php
function build(): string {
    $m = ["a" => "x"];
    $m["c"] = 1;
    $m["c"] += 2;
    return $m["a"] . $m["c"];
}
echo build();
"#);
    assert_eq!(out, "x3");
}

/// Writes of the hash's own value type keep it typed for indexed consumers.
#[test]
fn test_function_local_hash_same_type_writes_stay_typed() {
    let out = compile_and_run(r#"<?php
function build(): string {
    $m = [];
    $m["a"] = "x";
    $m["b"] = "y";
    return implode(",", $m);
}
echo build();
"#);
    assert_eq!(out, "x,y");
}
