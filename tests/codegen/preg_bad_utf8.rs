//! Purpose:
//! Regression coverage for `/u` patterns run on subjects that are not valid UTF-8,
//! and for the `preg_last_error()` / `preg_last_error_msg()` state they leave.
//!
//! Called from:
//! - `cargo test --test codegen_tests preg_bad_utf8` through Rust's test harness.
//!
//! Key details:
//! - Expected stdout was measured on PHP 8.5.12. A malformed `/u` subject makes
//!   `preg_match()`, `preg_match_all()` and `preg_split()` return false and
//!   `preg_replace()` and `preg_replace_callback()` return null, with
//!   `PREG_BAD_UTF8_ERROR`. `preg_match()` writes `[]` to `$matches` and
//!   `preg_match_all()` writes its empty result shape; a compile failure leaves
//!   `$matches` alone and reports `PREG_INTERNAL_ERROR`.
//! - The same bytes without `/u` still match, valid UTF-8 under `/u` still
//!   matches, and every regex call resets the error state on entry.
//! - The heap-debug fixtures prove the failure paths release the result arrays
//!   and helper buffers they had already allocated.

use crate::support::*;

/// `preg_match()` returns false for a malformed `/u` subject and records the error.
#[test]
fn test_preg_bad_utf8_preg_match_returns_false() {
    let out = compile_and_run_with_regex(r#"<?php
function show(int|false $r): string { return $r === false ? 'false' : (string) $r; }
foreach (["\xff\xfe", "\xc3", "a\xed\xa0\x80b"] as $t) {
    echo show(preg_match('/[\x00-\x08]/u', $t)), ',', show(preg_match('//u', $t)), ',', preg_last_error(), '|';
}
echo show(preg_match('/a/', "a\xff")), ',', preg_last_error(), '|';
echo show(preg_match('/a/u', "a\xff")), ',', preg_last_error(), ',', preg_last_error_msg(), '|';
echo show(preg_match('/\x{e9}/u', "caf\xc3\xa9")), ',', preg_last_error(), ',', preg_last_error_msg(), '|';
echo PREG_BAD_UTF8_ERROR, PREG_NO_ERROR, PREG_INTERNAL_ERROR, PREG_BACKTRACK_LIMIT_ERROR, PREG_RECURSION_LIMIT_ERROR, PREG_BAD_UTF8_OFFSET_ERROR, PREG_JIT_STACKLIMIT_ERROR;
"#);
    assert_eq!(
        out,
        "false,false,4|false,false,4|false,false,4|1,0|false,4,Malformed UTF-8 characters, possibly incorrectly encoded|1,0,No error|4012356"
    );
}

/// `preg_match()` writes an empty `$matches` for a malformed subject but not for a bad pattern.
#[test]
fn test_preg_bad_utf8_preg_match_matches_and_internal_error() {
    let out = compile_and_run_with_heap_debug(r#"<?php
$m = ['kept'];
var_dump(preg_match('/(a)(b)?/u', "a\xff", $m));
echo count($m), '|', preg_last_error(), '|';
$m = ['kept'];
var_dump(preg_match('/(a/u', 'a', $m));
echo $m[0], '|', preg_last_error(), '|', preg_last_error_msg(), '|';
var_dump(preg_match('/(a)(b)?/u', "\xc3\xa9a", $m));
echo count($m), ':', $m[1], '|', PrEg_LaSt_ErRoR(), '|';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "bool(false)\n0|4|bool(false)\nkept|1|Internal error|int(1)\n2:a|0|"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// `preg_match_all()` returns false and writes PHP's empty result shape in both orders.
#[test]
fn test_preg_bad_utf8_preg_match_all_returns_false() {
    let out = compile_and_run_with_heap_debug(r#"<?php
$bad = "ab\xc3";
var_dump(preg_match_all('/(a)(b)?/u', $bad));
echo preg_last_error(), '|';
var_dump(preg_match_all('/(a)(b)?/u', $bad, $m));
echo count($m), count($m[0]), count($m[1]), count($m[2]), '|', preg_last_error(), '|';
var_dump(preg_match_all('/(a)(b)?/u', $bad, $m, PREG_SET_ORDER));
echo count($m), '|', preg_last_error(), '|';
var_dump(preg_match_all('/(a)(b)?/', $bad, $m));
echo count($m[0]), '|', preg_last_error(), '|';
var_dump(preg_match_all('/a/u', "\xc3\xa9a\xc3\xa9a"));
echo preg_last_error(), '|';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "bool(false)\n4|bool(false)\n3000|4|bool(false)\n0|4|int(1)\n1|0|int(2)\n0|"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// `preg_replace()` returns null for a malformed `/u` subject and replaces otherwise.
#[test]
fn test_preg_bad_utf8_preg_replace_returns_null() {
    let out = compile_and_run_with_regex(r#"<?php
$bad = "a\xffa";
var_dump(preg_replace('/a/u', 'x', $bad));
echo preg_last_error(), '|';
echo bin2hex(preg_replace('/a/', 'x', $bad)), '|', preg_last_error(), '|';
echo preg_replace('/a/u', 'x', "\xc3\xa9a"), '|', preg_last_error(), '|';
"#);
    assert_eq!(out, "NULL\n4|78ff78|0|éx|0|");
}

/// `preg_replace_callback()` returns null without calling back and releases its buffers.
#[test]
fn test_preg_bad_utf8_preg_replace_callback_returns_null() {
    let out = compile_and_run_with_heap_debug(r#"<?php
$calls = 0;
$cb = function (array $m) use (&$calls): string { $calls++; return 'x'; };
var_dump(preg_replace_callback('/a/u', $cb, "a\xffa"));
echo $calls, '|', preg_last_error(), '|';
echo preg_replace_callback('/a/u', $cb, "\xc3\xa9a"), '|', $calls, '|', preg_last_error(), '|';
echo bin2hex(preg_replace_callback('/a/', $cb, "a\xff")), '|', preg_last_error(), '|';
echo preg_replace_callback('/a/', function (array $m): string { preg_match('/a/u', "\xff"); return 'y'; }, 'a'), '|', preg_last_error(), '|';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "NULL\n0|4|éx|1|0|78ff|0|y|4|");
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// `preg_split()` returns false and releases its result array; a limit of 1 never matches.
#[test]
fn test_preg_bad_utf8_preg_split_returns_false() {
    let out = compile_and_run_with_heap_debug(r#"<?php
$bad = "a,\xff,b";
var_dump(preg_split('/,/u', $bad));
echo preg_last_error(), '|';
echo count(preg_split('/,/u', $bad, 1)), '|', preg_last_error(), '|';
echo count(preg_split('/,/', $bad)), '|', preg_last_error(), '|';
echo implode('+', preg_split('/,/u', "\xc3\xa9,b")), '|', preg_last_error(), '|';
var_dump(preg_split('/,/u', $bad, -1, PREG_SPLIT_OFFSET_CAPTURE));
echo preg_last_error(), '|', preg_last_error_msg();
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "bool(false)\n4|1|0|3|0|é+b|0|bool(false)\n4|Malformed UTF-8 characters, possibly incorrectly encoded"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}
