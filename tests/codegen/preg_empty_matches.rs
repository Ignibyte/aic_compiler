//! Purpose:
//! Regression coverage for `preg_replace()`, `preg_replace_callback()` and `preg_split()`
//! on empty subjects and zero-length matches, and for `preg_split()` limits.
//!
//! Called from:
//! - `cargo test --test codegen_tests preg_empty_matches` through Rust's test harness.
//!
//! Key details:
//! - Expected stdout was measured on PHP 8.5. An empty subject still gets one probe,
//!   so an empty match there is replaced, and a zero-length match keeps the byte after
//!   it, retrying in place before PCRE2 steps over one whole character (`/u`).
//! - Every probe runs over the whole subject, so `^`, `\b` and lookbehind see the text
//!   before the current offset, and split pieces and captures keep absolute offsets.
//! - A positive `preg_split()` limit counts only the pieces PHP keeps, so empty pieces
//!   dropped by `PREG_SPLIT_NO_EMPTY` and captured delimiters do not use it up.

use crate::support::*;

/// `preg_replace()` on an empty subject returns the empty string, or the replacement
/// when the pattern matches the empty string, instead of crashing.
#[test]
fn test_preg_empty_matches_replace_empty_subject() {
    let out = compile_and_run_with_regex(r#"<?php
foreach (['/x/', '//', '/x/u', '//u', '/x*/', '/^/', '/$/'] as $p) {
    $r = preg_replace($p, 'y', '');
    echo $p, '=', $r === null ? 'null' : "'" . $r . "'", ',', preg_last_error(), '|';
}
"#);
    assert_eq!(out, "/x/='',0|//='y',0|/x/u='',0|//u='y',0|/x*/='y',0|/^/='y',0|/$/='y',0|");
}

/// Zero-length matches keep the following byte, the final empty match is replaced,
/// and anchors, word boundaries and lookbehind see the whole subject.
#[test]
fn test_preg_empty_matches_replace_zero_length_and_anchors() {
    let out = compile_and_run_with_regex(r#"<?php
echo preg_replace('/x*/', '-', 'abc'), '|';
echo preg_replace('/\b/', '|', 'ab cd'), '|';
echo preg_replace('/a|/', '-', 'bab'), '|';
echo preg_replace('/^/', '>', 'ab'), '|';
echo preg_replace('/^a/', 'X', 'aaa'), '|';
echo preg_replace('/(?<=a)b/', 'X', 'abab'), '|';
echo preg_replace('/(a)(b)?/', '[$2$1]', 'aab'), '|';
echo preg_replace('/(b)/', '<$1>', 'abcb'), '|';
echo preg_replace('/$/m', ';', "a\nb"), '|';
"#);
    assert_eq!(out, "-a-b-c-||ab| |cd||-b--b-|>ab|Xaa|aXaX|[a][ba]|a<b>c<b>|a;\nb;|");
}

/// Under `/u` a zero-length match steps over one whole character, never into one.
#[test]
fn test_preg_empty_matches_replace_utf8_step() {
    let out = compile_and_run_with_regex(r#"<?php
echo preg_replace('//u', '|', "\u{e0}b"), '|';
echo preg_replace('/x*/u', '-', "\u{e9}\u{20ac}"), '|';
echo preg_replace('/(?=b)|$/u', '#', "\u{e0}b\u{1f600}"), '|';
echo strlen(preg_replace('//', '.', "\u{e0}")), '|';
"#);
    assert_eq!(out, "|\u{e0}|b||-\u{e9}-\u{20ac}-|\u{e0}#b\u{1f600}#|5|");
}

/// `preg_split()` keeps the bytes after zero-length separators, ends with the final
/// empty piece, steps over whole characters under `/u` and reports absolute offsets.
#[test]
fn test_preg_empty_matches_split() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function show(array|false $parts): string {
    if ($parts === false) {
        return 'false';
    }
    $out = [];
    foreach ($parts as $part) {
        $out[] = is_array($part) ? $part[0] . '@' . $part[1] : $part;
    }
    return count($out) . ':' . implode(',', $out);
}
echo show(preg_split('//', 'abc')), '|';
echo show(preg_split('//u', "\u{e0}b")), '|';
echo show(preg_split('//u', "\u{e0}b", -1, PREG_SPLIT_NO_EMPTY)), '|';
echo show(preg_split('//u', "\u{e0}b", -1, PREG_SPLIT_OFFSET_CAPTURE)), '|';
echo show(preg_split('/x*/', 'axb')), '|';
echo show(preg_split('//', '')), '|';
echo show(preg_split('/,/', '')), '|';
echo show(preg_split('/^a/', 'aaa')), '|';
echo show(preg_split('/(-)|(?=c)/', 'a-bc', -1, PREG_SPLIT_DELIM_CAPTURE | PREG_SPLIT_OFFSET_CAPTURE)), '|';
echo show(preg_split('//', 'abc', 2)), '|';
echo show(preg_split('/\b/', 'ab cd', -1, PREG_SPLIT_NO_EMPTY)), '|';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "5:,a,b,c,|4:,\u{e0},b,|2:\u{e0},b|4:@0,\u{e0}@0,b@2,@3|5:,a,,b,|2:,|1:|2:,aa|4:a@0,-@1,b@2,c@3|2:,abc|3:ab, ,cd|"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// `preg_replace_callback()` calls the callback for an empty match on an empty subject,
/// keeps the bytes between zero-length matches and replaces the final empty match.
#[test]
fn test_preg_empty_matches_callback_zero_length() {
    let out = compile_and_run_with_heap_debug(r#"<?php
$cb = function (array $m): string { return '[' . $m[0] . ']'; };
echo preg_replace_callback('//', $cb, ''), '|';
echo preg_replace_callback('/x/', $cb, ''), '|';
echo preg_replace_callback('//', $cb, 'ab'), '|';
echo preg_replace_callback('/x*/', $cb, 'abc'), '|';
echo preg_replace_callback('/x*/', $cb, 'axxbc'), '|';
echo preg_replace_callback('/a|/', $cb, 'bab'), '|';
echo preg_replace_callback('/$/m', $cb, "a\nb"), '|';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "[]||[]a[]b[]|[]a[]b[]c[]|[]a[xx][]b[]c[]|[]b[a][]b[]|a[]\nb[]|"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// `preg_replace_callback()` probes the whole subject, so `^`, `\b` and lookbehind see
/// the text before the current offset, and captures are read at absolute offsets.
#[test]
fn test_preg_empty_matches_callback_anchors_and_captures() {
    let out = compile_and_run_with_regex(r#"<?php
$cb = function (array $m): string { return '[' . $m[0] . ']'; };
echo preg_replace_callback('/^a/', $cb, 'aaa'), '|';
echo preg_replace_callback('/\b/', $cb, 'ab cd'), '|';
echo preg_replace_callback('/(?<=a)b/', $cb, 'abab'), '|';
echo preg_replace_callback('/a\Kb/', $cb, 'abab'), '|';
echo preg_replace_callback('/(a)(b)?/', function (array $m): string {
    return count($m) . ':' . implode(',', $m);
}, 'xaby a'), '|';
"#);
    assert_eq!(out, "[a]aa|[]ab[] []cd[]|a[b]a[b]|a[b]a[b]|x3:ab,a,by 2:a,a|");
}

/// Under `/u` a zero-length `preg_replace_callback()` match steps over one whole
/// character, and a malformed subject still returns null before any callback.
#[test]
fn test_preg_empty_matches_callback_utf8_step() {
    let out = compile_and_run_with_regex(r#"<?php
$cb = function (array $m): string { return '[' . $m[0] . ']'; };
echo preg_replace_callback('//u', $cb, "\u{e9}!"), '|';
echo preg_replace_callback('/x*/u', $cb, "\u{e9}\u{20ac}"), '|';
var_dump(preg_replace_callback('//u', $cb, "a\xff"));
echo preg_last_error(), '|';
"#);
    assert_eq!(out, "[]\u{e9}[]![]|[]\u{e9}[]\u{20ac}[]|NULL\n4|");
}

/// A positive `preg_split()` limit with `PREG_SPLIT_NO_EMPTY` counts only the pieces
/// kept, not the empty pieces it drops or the delimiters it captures.
#[test]
fn test_preg_empty_matches_split_no_empty_limit() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function show(array|false $parts): string {
    if ($parts === false) {
        return 'false';
    }
    $out = [];
    foreach ($parts as $part) {
        $out[] = is_array($part) ? $part[0] . '@' . $part[1] : $part;
    }
    return count($out) . ':' . implode(',', $out);
}
echo show(preg_split('/,/', ',a,b', 2, PREG_SPLIT_NO_EMPTY)), '|';
echo show(preg_split('/,/', ',,a,b,c', 3, PREG_SPLIT_NO_EMPTY)), '|';
echo show(preg_split('/,/', 'a,,b,,c', 2, PREG_SPLIT_NO_EMPTY)), '|';
echo show(preg_split('/(,)/', ',a,b', 2, PREG_SPLIT_NO_EMPTY | PREG_SPLIT_DELIM_CAPTURE)), '|';
echo show(preg_split('/,/', ',,a,b', 2, PREG_SPLIT_NO_EMPTY | PREG_SPLIT_OFFSET_CAPTURE)), '|';
echo show(preg_split('//', 'abc', 2, PREG_SPLIT_NO_EMPTY)), '|';
echo show(preg_split('/,/', ',a,b', 2)), '|';
echo show(preg_split('/,/', ',a,b', 1, PREG_SPLIT_NO_EMPTY)), '|';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "2:a,b|3:a,b,c|2:a,,b,,c|4:,,a,,,b|2:a@2,b@4|2:a,bc|2:,a,b|1:,a,b|"
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}
