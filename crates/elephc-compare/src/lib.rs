//! Purpose:
//! Pure-Rust PHP 8 numeric-string-aware string comparison for elephc's ordered
//! (`<`/`<=`/`>`/`>=`) and spaceship (`<=>`) operators over `string` operands.
//! Replicates PHP's `zendi_smart_strcmp` + `is_numeric_string_ex`
//! (`Zend/zend_operators.c`): two numeric strings compare numerically —
//! integer-exact when both fit `i64`, by overflow sign or `double` otherwise —
//! and everything else compares by byte sequence and length. Byte-for-byte
//! identical to PHP 8.5.
//!
//! Called from:
//! - Compiled PHP programs via the `__rt_php_str_compare` C ABI (linked from
//!   `libelephc_compare.a` when codegen lowers a string comparison — see
//!   `src/codegen/lower_inst/comparisons.rs` and the pipeline's asm-scan gate).
//! - `cargo test -p elephc-compare` (the rlib) against a PHP 8.5 oracle.
//!
//! Key details:
//! - `is_numeric_string_ex` grammar (PHP 8): optional leading AND trailing ASCII
//!   whitespace, optional sign, decimal digits with an optional fraction and an
//!   optional `e`/`E` exponent; NO hex/binary/underscore/`inf`/`nan`; an empty
//!   mantissa or any trailing non-whitespace byte => not numeric. A pure integer
//!   that fits `i64` is `Long`; a fraction/exponent is `Double`; a pure integer
//!   that overflows `i64` is `Double` carrying an overflow sign (`-1`/`+1`) so it
//!   still orders exactly against an in-range `i64`.
//! - The integer-exact path is load-bearing: `"9007199254740993" <=> "9007199254740992"`
//!   (2^53+1 vs 2^53) must be `1`, which a double-only comparison would wrongly
//!   collapse to `0` (both round to 2^53). See ticket #691 "THE TRAP".

/// PHP numeric-string classification of a byte string.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Classification {
    /// Not a PHP numeric string (empty mantissa, trailing garbage, hex, `inf`/`nan`, ...).
    NotNumeric,
    /// A decimal integer that fits in `i64`.
    Long(i64),
    /// A float, OR an integer that overflowed `i64`. The second field is the
    /// overflow sign (PHP's `oflow_info`): `0` for a genuine float, `+1`/`-1` for
    /// an out-of-range positive/negative integer.
    Double(f64, i8),
}

/// ASCII whitespace PHP's numeric-string scanner skips: space, tab, newline,
/// carriage return, vertical tab, form feed.
fn is_php_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

/// Classifies `bytes` as a PHP 8 numeric string. Mirrors `is_numeric_string_ex`
/// with `allow_errors = 0`: any trailing non-whitespace byte means "not numeric".
fn classify(bytes: &[u8]) -> Classification {
    let n = bytes.len();
    let mut i = 0;

    // Leading whitespace.
    while i < n && is_php_ws(bytes[i]) {
        i += 1;
    }

    let num_start = i;
    // Optional sign.
    let negative = if i < n && (bytes[i] == b'+' || bytes[i] == b'-') {
        let neg = bytes[i] == b'-';
        i += 1;
        neg
    } else {
        false
    };

    // Integer digits.
    let int_start = i;
    while i < n && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;

    // Optional fraction: `.` followed by zero or more digits.
    let mut is_double = false;
    let mut frac_digits = 0usize;
    if i < n && bytes[i] == b'.' {
        is_double = true;
        i += 1;
        let frac_start = i;
        while i < n && bytes[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - frac_start;
    }

    // A numeric string needs at least one mantissa digit (`.` alone is not numeric).
    if int_digits + frac_digits == 0 {
        return Classification::NotNumeric;
    }

    // Optional exponent: `e`/`E`, optional sign, one or more digits. If the `e` is
    // not followed by a valid exponent it is left as trailing garbage (rejected below).
    if i < n && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < n && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let exp_start = j;
        while j < n && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_start {
            is_double = true;
            i = j;
        }
    }

    let num_end = i;
    // Trailing whitespace (PHP 8 allows it).
    while i < n && is_php_ws(bytes[i]) {
        i += 1;
    }
    // Any remaining byte is trailing non-whitespace garbage => not a numeric string.
    if i != n {
        return Classification::NotNumeric;
    }

    // `num_start..num_end` is pure ASCII: `[sign] digits [. digits] [e [sign] digits]`.
    let text = match core::str::from_utf8(&bytes[num_start..num_end]) {
        Ok(text) => text,
        Err(_) => return Classification::NotNumeric,
    };

    if is_double {
        return Classification::Double(text.parse::<f64>().unwrap_or(0.0), 0);
    }
    // Pure integer: exact when it fits `i64`, otherwise an overflow-`double`.
    match text.parse::<i64>() {
        Ok(value) => Classification::Long(value),
        Err(_) => Classification::Double(
            text.parse::<f64>().unwrap_or(0.0),
            if negative { -1 } else { 1 },
        ),
    }
}

/// Maps an `Ordering` to PHP's `-1`/`0`/`1`.
fn ord_i32(ordering: core::cmp::Ordering) -> i32 {
    match ordering {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

/// Compares two finite doubles to `-1`/`0`/`1` (no NaN reaches here — `classify`
/// never yields a numeric result for `nan`).
fn cmp_f64(a: f64, b: f64) -> i32 {
    if a < b {
        -1
    } else if a > b {
        1
    } else {
        0
    }
}

/// Byte-and-length comparison — PHP's `zend_binary_strcmp` normalized to `-1`/`0`/`1`.
/// Rust's slice `Ord` is exactly memcmp-then-shorter-is-less.
fn byte_cmp(a: &[u8], b: &[u8]) -> i32 {
    ord_i32(a.cmp(b))
}

/// PHP 8 smart string comparison (`zendi_smart_strcmp`), returning `-1`/`0`/`1`.
///
/// Two numeric strings compare numerically; if either operand is not numeric the
/// comparison falls back to bytes. Two integers that overflowed `i64` in the same
/// direction also fall back to bytes (their doubles may be indistinguishable), as
/// PHP does.
pub fn php_str_compare(a: &[u8], b: &[u8]) -> i32 {
    use Classification::{Double, Long, NotNumeric};
    match (classify(a), classify(b)) {
        (NotNumeric, _) | (_, NotNumeric) => byte_cmp(a, b),
        // Both integers overflowed i64 in the same direction: PHP compares bytes.
        (Double(_, oa), Double(_, ob)) if oa != 0 && oa == ob => byte_cmp(a, b),
        (Long(x), Long(y)) => ord_i32(x.cmp(&y)),
        // A LONG against an overflowed integer: the overflow's magnitude dominates,
        // so the sign is `-oflow` (b bigger when b overflowed positive, etc.).
        (Long(x), Double(dy, oy)) => {
            if oy != 0 {
                (-oy) as i32
            } else {
                cmp_f64(x as f64, dy)
            }
        }
        (Double(dx, ox), Long(y)) => {
            if ox != 0 {
                ox as i32
            } else {
                cmp_f64(dx, y as f64)
            }
        }
        // Reached only when NOT both-overflowed-same-direction: both genuine floats,
        // one overflow + one float, or overflows in opposite directions. PHP compares
        // the doubles directly.
        (Double(dx, _), Double(dy, _)) => cmp_f64(dx, dy),
    }
}

/// C ABI entry: compares two PHP byte strings with PHP 8 numeric-string semantics,
/// returning `-1`, `0`, or `1` widened to `i64` for the caller's integer result
/// register. The register convention matches `__rt_strcmp` on x86-64 (SysV:
/// rdi/rsi/rdx/rcx → rax); on AArch64 the caller loads x0–x3 for this C-ABI entry.
///
/// # Safety
/// `a_ptr` must point to `a_len` readable bytes (or be null with `a_len == 0`);
/// likewise `b_ptr`/`b_len`. The compiled caller always passes a live PHP string
/// descriptor's `{ptr, len}` pair.
#[no_mangle]
pub unsafe extern "C" fn __rt_php_str_compare(
    a_ptr: *const u8,
    a_len: usize,
    b_ptr: *const u8,
    b_len: usize,
) -> i64 {
    php_str_compare(slice_from(a_ptr, a_len), slice_from(b_ptr, b_len)) as i64
}

/// Reconstructs a byte slice from a compiled PHP string descriptor's `{ptr, len}`.
///
/// # Safety
/// Same contract as [`__rt_php_str_compare`].
unsafe fn slice_from<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    if ptr.is_null() || len == 0 {
        &[]
    } else {
        core::slice::from_raw_parts(ptr, len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The comparison oracle: `(a, b, a <=> b)` captured from PHP 8.5 with
    /// `declare(strict_types=1)` and `string`-typed operands (see the #691 spec's
    /// `oracle.php`). Covers non-numeric byte compares, integer vs float, the
    /// i64-overflow sign rows, the 2^53±1 integer-exactness trap, both-overflow
    /// byte fallback, whitespace trimming, leading zeros, and the hex/underscore/
    /// `inf`/`nan`/bare-`e` "not numeric" rows.
    const ORACLE: &[(&str, &str, i32)] = &[
        ("abc", "abd", -1),
        ("10", "9", 1),
        ("10", "9.5", 1),
        ("a", "1", 1),
        ("10", "9a", -1),
        ("0", "", 1),
        ("1e3", "1000", 0),
        ("01", "1", 0),
        ("-5", "-10", 1),
        ("5.", ".5", 1),
        ("0.0", "0", 0),
        ("007", "7", 0),
        ("9223372036854775807", "9223372036854775808", -1), // i64max vs i64max+1
        ("9223372036854775808", "9223372036854775807", 1),  // reversed
        ("9007199254740993", "9007199254740992", 1),        // 2^53+1 vs 2^53 (THE TRAP)
        ("-9223372036854775808", "-9223372036854775809", 1), // i64min vs i64min-1
        (" 1 ", "1", 0),
        ("1", " 1", 0),
        ("1", "1 ", 0),
        ("  10", "9", 1),
        ("", "", 0),
        ("1e", "1", 1),   // bare 'e' => not numeric => byte compare
        ("0x1A", "26", -1), // hex => not numeric
        ("1_0", "10", 1),   // underscore => not numeric
        ("inf", "1", 1),    // inf => not numeric
        ("nan", "1", 1),    // nan => not numeric
        ("+5", "5", 0),
        ("+5", "-5", 1),
        ("100", "20", 1),
        ("100", "99", 1),
        ("3.14", "3.2", -1),
        ("1.0", "1", 0),
        ("999999999999999999999", "1000000000000000000000", 1), // both overflow +, byte compare
        ("-999999999999999999999", "-1000000000000000000000", 1), // both overflow -, byte compare
        ("12345", "0012345", 0),
        ("0", "-0", 0),
        ("1e2", "100", 0),
        ("1E2", "100", 0),
    ];

    #[test]
    fn matches_php_oracle() {
        for &(a, b, expected) in ORACLE {
            let got = php_str_compare(a.as_bytes(), b.as_bytes());
            assert_eq!(
                got, expected,
                "php_str_compare({a:?}, {b:?}) = {got}, expected {expected}",
            );
        }
    }

    #[test]
    fn oracle_is_antisymmetric() {
        // `a <=> b == -(b <=> a)` must hold for every pair (a sanity check the
        // classification/comparison never produces an inconsistent ordering).
        for &(a, b, expected) in ORACLE {
            let reverse = php_str_compare(b.as_bytes(), a.as_bytes());
            assert_eq!(
                reverse, -expected,
                "reverse of ({a:?}, {b:?}) = {reverse}, expected {}",
                -expected,
            );
        }
    }

    #[test]
    fn classifies_edge_cases() {
        use Classification::*;
        assert_eq!(classify(b"9a"), NotNumeric); // trailing garbage
        assert_eq!(classify(b"1e"), NotNumeric); // bare exponent
        assert_eq!(classify(b"0x1A"), NotNumeric); // hex
        assert_eq!(classify(b"1_0"), NotNumeric); // underscore
        assert_eq!(classify(b"inf"), NotNumeric);
        assert_eq!(classify(b"nan"), NotNumeric);
        assert_eq!(classify(b""), NotNumeric);
        assert_eq!(classify(b"."), NotNumeric); // no mantissa digit
        assert_eq!(classify(b"+"), NotNumeric);
        assert_eq!(classify(b"007"), Long(7));
        assert_eq!(classify(b" 1 "), Long(1)); // leading + trailing ws
        assert_eq!(classify(b"+5"), Long(5));
        assert_eq!(classify(b"-0"), Long(0));
        assert!(matches!(classify(b"5."), Double(v, 0) if v == 5.0));
        assert!(matches!(classify(b".5"), Double(v, 0) if v == 0.5));
        assert!(matches!(classify(b"1e3"), Double(v, 0) if v == 1000.0));
        assert_eq!(classify(b"9223372036854775807"), Long(i64::MAX));
        assert_eq!(classify(b"-9223372036854775808"), Long(i64::MIN));
        assert!(matches!(classify(b"9223372036854775808"), Double(_, 1))); // i64max+1
        assert!(matches!(classify(b"-9223372036854775809"), Double(_, -1))); // i64min-1
    }

    #[test]
    fn c_abi_entry_handles_null_as_empty() {
        // A null pointer with zero length is the empty string; "0" > "" per PHP.
        let got = unsafe { __rt_php_str_compare(b"0".as_ptr(), 1, core::ptr::null(), 0) };
        assert_eq!(got, 1);
        let empty_both = unsafe { __rt_php_str_compare(core::ptr::null(), 0, core::ptr::null(), 0) };
        assert_eq!(empty_both, 0);
    }
}
