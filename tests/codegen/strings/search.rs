//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of strings search, including substr basic, substr with length, and substr negative offset.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Verifies substr extracts the suffix starting at a positive offset.
/// Fixture: "Hello World" with offset 6 returns "World".
#[test]
fn test_substr_basic() {
    let out = compile_and_run(r#"<?php echo substr("Hello World", 6);"#);
    assert_eq!(out, "World");
}

/// Verifies substr respects a length parameter to limit the extraction.
/// Fixture: "Hello World" with offset 0 and length 5 returns "Hello".
#[test]
fn test_substr_with_length() {
    let out = compile_and_run(r#"<?php echo substr("Hello World", 0, 5);"#);
    assert_eq!(out, "Hello");
}

/// Verifies substr interprets a negative offset as distance from the end of the string.
/// Fixture: "Hello World" with offset -5 returns "World".
#[test]
fn test_substr_negative_offset() {
    let out = compile_and_run(r#"<?php echo substr("Hello World", -5);"#);
    assert_eq!(out, "World");
}

/// Verifies substr accepts a non-negative integer offset derived from a function return via addition.
/// Regression test: int-to-integer coercion path for the offset expression `$o + 1`.
/// Fixture: queries with `?` delimiter, strpos + intval, then substr with +1 offset.
#[test]
fn test_substr_coerces_mixed_numeric_offset_from_function_return_add() {
    let out = compile_and_run(
        r#"<?php
function get_index(string $s): int {
    $p = strpos($s, "?");
    return intval($p);
}
function slice_after(string $s): string {
    $o = get_index($s);
    $p = $o + 1;
    return substr($s, $p);
}
echo slice_after("/hello?name=elephc"), "\n";
echo substr("/hello?name=elephc", get_index("/hello?name=elephc") + 1), "\n";
"#,
    );
    assert_eq!(out, "name=elephc\nname=elephc\n");
}

/// Verifies strpos returns the integer byte offset when the needle is found.
/// Fixture: "Hello World" contains "World" starting at offset 6.
#[test]
fn test_strpos_found() {
    let out = compile_and_run(r#"<?php echo strpos("Hello World", "World");"#);
    assert_eq!(out, "6");
}

/// Verifies strpos returns empty string when the needle is absent.
/// Fixture: "Hello" does not contain "xyz".
#[test]
fn test_strpos_not_found() {
    let out = compile_and_run(r#"<?php echo strpos("Hello", "xyz");"#);
    assert_eq!(out, "");
}

/// Verifies strpos uses strict `=== false` comparison when the needle is not found.
/// Fixture: strpos on "Hello"/"xyz" is strict-false, not just falsy.
#[test]
fn test_strpos_not_found_is_strict_false() {
    let out = compile_and_run(r#"<?php echo strpos("Hello", "xyz") === false ? "miss" : "hit";"#);
    assert_eq!(out, "miss");
}

/// Verifies assignment of strpos result to a variable preserves strict-false semantics.
/// Fixture: `$pos = strpos(...)` then strict comparison against false.
#[test]
fn test_strpos_assigned_not_found_is_strict_false() {
    let out = compile_and_run(
        r#"<?php
$pos = strpos("Hello", "xyz");
echo $pos === false ? "miss" : "hit";
"#,
    );
    assert_eq!(out, "miss");
}

/// Verifies strpos returns 0 (not false) when the needle is at the start of the string.
/// Regression: zero is a valid offset and must not be confused with the false sentinel.
/// Fixture: "abc" contains "a" at offset 0, which is !== false.
#[test]
fn test_strpos_zero_offset_is_not_false() {
    let out = compile_and_run(r#"<?php echo strpos("abc", "a") === false ? "miss" : "zero";"#);
    assert_eq!(out, "zero");
}

/// Verifies strrpos finds the last occurrence of a needle.
/// Fixture: "abcabc" last "bc" starts at offset 4.
#[test]
fn test_strrpos() {
    let out = compile_and_run(r#"<?php echo strrpos("abcabc", "bc");"#);
    assert_eq!(out, "4");
}

/// Verifies strrpos returns strict false when the needle is absent.
/// Fixture: "abcabc" does not contain "zz".
#[test]
fn test_strrpos_not_found_is_strict_false() {
    let out = compile_and_run(r#"<?php echo strrpos("abcabc", "zz") === false ? "miss" : "hit";"#);
    assert_eq!(out, "miss");
}

/// Verifies strstr returns the portion of the string starting from the first needle occurrence.
/// Fixture: "user@example.com" split on "@" yields "@example.com".
#[test]
fn test_strstr_found() {
    let out = compile_and_run(r#"<?php echo strstr("user@example.com", "@");"#);
    assert_eq!(out, "@example.com");
}

/// Verifies strcmp returns 0 when two identical strings compare equal.
#[test]
fn test_strcmp_equal() {
    let out = compile_and_run(r#"<?php echo strcmp("abc", "abc");"#);
    assert_eq!(out, "0");
}

/// Verifies strcmp returns a negative value when the first string sorts before the second.
/// Fixture: "abc" < "abd" lexicographically.
#[test]
fn test_strcmp_less() {
    let out = compile_and_run(r#"<?php echo (strcmp("abc", "abd") < 0 ? "yes" : "no");"#);
    assert_eq!(out, "yes");
}

/// Verifies strcasecmp performs case-insensitive string comparison, returning 0 for equal strings.
#[test]
fn test_strcasecmp() {
    let out = compile_and_run(r#"<?php echo strcasecmp("Hello", "hello");"#);
    assert_eq!(out, "0");
}

/// Verifies the injected `substr_compare()` prelude (PHP's substr_compare is not a native builtin,
/// so a call was "Undefined function"). Covers the case-insensitive keyword-match use (content
/// scanners), a mid-string offset, ordering sign, and case-sensitive match vs mismatch. Reduced
/// to substr + strcmp/strcasecmp with each argument evaluated once.
#[test]
fn test_substr_compare() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
$css = "  COLOR: red";
$keyword = "color";
echo substr_compare(substr($css, 2), $keyword, 0, strlen($keyword), true) === 0 ? "1" : "0";
echo substr_compare("Hello", "ell", 1, 3) === 0 ? "1" : "0";
echo substr_compare("Hello", "world", 0, 5) < 0 ? "1" : "0";
echo substr_compare("abcABC", "abc", 0, 3, true) === 0 ? "1" : "0";
echo substr_compare("abcABC", "abc", 0, 3, false) === 0 ? "1" : "0";
echo substr_compare("ABCdef", "abc", 0, 3, false) === 0 ? "1" : "0";
"#,
    );
    assert_eq!(out, "111110");
}

/// Verifies `strncmp` / `strncasecmp` (length-bounded string compare, absent natively — they
/// desugar to injected helpers). Equal prefixes compare 0; a difference beyond the length is
/// ignored; the case-insensitive form ignores case within the compared length.
#[test]
fn test_strncmp_and_strncasecmp() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
echo strncmp("abcXYZ", "abcQQQ", 3) === 0 ? "1" : "0";
echo strncmp("abc", "abd", 3) < 0 ? "1" : "0";
echo strncasecmp("ABCxyz", "abc", 3) === 0 ? "1" : "0";
echo strncmp("ABC", "abc", 3) === 0 ? "1" : "0";
"#,
    );
    assert_eq!(out, "1110");
}

/// Verifies str_contains returns 1 when the needle is present in the haystack.
/// Fixture: "Hello World" contains "World".
#[test]
fn test_str_contains_true() {
    let out = compile_and_run(r#"<?php echo str_contains("Hello World", "World");"#);
    assert_eq!(out, "1");
}

/// Verifies str_contains returns empty string when the needle is absent.
/// Fixture: "Hello" does not contain "xyz".
#[test]
fn test_str_contains_false() {
    let out = compile_and_run(r#"<?php echo str_contains("Hello", "xyz");"#);
    assert_eq!(out, "");
}

/// Verifies str_starts_with returns 1 when the haystack starts with the needle.
/// Fixture: "Hello World" starts with "Hello".
#[test]
fn test_str_starts_with_true() {
    let out = compile_and_run(r#"<?php echo str_starts_with("Hello World", "Hello");"#);
    assert_eq!(out, "1");
}

/// Verifies str_starts_with returns empty string when the haystack does not start with the needle.
/// Fixture: "Hello" does not start with "World".
#[test]
fn test_str_starts_with_false() {
    let out = compile_and_run(r#"<?php echo str_starts_with("Hello", "World");"#);
    assert_eq!(out, "");
}

/// Verifies str_ends_with returns 1 when the haystack ends with the needle.
/// Fixture: "Hello World" ends with "World".
#[test]
fn test_str_ends_with_true() {
    let out = compile_and_run(r#"<?php echo str_ends_with("Hello World", "World");"#);
    assert_eq!(out, "1");
}

/// Verifies str_ends_with returns empty string when the haystack does not end with the needle.
/// Fixture: "Hello" does not end with "xyz".
#[test]
fn test_str_ends_with_false() {
    let out = compile_and_run(r#"<?php echo str_ends_with("Hello", "xyz");"#);
    assert_eq!(out, "");
}

/// Verifies substr_replace replaces a substring at a given offset and length with the replacement string.
/// Fixture: "hello world" replaced at offset 6, length 5 with "PHP" yields "hello PHP".
#[test]
fn test_substr_replace() {
    let out = compile_and_run(r#"<?php echo substr_replace("hello world", "PHP", 6, 5);"#);
    assert_eq!(out, "hello PHP");
}

/// Verifies substr_replace replaces from offset to end of string when length is omitted.
/// Fixture: "hello world" replaced at offset 5 with "!" yields "hello!".
#[test]
fn test_substr_replace_no_length() {
    let out = compile_and_run(r#"<?php echo substr_replace("hello world", "!", 5);"#);
    assert_eq!(out, "hello!");
}

/// Verifies `mb_strpos` returns a UTF-8 CODE-POINT index (not a byte offset): the needle
/// "llo" in "héllo" is at code-point 2 even though "é" occupies two bytes. Also covers a
/// multibyte needle, an ASCII match at 0, a miss (strict `false`), and an empty needle (0).
/// Fixture asserts `2|1|0|F|0|2`.
#[test]
fn test_mb_strpos_codepoint_offsets() {
    let out = compile_and_run(
        r#"<?php
echo mb_strpos("héllo", "llo"), "|";
echo mb_strpos("héllo", "é"), "|";
echo mb_strpos("abc", "a"), "|";
echo (mb_strpos("abc", "z") === false ? "F" : "X"), "|";
echo mb_strpos("abc", ""), "|";
echo mb_strpos("日本語", "語");
"#,
    );
    assert_eq!(out, "2|1|0|F|0|2");
}

/// Verifies the 3-argument `strtr($str, $from, $to)` byte-map form: equal-length maps,
/// PHP's "last mapping wins" for a repeated `from` byte (`strtr("test","tt","12")` → "2es2"),
/// truncation to `min(len($from), len($to))` (extra `from` byte ignored), a mixed-case map,
/// and single-byte translation.
/// Fixture asserts `Hippo|2es2|xyc|1324|bbb`.
#[test]
fn test_strtr_three_arg_char_map() {
    let out = compile_and_run(
        r#"<?php
echo strtr("Hello", "el", "ip"), "|";
echo strtr("test", "tt", "12"), "|";
echo strtr("abc", "abcd", "xy"), "|";
echo strtr("aAbB", "abAB", "1234"), "|";
echo strtr("aaa", "a", "b");
"#,
    );
    assert_eq!(out, "Hippo|2es2|xyc|1324|bbb");
}

/// Regression: `strtr($str, $pairs)` — the two-argument replacement-map form — desugars to the
/// injected `__elephc_strtr_pairs` and applies PHP's longest-match-wins, non-re-scanned,
/// empty-key-ignored semantics. The three-argument single-character-translation form stays the
/// native builtin. Kernel/scaffold template renderers use the pair-map form for
/// placeholder substitution.
#[test]
fn test_strtr_two_arg_replacement_pairs() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
$tpl = strtr("Hello {name}, you are {role}", ["{name}" => "Al", "{role}" => "admin"]);
$longest = strtr("hell", ["h" => "1", "hell" => "Z"]);
$native = strtr("abc", "abc", "xyz");
echo $tpl, "|", $longest, "|", $native;
"#,
    );
    assert_eq!(out, "Hello Al, you are admin|Z|xyz");
}

/// Regression: `strpos($haystack, $needle, $offset)` — the three-argument start-offset form —
/// desugars to the injected `__elephc_strpos_offset` (offset-slice + native 2-arg strpos,
/// re-based; negative offsets count from the end). The 2-arg form stays native.
#[test]
fn test_strpos_three_arg_offset() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
$a = strpos("abcabc", "bc", 2);
$b = strpos("abcabc", "z", 0) === false ? "F" : "?";
$c = strpos("hello", "l");
echo $a, "|", $b, "|", $c;
"#,
    );
    assert_eq!(out, "4|F|2");
}

/// Regression: `strcspn($subject, $characters)` (the length of the initial run of `$subject`
/// containing none of the bytes in `$characters`) had no native implementation — it desugars
/// to the injected `__elephc_strcspn`. ward-components' CssTokenScanner scans with it.
#[test]
fn test_strcspn_two_arg() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
$a = strcspn("header text\n{body}", "\n{");
$b = strcspn("abc", "xyz");
$c = strcspn("xabc", "x");
echo $a, "|", $b, "|", $c;
"#,
    );
    assert_eq!(out, "11|3|0");
}
