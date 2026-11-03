//! Purpose:
//! Verifies instance calls through a base class or interface whose overrides widen a parameter
//! (`int` to `int|string`, `string` to `?string`), which PHP allows by contravariance.
//!
//! Called from:
//! - The codegen integration suite through `oop`.
//!
//! Key details:
//! - The widened body reads the argument in other storage than the declared signature, so the
//!   call selects it by the receiver's runtime class and binds the argument by its signature.
//! - Arguments are checked by the body that runs, as php-src does, and its class is named.
//! - A by-reference parameter bound in other storage is refused at compile time.

use super::*;

/// An `int` parameter widened to `int|string` receives the caller's value through a base-class
/// receiver, a `mixed` string and int and a literal alike, while the base body keeps its own binding.
#[test]
fn test_widened_override_param_through_base_receiver() {
    let out = compile_and_run_with_heap_debug(r#"<?php
class Wide { function m(int $i): string { return 'wide:' . $i; } }
class Wider extends Wide { function m(int|string $i): string { return 'wider:' . $i; } }
function viaBase(Wide $w, mixed $v): string { return $w->m($v); }
function viaLiteral(Wide $w): string { return $w->m(7); }
echo viaBase(new Wider(), json_decode('"x"')), "\n";
echo viaBase(new Wider(), json_decode('7')), "\n";
echo viaLiteral(new Wider()), "\n";
echo viaBase(new Wide(), json_decode('8')), "\n";
echo viaLiteral(new Wide()), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "wider:x\nwider:7\nwider:7\nwide:8\nwide:7\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// An `int` parameter widened to `mixed` receives every value boxed as it was given.
#[test]
fn test_override_widened_to_mixed_through_base_receiver() {
    let out = compile_and_run_with_heap_debug(r#"<?php
class Narrow { function m(int $i): string { return 'narrow:' . $i; } }
class Open extends Narrow { function m(mixed $i): string { return 'open:' . gettype($i) . ':' . json_encode($i); } }
function viaBase(Narrow $n, mixed $v): string { return $n->m($v); }
echo viaBase(new Open(), 7), "\n";
echo viaBase(new Open(), 2.5), "\n";
echo viaBase(new Open(), json_decode('[1,"b"]')), "\n";
echo viaBase(new Open(), null), "\n";
echo viaBase(new Narrow(), 9), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "open:integer:7\nopen:double:2.5\nopen:array:[1,\"b\"]\nopen:NULL:null\nnarrow:9\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A `string` parameter widened to `?string` receives `null` as `null`, not as an empty string.
#[test]
fn test_override_widened_to_nullable_string_through_base_receiver() {
    let out = compile_and_run(r#"<?php
class Label { function show(string $s): string { return 'label:' . $s; } }
class OptionalLabel extends Label { function show(?string $s): string { return 'optional:' . ($s ?? 'none'); } }
function viaBase(Label $l, mixed $v): string { return $l->show($v); }
echo viaBase(new OptionalLabel(), 'hi'), "\n";
echo viaBase(new OptionalLabel(), null), "\n";
echo viaBase(new OptionalLabel(), str_repeat('ab', 2)), "\n";
echo viaBase(new Label(), 'yo'), "\n";
"#);
    assert_eq!(
        out,
        "optional:hi\noptional:none\noptional:abab\nlabel:yo\n"
    );
}

/// An interface method implemented with a wider parameter accepts the value, and the other
/// implementations still refuse it naming their own class.
#[test]
fn test_interface_method_implemented_with_wider_param() {
    let out = compile_and_run(r#"<?php
interface Formatter { function format(int $n): string; }
class Loose implements Formatter { function format(int|string $n): string { return 'loose:' . $n; } }
class Tight implements Formatter { function format(int $n): string { return 'tight:' . $n; } }
class Other implements Formatter { function format(int $n): string { return 'other:' . $n; } }
function viaInterface(Formatter $f, mixed $v): string { return $f->format($v); }
foreach ([new Loose(), new Tight(), new Other()] as $f) {
    echo viaInterface($f, 1), "\n";
    try { echo viaInterface($f, 'two'), "\n"; } catch (TypeError $e) { echo $e->getMessage(), "\n"; }
}
"#);
    assert_eq!(
        out,
        "loose:1\nloose:two\ntight:1\nTight::format(): Argument #1 ($n) must be of type int, string given\nother:1\nOther::format(): Argument #1 ($n) must be of type int, string given\n"
    );
}

/// The argument is checked and bound by the parameters of the body that runs, as php-src does: a
/// value neither implementation accepts throws `TypeError` naming the implementation's class.
#[test]
fn test_widened_override_checks_by_the_body_that_runs() {
    let out = compile_and_run_with_heap_debug(r#"<?php
class Base { function m(int $i): string { return 'base:' . $i; } }
class Wider extends Base { function m(int|string $i): string { return 'wider:' . $i; } }
class Numeric extends Base { function m(int|float $i): string { return 'numeric:' . var_export($i, true); } }
function viaBase(Base $b, mixed $v): string {
    try { return $b->m($v); } catch (TypeError $e) { return $e->getMessage(); }
}
foreach ([new Base(), new Wider(), new Numeric()] as $b) {
    echo viaBase($b, json_decode('[1]')), "\n";
    echo viaBase($b, 'abc'), "\n";
    echo viaBase($b, '7'), "\n";
    echo viaBase($b, 1.5), "\n";
}
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "Base::m(): Argument #1 ($i) must be of type int, array given\nBase::m(): Argument #1 ($i) must be of type int, string given\nbase:7\nbase:1\nWider::m(): Argument #1 ($i) must be of type string|int, array given\nwider:abc\nwider:7\nwider:1\nNumeric::m(): Argument #1 ($i) must be of type int|float, array given\nNumeric::m(): Argument #1 ($i) must be of type int|float, string given\nnumeric:7\nnumeric:1.5\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// `$this`, nullable and inherited-override receivers dispatch to the widened body with an
/// abstract base and several overrides per hierarchy.
#[test]
fn test_widened_override_through_this_nullable_and_inherited_receivers() {
    let out = compile_and_run_with_heap_debug(r#"<?php
abstract class Shape {
    abstract function area(int $scale, string $unit): string;
    function twice(mixed $scale): string { return $this->area($scale, 'cm') . '|' . $this->area(2, 'mm'); }
}
class Square extends Shape { function area(int|float $scale, string $unit): string { return 'square:' . var_export($scale, true) . $unit; } }
class BigSquare extends Square {}
class Circle extends Shape { function area(mixed $scale, ?string $unit): string { return 'circle:' . gettype($scale) . ($unit ?? 'none'); } }
class Plain extends Shape { function area(int $scale, string $unit): string { return 'plain:' . $scale . $unit; } }
function viaNullable(?Shape $s, mixed $scale): string { return $s?->area($scale, 'x') ?? 'null'; }
foreach ([new Square(), new BigSquare(), new Circle(), new Plain()] as $shape) {
    echo $shape->twice(4), "\n";
    echo viaNullable($shape, 5), "\n";
}
echo viaNullable(null, 1), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "square:4cm|square:2mm\nsquare:5x\nsquare:4cm|square:2mm\nsquare:5x\ncircle:integercm|circle:integermm\ncircle:integerx\nplain:4cm|plain:2mm\nplain:5x\nnull\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A call in a loop alternates between the widened and the declared body.
#[test]
fn test_widened_override_in_a_loop() {
    let out = compile_and_run(r#"<?php
class Label { function show(string $s): string { return 'label:' . $s; } }
class OptionalLabel extends Label { function show(?string $s): string { return 'optional:' . ($s ?? 'none'); } }
function viaBase(Label $l, string $s): string { return $l->show($s); }
for ($i = 0; $i < 3; $i++) {
    echo viaBase($i % 2 ? new Label() : new OptionalLabel(), str_repeat('z', $i + 1)), "\n";
}
"#);
    assert_eq!(
        out,
        "optional:z\nlabel:zz\noptional:zzz\n"
    );
}

/// A strict file checks each body's own declaration: the widened one accepts a string, neither
/// accepts a float for an `int`.
#[test]
fn test_widened_override_strict_types() {
    let out = compile_and_run_with_heap_debug(r#"<?php
declare(strict_types=1);
class Base { function m(int $i): string { return 'base:' . $i; } }
class Wider extends Base { function m(int|string $i): string { return 'wider:' . $i; } }
function viaBase(Base $b, mixed $v): string {
    try { return $b->m($v); } catch (TypeError $e) { return $e->getMessage(); }
}
foreach ([new Base(), new Wider()] as $b) {
    echo viaBase($b, 'x'), "\n";
    echo viaBase($b, 3), "\n";
    echo viaBase($b, 1.0), "\n";
}
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(
        out.stdout,
        "Base::m(): Argument #1 ($i) must be of type int, string given\nbase:3\nBase::m(): Argument #1 ($i) must be of type int, float given\nwider:x\nwider:3\nWider::m(): Argument #1 ($i) must be of type string|int, float given\n",
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// The base-receiver call keeps its meaning without EIR optimization.
#[test]
fn test_widened_override_param_through_base_receiver_no_ir_opt() {
    without_ir_opt(test_widened_override_param_through_base_receiver);
}

/// Argument checks by the running body keep their meaning without EIR optimization.
#[test]
fn test_widened_override_checks_by_the_body_that_runs_no_ir_opt() {
    without_ir_opt(test_widened_override_checks_by_the_body_that_runs);
}

/// A by-reference parameter an override binds in other storage is refused with its position and
/// both classes, instead of handing the override a reference cell of the wrong type.
#[test]
fn test_widened_override_by_reference_param_is_refused() {
    let error = compile_source_expect_backend_error(r#"<?php
class Counter { function bump(int &$i): string { $i++; return 'counter'; } }
class Appender extends Counter { function bump(int|string &$i): string { $i = $i . '!'; return 'appender'; } }
function viaBase(Counter $c): string { $v = 5; $r = $c->bump($v); return $r . ':' . $v; }
echo viaBase(new Appender());
"#);
    assert!(
        error.contains("by-reference parameter #1 of Appender::bump is bound in other storage than Counter::bump declares it"),
        "{error}"
    );
}

/// A hierarchy whose overrides agree on storage keeps its single vtable call; a widened one
/// adds the runtime-class test, on every supported target.
#[test]
fn test_widened_override_dispatch_only_where_storage_differs() {
    let dir = make_cli_test_dir("elephc_widened_override_dispatch");
    let source = dir.join("main.php");
    fs::write(&source, r#"<?php
class Same { public function m(int $i): string { return 'same'; } }
class SameChild extends Same { public function m(int $i): string { return 'child'; } }
class Wide { public function m(int $i): string { return 'wide'; } }
class Wider extends Wide { public function m(int|string $i): string { return 'wider'; } }
function read_same(Same $object, int $value): string { return $object->m($value); }
function read_wide(Wide $object, int $value): string { return $object->m($value); }
echo read_same(new SameChild(), $argc), read_wide(new Wider(), $argc);
"#).unwrap();
    for target in ["macos-aarch64", "ios-arm64", "ios-sim-arm64", "linux-aarch64", "linux-x86_64"] {
        let output = elephc_cli_command(&dir)
            .args(["--ir-opt=off", "--emit-asm", "--emit", "staticlib", "--target", target])
            .arg(&source).output().unwrap();
        assert!(output.status.success(), "{target}: {}", String::from_utf8_lossy(&output.stderr));
        let assembly = fs::read_to_string(dir.join("main.s")).unwrap();
        let body = |name: &str| {
            let start = format!("{}:\n", elephc::names::function_symbol(name));
            let end = format!("{}:", elephc::names::function_epilogue_symbol(name));
            assembly.split_once(&start).unwrap_or_else(|| panic!("missing {start}"))
                .1.split_once(&end).unwrap_or_else(|| panic!("missing {end}")).0.to_string()
        };
        let same = body("read_same");
        assert!(!same.contains("diverging_override"), "{target}: {same}");
        assert!(same.contains("vtable_ptrs"), "{target}: {same}");
        let wide = body("read_wide");
        assert!(wide.contains("diverging_override_Wider"), "{target}: {wide}");
        assert!(wide.contains("vtable_ptrs"), "{target}: {wide}");
    }
    let _ = fs::remove_dir_all(&dir);
}
