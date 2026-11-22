//! Purpose:
//! Verifies override variance through a base receiver beyond widened parameters: narrowed and
//! untyped returns, `static::` calls, `mixed` receivers, and the parameter types PHP refuses.
//!
//! Called from:
//! - The codegen integration suite through `oop`.
//!
//! Key details:
//! - An override that returns in other storage than the receiver's signature is called by the
//!   runtime class and its result boxed, so the call is typed `mixed`.
//! - A `static::` call selects such an override by the late-bound called class.
//! - A `mixed` receiver checks each argument by the declaration of the body that runs.
//! - A declared scalar parameter must accept the parent's, as PHP's compile-time rule says.
//! - Every expected output was produced by php 8.5.

use super::*;

/// A `?int`, `?string` or `int|string` return narrowed by an override, called through the base
/// class or interface, hands back the value the running body returned.
#[test]
fn test_narrower_override_return_through_base_receiver() {
    let out = compile_and_run_with_heap_debug(r#"<?php
abstract class Source { abstract function read(): ?int; }
class Fixed extends Source { function read(): int { return 7; } }
class Missing extends Source { function read(): ?int { return null; } }
function viaSource(Source $s): string { $r = $s->read(); return var_export($r, true) . ':' . (($r ?? -1) + 1); }
echo viaSource(new Fixed()), " ", viaSource(new Missing()), "\n";
class Label { function text(): ?string { return null; } }
class Named extends Label { function text(): string { return str_repeat('n', 2); } }
function viaLabel(Label $l): string { return var_export($l->text(), true); }
echo viaLabel(new Label()), " ", viaLabel(new Named()), "\n";
class Wide { function key(): int|string { return 'w'; } }
class Narrow extends Wide { function key(): int { return 2; } }
function viaWide(Wide $w): string { return var_export($w->key(), true); }
echo viaWide(new Wide()), " ", viaWide(new Narrow()), "\n";
interface Counter { function count(): ?int; }
class Ten implements Counter { function count(): int { return 10; } }
class Unknown implements Counter { function count(): ?int { return null; } }
function viaCounter(Counter $c): string { return var_export($c->count(), true); }
echo viaCounter(new Ten()), " ", viaCounter(new Unknown()), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "7:8 NULL:0\nNULL 'nn'\n'w' 2\n10 NULL\n", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// Untyped overrides that return other types than the base body, an abstract placeholder, a
/// body without a return and a declared `array` with other element storage keep each value.
#[test]
fn test_untyped_override_returns_through_base_receiver() {
    let out = compile_and_run_with_heap_debug(r#"<?php
abstract class Shape { abstract public function tag($v); }
class Sq extends Shape { public function tag($v) { return "Sq " . $v; } }
class Ci extends Shape { public function tag($v) { return $v; } }
function go(Shape $s) { echo json_encode($s->tag(1)), " ", json_encode($s->tag("s")), "\n"; }
go(new Sq()); go(new Ci());
class A { public function f() { return 1; } public function twice() { return [$this->f(), $this->f()]; } }
class B extends A { public function f() { return "s"; } }
class C extends A { public function f() { return null; } }
function viaA(A $a) { echo json_encode($a->f()), " ", json_encode($a->twice()), "\n"; }
viaA(new A()); viaA(new B()); viaA(new C());
abstract class Op { abstract public function f($v); public function run($v) { return $this->f($v); } }
class Inc extends Op { public function f($v) { return $v + 1; } }
class Str extends Op { public function f($v) { return "s$v"; } }
foreach ([new Inc(), new Str()] as $op) { echo json_encode($op->f(1)), " ", json_encode($op->run(2)), "\n"; }
class Quiet { function f() { } }
class Loud extends Quiet { function f() { return 3; } }
function viaQuiet(Quiet $q) { return var_export($q->f(), true); }
echo viaQuiet(new Quiet()), " ", viaQuiet(new Loud()), "\n";
class Ints { function items(): array { return [1, 2]; } }
class Strs extends Ints { function items(): array { return ["a", "b"]; } }
function viaInts(Ints $i) { return json_encode($i->items()); }
echo viaInts(new Ints()), " ", viaInts(new Strs()), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "\"Sq 1\" \"Sq s\"\n1 \"s\"\n1 [1,1]\n\"s\" [\"s\",\"s\"]\nnull [null,null]\n2 3\n\"s1\" \"s2\"\nNULL 3\n[1,2] [\"a\",\"b\"]\n", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A `mixed` abstract return narrowed to scalars and arrays, and a `?Node` return narrowed to
/// `Node`, through the abstract class and through an interface.
#[test]
fn test_narrowed_mixed_and_nullable_object_returns() {
    let out = compile_and_run_with_heap_debug(r#"<?php
abstract class Any { abstract public function f(): mixed; }
class AnyString extends Any { public function f(): string { return "b"; } }
class AnyInt extends Any { public function f(): int { return 7; } }
class AnyArray extends Any { public function f(): array { return [1]; } }
function viaAny(Any $a) { return json_encode($a->f()); }
echo viaAny(new AnyString()), viaAny(new AnyInt()), viaAny(new AnyArray()), "\n";
class Node { public $n = "N"; }
interface Finds { public function g(): Node; }
abstract class Lookup { abstract public function g(): ?Node; }
class Found extends Lookup implements Finds { public function g(): Node { return new Node(); } }
class Lost extends Lookup { public function g(): ?Node { return null; } }
function viaFinds(Finds $f) { return $f->g()->n; }
function viaLookup(Lookup $l) { $r = $l->g(); return $r === null ? "null" : $r->n; }
echo viaFinds(new Found()), viaLookup(new Found()), viaLookup(new Lost()), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "\"b\"7[1]\nNNnull\n", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A covariant object return keeps chaining by class, and a private method stays bound
/// lexically even when a subclass declares its own with another return.
#[test]
fn test_agreeing_override_returns_keep_their_binding() {
    let out = compile_and_run(r#"<?php
class Fluent { function self(): Fluent { return $this; } function name(): string { return 'f'; } }
class Fluent2 extends Fluent { function self(): Fluent2 { return $this; } function name(): string { return 'f2'; } }
function chain(Fluent $f) { return $f->self()->name(); }
echo chain(new Fluent()), chain(new Fluent2()), "\n";
class Base { private function p() { return 1; } function run() { return $this->p(); } }
class Kid extends Base { private function p() { return "kid"; } }
var_dump((new Kid())->run());
"#);
    assert_eq!(out, "ff2\nint(1)\n");
}

/// A `static::` call binds a widened parameter and boxes a narrowed or untyped return by the
/// body the called class runs, inherited bodies included.
#[test]
fn test_late_static_override_storage() {
    let out = compile_and_run_with_heap_debug(r#"<?php
class P { static function m(int $i): string { return 'p:' . $i; } static function go(mixed $v): string { return static::m($v); } }
class Q extends P { static function m(int|string $i): string { return 'q:' . var_export($i, true); } }
class R extends Q {}
echo P::go(5), " ", Q::go(5), " ", Q::go("x"), " ", R::go("r"), "\n";
class Maker { static function make() { return 1; } static function show(): string { return var_export(static::make(), true); } }
class StrMaker extends Maker { static function make() { return "q"; } }
echo Maker::show(), " ", StrMaker::show(), "\n";
abstract class Opt { static function m(?int $v): ?int { return $v; } static function go(mixed $v) { return var_export(static::m($v), true); } }
class Always extends Opt { static function m(int|string|null $v): int { return 42; } }
echo Opt::go(null), " ", Always::go("x"), " ", Opt::go(3), "\n";
"#);
    assert!(out.success, "{}", out.stderr);
    assert_eq!(out.stdout, "p:5 q:5 q:'x' q:'r'\n1 'q'\nNULL 42 3\n", "{}", out.stderr);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}

/// A call on a `mixed` or `object` receiver checks each argument against the declaration of the
/// body that runs and names that class in the `TypeError`.
///
/// No heap check: a `mixed` receiver whose candidates bind a string parameter and another
/// storage leaks on the base as well, an ownership gap of `mixed` dispatch itself.
#[test]
fn test_mixed_receiver_checks_arguments_by_the_running_body() {
    let out = compile_and_run(r#"<?php
class A { function m(int $i): string { return 'a:' . $i; } }
class B { function m(string $s): int { return strlen($s) * 10; } }
class C extends A { function m(int|string $i): string { return 'c:' . var_export($i, true); } }
function message(TypeError $e): string { return substr($e->getMessage(), 0, strpos($e->getMessage(), ' given') + 6); }
function go(mixed $o, mixed $v) { try { echo $o->m($v), "\n"; } catch (TypeError $e) { echo message($e), "\n"; } }
go(new A(), 5); go(new A(), "7"); go(new A(), "abc"); go(new B(), "x"); go(new B(), [1]); go(new C(), "abc"); go(new A(), null);
function viaObject(object $o, mixed $v) { try { echo $o->m($v), "\n"; } catch (TypeError $e) { echo message($e), "\n"; } }
viaObject(new A(), "abc"); viaObject(new C(), "abc");
"#);
    assert_eq!(out, "a:5\na:7\nA::m(): Argument #1 ($i) must be of type int, string given\n10\nB::m(): Argument #1 ($s) must be of type string, array given\nc:'abc'\nA::m(): Argument #1 ($i) must be of type int, null given\nA::m(): Argument #1 ($i) must be of type int, string given\nc:'abc'\n");
}

/// A `mixed` receiver whose method has one implementation checks and coerces its argument.
#[test]
fn test_mixed_receiver_single_implementation_checks_arguments() {
    let out = compile_and_run(r#"<?php
class Only { function m(int $i): string { return 'only:' . $i; } }
function go(mixed $o, mixed $v) { try { echo $o->m($v), "\n"; } catch (TypeError $e) { echo substr($e->getMessage(), 0, strpos($e->getMessage(), ' given') + 6), "\n"; } }
go(new Only(), 4); go(new Only(), "abc"); go(new Only(), "12");
"#);
    assert_eq!(out, "only:4\nOnly::m(): Argument #1 ($i) must be of type int, string given\nonly:12\n");
}

/// Parameter widenings PHP accepts still compile: a union, `null`, `mixed`, and `float|int` for
/// a `float`.
#[test]
fn test_accepted_parameter_widenings_still_compile() {
    let out = compile_and_run(r#"<?php
class A { function m(int $i) { return "a$i"; } function n(?int $i) { return 1; } function o(string $s) { return 1; } function p(float $f) { return 1; } function q(int $i) { return 1; } function r(bool $b) { return 1; } }
class B extends A { function m(int|string $i) { return "b$i"; } function n(int|null|string $i) { return 2; } function o(?string $s) { return 2; } function p(float|int $f) { return 2; } function q(mixed $i) { return 2; } function r(bool|int $b) { return 2; } }
interface I { function m(int $i); }
class C implements I { function m(int|float $i) { return 1; } }
echo (new B())->m(1), "\n";
"#);
    assert_eq!(out, "b1\n");
}

/// Narrowed returns keep their values without EIR optimization.
#[test]
fn test_narrower_override_return_through_base_receiver_no_ir_opt() {
    without_ir_opt(test_narrower_override_return_through_base_receiver);
}

/// Untyped override returns keep their values without EIR optimization.
#[test]
fn test_untyped_override_returns_through_base_receiver_no_ir_opt() {
    without_ir_opt(test_untyped_override_returns_through_base_receiver);
}

/// `static::` overrides keep their storage without EIR optimization.
#[test]
fn test_late_static_override_storage_no_ir_opt() {
    without_ir_opt(test_late_static_override_storage);
}

/// `mixed` receivers keep their argument checks without EIR optimization.
#[test]
fn test_mixed_receiver_checks_arguments_by_the_running_body_no_ir_opt() {
    without_ir_opt(test_mixed_receiver_checks_arguments_by_the_running_body);
}

/// A declared scalar parameter that does not accept the parent's is refused, as php refuses
/// it with "Declaration of ... must be compatible with ...": `float` does not accept `int`.
#[test]
fn test_incompatible_scalar_parameter_override_is_refused() {
    let cases = [
        ("class A { function m(int $i) {} } class B extends A { function m(float $i) {} }",
            "Cannot override method B::m with incompatible parameter $i of type float (parent accepts int)"),
        ("class A { function m(int $i) {} } class B extends A { function m(string $i) {} }",
            "Cannot override method B::m with incompatible parameter $i of type string (parent accepts int)"),
        ("class A { function m(float $i) {} } class B extends A { function m(int $i) {} }",
            "Cannot override method B::m with incompatible parameter $i of type int (parent accepts float)"),
        ("class A { function m(?int $i) {} } class B extends A { function m(int $i) {} }",
            "Cannot override method B::m with incompatible parameter $i of type int"),
        ("class A { function m(bool $i) {} } class B extends A { function m(int $i) {} }",
            "Cannot override method B::m with incompatible parameter $i of type int (parent accepts bool)"),
        ("class A { static function m(int $i) {} } class B extends A { static function m(float $i) {} }",
            "Cannot override static method B::m with incompatible parameter $i of type float (parent accepts int)"),
        ("interface I { function m(int $i); } class B implements I { function m(float $i) {} }",
            "Cannot implement method B::m with incompatible parameter $i of type float (interface accepts int)"),
        ("interface I { static function m(int $i); } class B implements I { static function m(float $i) {} }",
            "Cannot implement static method B::m with incompatible parameter $i of type float (interface accepts int)"),
    ];
    for (declarations, expected) in cases {
        let error = compile_expect_type_error(&format!("<?php {declarations} echo 'ok';"));
        assert!(error.contains(expected), "{declarations}: {error}");
    }
}
