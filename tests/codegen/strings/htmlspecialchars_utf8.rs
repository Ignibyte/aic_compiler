//! Purpose:
//! Regression tests for `htmlspecialchars()` on invalid UTF-8: `ENT_SUBSTITUTE`, `ENT_IGNORE` and neither.
//!
//! Called from:
//! - `cargo test` through `tests/codegen/strings/encoding.rs`.
//!
//! Key details:
//! - Expected bytes are PHP 8.5's own output, printed as hex so no invalid byte sits in a Rust literal.
//! - Invalid sequences are one to four bytes long exactly as php-src `get_next_char()` counts them.

use super::*;

/// Under UTF-8, `htmlspecialchars()` writes U+FFFD for each invalid sequence with `ENT_SUBSTITUTE` (the
/// default flags), drops it with `ENT_IGNORE`, and returns the empty string with neither; a non-UTF-8
/// `$encoding` keeps the bytes. Lone, truncated, overlong, surrogate and out-of-range sequences, and a lead
/// byte followed by ASCII. The bytes passed through before #158.
#[test]
fn test_htmlspecialchars_invalid_utf8_follows_its_flags() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$cases = ["a\xffb", "\xc3", "\xc3<", "\xe2\x82", "\xe2\x82A", "\xe0\x80\x80", "\xed\xa0\x80", "\xf4\x90\x80\x80", "\xf0\x9f\x98\x80", "caf\xc3\xa9", "\xf5", "\xc0\xaf", "\xf0\x9f\x98", "\xf0\x9f\x98<", "\x80\x80", "\xe2\x82\xac&\"'"];
foreach ($cases as $s) {
    echo bin2hex(htmlspecialchars($s)), ' ', bin2hex(htmlspecialchars($s, ENT_QUOTES | ENT_IGNORE, 'UTF-8')), ' ', bin2hex(htmlspecialchars($s, ENT_QUOTES, 'UTF-8')), ' ', bin2hex(htmlspecialchars($s, ENT_QUOTES | ENT_SUBSTITUTE, 'ISO-8859-1')), "\n";
}
"#,
    );
    assert_eq!(
        out.stdout,
        concat!(
            "61efbfbd62 6162  61ff62\n",
            "efbfbd   c3\n",
            "efbfbd266c743b 266c743b  c3266c743b\n",
            "efbfbd   e282\n",
            "efbfbd41 41  e28241\n",
            "efbfbd   e08080\n",
            "efbfbd   eda080\n",
            "efbfbd   f4908080\n",
            "f09f9880 f09f9880 f09f9880 f09f9880\n",
            "636166c3a9 636166c3a9 636166c3a9 636166c3a9\n",
            "efbfbd   f5\n",
            "efbfbdefbfbd   c0af\n",
            "efbfbd   f09f98\n",
            "efbfbd266c743b 266c743b  f09f98266c743b\n",
            "efbfbdefbfbd   8080\n",
            "e282ac26616d703b2671756f743b26233033393b e282ac26616d703b2671756f743b26233033393b e282ac26616d703b2671756f743b26233033393b e282ac26616d703b2671756f743b26233033393b\n",
        )
    );
}

/// A string of many invalid bytes grows threefold under `ENT_SUBSTITUTE` and stays inside the reservation;
/// `ENT_IGNORE` keeps only the valid characters.
#[test]
fn test_htmlspecialchars_invalid_utf8_long_input() {
    let out = compile_and_run_with_heap_debug(
        r#"<?php
$s = str_repeat("\xff<\xc3\xa9", 20000);
echo strlen(htmlspecialchars($s)), " ", strlen(htmlspecialchars($s, ENT_QUOTES | ENT_IGNORE)), " ", strlen(htmlspecialchars($s, ENT_QUOTES)), "\n";
"#,
    );
    assert_eq!(out.stdout, "180000 120000 0\n");
}
