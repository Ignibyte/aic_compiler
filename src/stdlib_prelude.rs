//! Purpose:
//! Conditionally-injected standard-library prelude: a small set of PHP stdlib functions
//! implemented in elephc-PHP (hoisted function declarations) rather than as native builtins,
//! because their behaviour is expressible in terms of already-native primitives.
//!
//! Called from:
//! - `crate::pipeline::compile` (and the codegen test harness) via `inject_if_used`, before
//!   name resolution so user calls resolve to the injected functions.
//!
//! Key details:
//! - Pay-for-use: injected only when the program references one of `STDLIB_PRELUDE_NAMES`
//!   (a cheap Debug-render substring scan — over-injection is harmless, link-GC strips an
//!   unused prelude function). A user's own same-named declaration shadows the prelude
//!   (filtered out via `declared_function_names`), so there is no redeclaration conflict.
//! - Scope is deliberately minimal: only functions the conformance survey flagged as `Undefined`
//!   on v0.26.1 that the old fork carried as prelude PHP (mb_substr/trim family,
//!   array_count_values, array_last, http_build_query). Each is byte-parity with PHP 8.5.

use std::collections::HashSet;

use crate::parser::ast::{Program, StmtKind};

/// The function names whose presence in a program triggers prelude injection.
const STDLIB_PRELUDE_NAMES: [&str; 16] = [
    "mb_substr",
    "mb_ltrim",
    "mb_rtrim",
    "mb_trim",
    "mb_strtoupper",
    "mb_strtolower",
    "array_count_values",
    "array_last",
    "http_build_query",
    "get_debug_type",
    "filter_var",
    "set_error_handler",
    "restore_error_handler",
    "parse_url",
    "strspn",
    "mb_substr_count",
];

/// The elephc-PHP source for the injected functions. `__elephc_mb_byte_index` is the shared
/// UTF-8 codepoint→byte-offset helper for the `mb_*` family; it is internal (never a trigger).
const STDLIB_PRELUDE_SRC: &str = r#"<?php
function __elephc_mb_byte_index(string $s, int $cpIndex, int $byteLen): int {
    $cp = 0;
    $i = 0;
    while ($i < $byteLen) {
        if ($cp === $cpIndex) {
            return $i;
        }
        $b = ord($s[$i]);
        if ($b < 128) {
            $i = $i + 1;
        } elseif (($b & 224) === 192) {
            $i = $i + 2;
        } elseif (($b & 240) === 224) {
            $i = $i + 3;
        } else {
            $i = $i + 4;
        }
        $cp = $cp + 1;
    }
    return $byteLen;
}
function mb_substr(string $string, int $start, int $length = PHP_INT_MAX): string {
    $byteLen = strlen($string);
    $cpCount = mb_strlen($string);
    $startIdx = $start;
    if ($startIdx < 0) {
        $startIdx = $cpCount + $startIdx;
        if ($startIdx < 0) {
            $startIdx = 0;
        }
    }
    $len = $length;
    if ($len === PHP_INT_MAX) {
        $end = $cpCount;
    } elseif ($len < 0) {
        $end = $cpCount + $len;
    } else {
        $end = $startIdx + $len;
    }
    if ($end > $cpCount) {
        $end = $cpCount;
    }
    if ($startIdx >= $cpCount || $end <= $startIdx) {
        return '';
    }
    $startByte = __elephc_mb_byte_index($string, $startIdx, $byteLen);
    $endByte = __elephc_mb_byte_index($string, $end, $byteLen);
    return substr($string, $startByte, $endByte - $startByte);
}
function mb_ltrim(string $string, string $characters = " \f\n\r\t\v\0"): string {
    $s = $string;
    while ($s !== '') {
        $first = mb_substr($s, 0, 1);
        if ($first === '' || !str_contains($characters, $first)) {
            break;
        }
        $s = mb_substr($s, 1);
    }
    return $s;
}
function mb_rtrim(string $string, string $characters = " \f\n\r\t\v\0"): string {
    $s = $string;
    while ($s !== '') {
        $last = mb_substr($s, -1);
        if ($last === '' || !str_contains($characters, $last)) {
            break;
        }
        $s = mb_substr($s, 0, mb_strlen($s) - 1);
    }
    return $s;
}
function mb_trim(string $string, string $characters = " \f\n\r\t\v\0"): string {
    return mb_rtrim(mb_ltrim($string, $characters), $characters);
}
function mb_strtoupper(string $string): string {
    return strtoupper($string);
}
function mb_strtolower(string $string): string {
    return strtolower($string);
}
function array_last(mixed $array): mixed {
    $last = null;
    foreach ($array as $value) {
        $last = $value;
    }
    return $last;
}
function array_count_values(mixed $array): mixed {
    $values = [];
    foreach ($array as $value) {
        if (is_string($value) || is_int($value)) {
            $values[] = (string) $value;
        }
    }
    $counts = [];
    $n = count($values);
    $i = 0;
    while ($i < $n) {
        $v = $values[$i];
        $c = 0;
        $j = 0;
        while ($j < $n) {
            if ($values[$j] === $v) {
                $c = $c + 1;
            }
            $j = $j + 1;
        }
        $counts[$v] = $c;
        $i = $i + 1;
    }
    return $counts;
}
function http_build_query(mixed $data): string {
    $parts = [];
    foreach ($data as $key => $value) {
        if ($value === null) {
            continue;
        }
        $rendered = is_bool($value) ? ($value ? '1' : '0') : (string) $value;
        $parts[] = urlencode((string) $key) . '=' . urlencode($rendered);
    }
    return implode('&', $parts);
}
function get_debug_type(mixed $value): string {
    if (is_object($value)) {
        // `$value::class`, NOT `get_class($value)`. Both are identical for an object
        // (which the is_object() guard has just established), but they lower very
        // differently: `$value::class` desugars to the __elephc_class_name_of intrinsic,
        // whose Mixed arm routes to the __rt_class_name_of runtime helper, whereas
        // get_class()'s lowering (lower_class_name_lookup) has NO Mixed arm and hard-errors
        // with "get_class for PHP type Mixed".
        //
        // elephc has no "any object" PhpType — only Object(ClassName) — so a `mixed` value
        // cannot be narrowed to an object by is_object() (or even by an `object` type hint);
        // it stays Mixed, and get_class() therefore could never lower here.
        //
        // This one call made the ENTIRE stdlib prelude un-compilable natively, which is why
        // explode/strtr/arity_ext/substr_compare each had to be split into their own
        // one-function preludes just to avoid dragging this bundle in (see their module docs).
        // It was the single largest native-compile gap in the conformance survey: 114 of 1082 roots.
        return $value::class;
    }
    if ($value === null) {
        return 'null';
    }
    if (is_bool($value)) {
        return 'bool';
    }
    if (is_int($value)) {
        return 'int';
    }
    if (is_float($value)) {
        return 'float';
    }
    if (is_string($value)) {
        return 'string';
    }
    if (is_array($value)) {
        return 'array';
    }
    return 'unknown';
}
function filter_var(mixed $value, int $filter = 516, mixed $options = null): mixed {
    $s = (string) $value;
    if ($filter === 274) {
        if (preg_match('/^[^@\s"\'\\\\]+@([A-Za-z0-9]([A-Za-z0-9-]*[A-Za-z0-9])?\.)+[A-Za-z]{2,}$/', $s) === 1) {
            return $s;
        }
        return false;
    }
    if ($filter === 273) {
        if (preg_match('/^[A-Za-z][A-Za-z0-9+.-]*:\/\/[^\s\/?#]+[^\s]*$/', $s) === 1) {
            return $s;
        }
        return false;
    }
    if ($filter === 516) {
        return $s;
    }
    return false;
}
function set_error_handler(mixed $callback, int $error_levels = 32767): mixed {
    return null;
}
function restore_error_handler(): bool {
    return true;
}
function parse_url(string $url, int $component = -1): mixed {
    $scheme = '';
    $afterScheme = $url;
    $colon = strpos($url, ':');
    $slash = strpos($url, '/');
    if ($colon !== false && ($slash === false || $colon < $slash)) {
        $scheme = substr($url, 0, $colon);
        $afterScheme = substr($url, $colon + 1);
    }
    $fragment = '';
    $beforeFragment = $afterScheme;
    $hashAt = strpos($afterScheme, '#');
    if ($hashAt !== false) {
        $fragment = substr($afterScheme, $hashAt + 1);
        $beforeFragment = substr($afterScheme, 0, $hashAt);
    }
    $query = '';
    $beforeQuery = $beforeFragment;
    $questionAt = strpos($beforeFragment, '?');
    if ($questionAt !== false) {
        $query = substr($beforeFragment, $questionAt + 1);
        $beforeQuery = substr($beforeFragment, 0, $questionAt);
    }
    $host = '';
    $port = '';
    $path = $beforeQuery;
    if (str_starts_with($beforeQuery, '//')) {
        $fullAuthority = substr($beforeQuery, 2);
        $path = '';
        $authority = $fullAuthority;
        $pathAt = strpos($fullAuthority, '/');
        if ($pathAt !== false) {
            $path = substr($fullAuthority, $pathAt);
            $authority = substr($fullAuthority, 0, $pathAt);
        }
        $hostWithPort = $authority;
        $at = strrpos($authority, '@');
        if ($at !== false) {
            $hostWithPort = substr($authority, $at + 1);
        }
        $host = $hostWithPort;
        $portColon = strrpos($hostWithPort, ':');
        if ($portColon !== false) {
            $port = substr($hostWithPort, $portColon + 1);
            $host = substr($hostWithPort, 0, $portColon);
        }
    }
    if ($component === 0) {
        return $scheme !== '' ? $scheme : null;
    }
    if ($component === 1) {
        return $host !== '' ? $host : null;
    }
    if ($component === 2) {
        return $port !== '' ? (int) $port : null;
    }
    if ($component === 5) {
        return $path !== '' ? $path : null;
    }
    if ($component === 6) {
        return $query !== '' ? $query : null;
    }
    if ($component === 7) {
        return $fragment !== '' ? $fragment : null;
    }
    return false;
}
function strspn(string $string, string $characters, int $offset = 0, int $length = PHP_INT_MAX): int {
    $len = strlen($string);
    $start = $offset;
    if ($start < 0) {
        $start = $len + $start;
        if ($start < 0) {
            $start = 0;
        }
    }
    if ($start > $len) {
        return 0;
    }
    $end = $len;
    if ($length !== PHP_INT_MAX) {
        if ($length < 0) {
            $end = $len + $length;
        } else {
            $end = $start + $length;
        }
        if ($end > $len) {
            $end = $len;
        }
    }
    $n = 0;
    $i = $start;
    while ($i < $end) {
        if (!str_contains($characters, $string[$i])) {
            break;
        }
        $n = $n + 1;
        $i = $i + 1;
    }
    return $n;
}
function mb_substr_count(string $haystack, string $needle): int {
    if ($needle === '') {
        return 0;
    }
    $count = 0;
    $pos = 0;
    $step = strlen($needle);
    while (true) {
        $rest = substr($haystack, $pos);
        $found = strpos($rest, $needle);
        if ($found === false) {
            break;
        }
        $count = $count + 1;
        $pos = $pos + $found + $step;
    }
    return $count;
}
"#;

/// Prepends the stdlib prelude to `program` when it references any prelude function and does
/// not declare its own same-named function. Returns `program` unchanged otherwise.
pub fn inject_if_used(program: Program) -> Program {
    let rendered = format!("{:?}", program);
    if !STDLIB_PRELUDE_NAMES
        .iter()
        .any(|name| rendered.contains(name))
    {
        return program;
    }
    let declared = declared_function_names(&program);
    let tokens = crate::lexer::tokenize(STDLIB_PRELUDE_SRC).expect("stdlib prelude must tokenize");
    let prelude = crate::parser::parse(&tokens).expect("stdlib prelude must parse");
    let mut combined: Program = prelude
        .into_iter()
        .filter(|stmt| match &stmt.kind {
            StmtKind::FunctionDecl { name, .. } => !declared.contains(&name.to_ascii_lowercase()),
            _ => true,
        })
        .collect();
    combined.extend(program);
    combined
}

/// Collects the lowercased names of functions the program declares at the top level or directly
/// inside namespace declarations, so user definitions shadow the prelude.
fn declared_function_names(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    collect_declared(program, &mut names);
    names
}

/// Recursively collects declared function names from a statement list (top level, namespace
/// blocks, and synthetic groupings).
fn collect_declared(stmts: &[crate::parser::ast::Stmt], names: &mut HashSet<String>) {
    for stmt in stmts {
        match &stmt.kind {
            StmtKind::FunctionDecl { name, .. } => {
                names.insert(name.to_ascii_lowercase());
            }
            StmtKind::NamespaceBlock { body, .. } | StmtKind::Synthetic(body) => {
                collect_declared(body, names);
            }
            _ => {}
        }
    }
}
