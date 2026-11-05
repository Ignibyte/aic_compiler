//! Purpose:
//! End-to-end tests for the PHP-written stdlib prelude (`src/stdlib_prelude/*.php`): `pack` and
//! `unpack`.
//!
//! Called from:
//! - `cargo test --test codegen_tests stdlib_builtins` through Rust's test harness.
//!
//! Key details:
//! - Each fixture's expected output was generated with `php` 8.5, so these assert byte-for-byte
//!   PHP parity.

use crate::support::*;

/// Parity with PHP 8.5.10 for the `namespaced_callable` probe.
#[test]
fn test_stdlib_namespaced_callable_matches_php() {
    let source = r####"<?php
namespace App\Util;
var_dump(function_exists('pack'), function_exists('unpack'));
$f = unpack(...);
echo $f('n', "\x01\x02")[1], "|", call_user_func('pack', 'C', 65), "\n";
echo bin2hex(\pack("N", 258)), "|", bin2hex(pack("v", 258)), PHP_EOL;
"####;
    let expected = r####"bool(true)
bool(true)
258|A
00000102|0201
"####;
    assert_eq!(compile_and_run(source), expected);
}

/// Parity with PHP 8.5.10 for pack()/unpack() as vendor code uses them.
#[test]
fn test_stdlib_pack_vendor_usage_matches_php() {
    let source = r####"<?php
echo unpack('N', "\x00\x00\x00\x2a")[1], "\n";
$hex = (array) unpack('H*hex', "\xde\xad\xbe\xef");
echo $hex['hex'], "\n";
echo bin2hex(pack('H32', md5('abc'))), "\n";
$words = [1, 2, 258];
echo bin2hex(pack('V*', ...$words)), "\n";
echo bin2hex(pack('n*', ...array_reverse([1, 2, 3]))), "\n";
$u = unpack('n*', "\x00\x01\x00\x02");
echo implode(",", array_values($u)), "\n";
echo implode(",", array_values(unpack('C*', "AB"))), "\n";
$r = unpack('N2', "\x00\x00\x00\x01\x00\x00\x00\x02");
echo $r[1] + $r[2], "\n";
echo bin2hex(pack('E', 1.0)) === "3ff0000000000000" ? "ok" : "bad", "\n";
echo unpack('J', pack('E', 1.0))[1], "\n";
echo bin2hex(pack('vC2v4C12@32', 1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)), "\n";
"####;
    let expected = r####"42
deadbeef
900150983cd24fb0d6963f7d28e17f72
010000000200000002010000
000300020001
1,2
65,66
3
ok
4607182418800017408
0100020304000500060007000102030405060708090a0b0c0000000000000000
"####;
    assert_eq!(compile_and_run(source), expected);
}

/// Parity with PHP 8.5.10 for the `pack_unpack` probe.
#[test]
fn test_stdlib_pack_unpack_matches_php() {
    let source = r####"<?php
echo bin2hex(pack("nvc*", 0x1234, 0x5678, 65, 66)), "\n";
echo bin2hex(pack("NVJPq", 1, 2, 3, 4, -5)), "\n";
echo bin2hex(pack("A5a5Z5", "ab", "cd", "efghij")), "\n";
echo bin2hex(pack("H*h*", "1f2e3", "1f2e3")), "\n";
echo bin2hex(pack("vC2v4C12@32", 1, 2, 3, 4, 5, 6, 7, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)), "\n";
echo bin2hex(pack("EeGgdf", 1.5, -2.25, 0.1, 3.0, 1e100, 0.1)), "\n";
echo bin2hex(pack("s2x2X1", -2, 300)), "\n";
print_r(unpack("N", "\x00\x00\x01\x00"));
print_r(unpack("nbig/vlittle/Cbyte", "\x01\x02\x01\x02\xff"));
print_r(unpack("C*", "abc"));
print_r(unpack("n2x/a3s", "\x00\x01\x00\x02abc"));
print_r(unpack("H*hex", "\x1f\xe2"));
print_r(unpack("c2chars/a*rest", "\xff\x7fzz"));
print_r(unpack("Jv", pack("J", PHP_INT_MAX)));
print_r(unpack("qv/Qw", pack("qQ", -7, 7)));
print_r(unpack("Ef/Gg/dd", pack("EGd", 1.5, -0.5, 3.14159)));
print_r(unpack("a2x/A3y/Z*z", "hi  a b\0qqq"));
print_r(unpack("N", "xxxx\x00\x00\x00\x07", 4));
"####;
    let expected = r####"123478564142
000000010200000000000000000000030400000000000000fbffffffffffffff
616220202063640000006566676800
1f2e30f1e203
0100020304000500060007000102030405060708090a0b0c0000000000000000
3ff800000000000000000000000002c03dcccccd000040407dc39425ad49b254cdcccc3d
feff2c0100
Array
(
    [1] => 256
)
Array
(
    [big] => 258
    [little] => 513
    [byte] => 255
)
Array
(
    [1] => 97
    [2] => 98
    [3] => 99
)
Array
(
    [x1] => 1
    [x2] => 2
    [s] => abc
)
Array
(
    [hex] => 1fe2
)
Array
(
    [chars1] => -1
    [chars2] => 127
    [rest] => zz
)
Array
(
    [v] => 9223372036854775807
)
Array
(
    [v] => -7
    [w] => 7
)
Array
(
    [f] => 1.5
    [g] => -0.5
    [d] => 3.14159
)
Array
(
    [x] => hi
    [y] =>   a
    [z] =>  b
)
Array
(
    [1] => 7
)
"####;
    assert_eq!(compile_and_run(source), expected);
}

/// Vendor / PSR-4 classes pulled in by the autoloader see the prelude functions, both
/// fully qualified and through the unqualified namespace fallback.
#[test]
fn test_stdlib_prelude_is_visible_to_autoloaded_psr4_classes() {
    let out = compile_and_run_files(
        &[
            ("composer.json", r#"{"autoload":{"psr-4":{"App\\":"src/"}}}"#),
            (
                "src/Tok.php",
                r#"<?php
namespace App;
class Tok {
    public static function make(): string {
        return bin2hex(pack("n", 258)) . "|" . \unpack("V", "\x2a\x00\x00\x00")[1];
    }
}
"#,
            ),
            ("main.php", "<?php\necho App\\Tok::make(), \"\\n\";\n"),
        ],
        "main.php",
    );
    assert_eq!(out, "0102|42\n");
}
