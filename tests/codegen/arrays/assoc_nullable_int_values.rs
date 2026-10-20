//! Purpose:
//! Integration tests for associative arrays whose values are `?int`.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - A hash has no `?int` payload, so such a hash stores Mixed cells; literals, writes, reads,
//!   null checks and iteration must all keep PHP's int-or-null values.

use crate::support::*;

/// A string-keyed literal holding a `?int` keeps both the integer and the null.
#[test]
fn test_assoc_literal_with_nullable_int_value() {
    let out = compile_and_run(r#"<?php
function pack_port(?int $port): array { return ['host' => 'h', 'port' => $port]; }
var_dump(pack_port(8080), pack_port(null));
"#);
    assert_eq!(
        out,
        "array(2) {\n  [\"host\"]=>\n  string(1) \"h\"\n  [\"port\"]=>\n  int(8080)\n}\narray(2) {\n  [\"host\"]=>\n  string(1) \"h\"\n  [\"port\"]=>\n  NULL\n}\n"
    );
}

/// Writing a `?int` into a hash element and reading it back keeps the value and its nullness.
#[test]
fn test_assoc_element_write_and_read_of_nullable_int() {
    let out = compile_and_run_with_heap_debug(r#"<?php
function store(?int $port): array { $a = []; $a['port'] = $port; return $a; }
function port(array $a): ?int { return $a['port']; }
$set = store(7); $unset = store(null);
echo port($set) + 1, ':', port($unset) === null ? 'null' : 'bad', ':', $unset['port'] ?? 'default';
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "8:null:default");
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Iterating a hash of `?int` values yields each integer and null in order.
#[test]
fn test_assoc_foreach_over_nullable_int_values() {
    let out = compile_and_run(r#"<?php
function ports(?int $first, ?int $second): array { return ['a' => $first, 'b' => $second]; }
foreach (ports(1, null) as $name => $port) { echo $name, '=', $port === null ? 'null' : $port, ';'; }
"#);
    assert_eq!(out, "a=1;b=null;");
}

/// `clone` with a `?int` property override (AIC #17) sets both an integer and null.
#[test]
fn test_clone_with_nullable_int_property() {
    let out = compile_and_run(r#"<?php
final readonly class CloneNullInt {
    public function __construct(private ?int $port = null) {}
    public function port(): ?int { return $this->port; }
    public function withPort(?int $port): self { return clone($this, ['port' => $port]); }
}
$a = new CloneNullInt(); $b = $a->withPort(8080); $c = $b->withPort(null);
var_dump($a->port(), $b->port(), $c->port());
"#);
    assert_eq!(out, "NULL\nint(8080)\nNULL\n");
}

/// A `?int` return of a hash element read with `?? null` keeps the integer and the null.
#[test]
fn test_nullable_int_return_of_hash_element_or_null() {
    let out = compile_and_run(r#"<?php
function size(array $stats): ?int { return $stats['size'] ?? null; }
var_dump(size(['size' => 12]));
var_dump(size([]));
"#);
    assert_eq!(out, "int(12)\nNULL\n");
}
