//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of object-oriented PHP interfaces, including interface contract can be satisfied by concrete class, abstract base can defer method to concrete child, and class can implement multiple interfaces.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Uses checked-in example PHP fixtures through include_str! in addition to inline native-output assertions.

use super::*;

/// Verifies a concrete class can satisfy an interface contract by implementing all required methods.
/// Fixture: interface `Named` with method `name()`, concrete `User` implementing `Named`.
/// Asserts the method call on the concrete instance returns the expected string.
#[test]
fn test_interface_contract_can_be_satisfied_by_concrete_class() {
    let out = compile_and_run(
        r#"<?php
interface Named {
    public function name();
}

class User implements Named {
    public function name() {
        return "Ada";
    }
}

$user = new User();
echo $user->name();
"#,
    );
    assert_eq!(out, "Ada");
}

/// Verifies an abstract class can defer interface method implementation to a concrete child class.
/// Fixture: abstract `BaseGreeter` with abstract method `label()` and concrete `PersonGreeter`.
/// Asserts calling `greet()` on the concrete child triggers `label()` via `$this->label()`.
#[test]
fn test_abstract_base_can_defer_method_to_concrete_child() {
    let out = compile_and_run(
        r#"<?php
abstract class BaseGreeter {
    abstract public function label();

    public function greet() {
        return "hi " . $this->label();
    }
}

class PersonGreeter extends BaseGreeter {
    public function label() {
        return "world";
    }
}

$g = new PersonGreeter();
echo $g->greet();
"#,
    );
    assert_eq!(out, "hi world");
}

/// Verifies a class can implement multiple interfaces simultaneously.
/// Fixture: `Named` and `Tagged` interfaces, `Item` implementing both.
/// Asserts chained method calls resolve to the correct interface method on the same instance.
#[test]
fn test_class_can_implement_multiple_interfaces() {
    let out = compile_and_run(
        r#"<?php
interface Named {
    public function name();
}

interface Tagged {
    public function tag();
}

class Item implements Named, Tagged {
    public function name() {
        return "box";
    }

    public function tag() {
        return "BX";
    }
}

$item = new Item();
echo $item->name() . ":" . $item->tag();
"#,
    );
    assert_eq!(out, "box:BX");
}

/// Verifies transitive interface extension is enforced: a class must satisfy the full chain.
/// Fixture: `Labeled extends Named`, `Product implements Labeled`. Uses `strtoupper($this->name())`.
/// Asserts the method call correctly resolves through the transitive interface hierarchy.
#[test]
fn test_transitive_interface_extends_is_enforced() {
    let out = compile_and_run(
        r#"<?php
interface Named {
    public function name();
}

interface Labeled extends Named {
    public function label();
}

class Product implements Labeled {
    public function name() {
        return "widget";
    }

    public function label() {
        return strtoupper($this->name());
    }
}

$product = new Product();
echo $product->label();
"#,
    );
    assert_eq!(out, "WIDGET");
}

/// Verifies the checked-in example at `examples/interfaces/main.php` compiles and runs end-to-end.
/// Loads the PHP fixture via `include_str!`, asserts stdout matches expected multi-line output.
#[test]
fn test_example_interfaces_compiles_and_runs() {
    let out = compile_and_run(include_str!("../../../examples/interfaces/main.php"));
    // `isset(...) . "\n"`: a bool false stringifies to "" (not "0") in PHP, so the
    // post-unset isset line is empty.
    assert_eq!(out, "WIDGET\nproduct\nA-42\n1\n\n");
}

/// Verifies an interface with a read-only property (`get;`) can be satisfied by a concrete property.
/// Fixture: interface `HasId` with `public int $id { get; }`, concrete `User` with int field.
/// Asserts reading the property on the concrete instance returns the expected value.
#[test]
fn test_interface_get_property_contract_is_satisfied_by_concrete_property() {
    let out = compile_and_run(
        r#"<?php
interface HasId {
    public int $id { get; }
}

class User implements HasId {
    public int $id = 42;
}

$user = new User();
echo $user->id;
"#,
    );
    assert_eq!(out, "42");
}

/// Verifies interface property setters allow contravariant type (subclass) in implementing class.
/// Fixture: `Dog extends Animal`, interface `DogSink` with `public Dog $pet { set; }`,
/// implementing `Kennel` declares `public Animal $pet`. Sets a `Dog` instance and checks `instanceof Animal`.
/// Asserts contravariant property types are accepted per PHP semantics.
#[test]
fn test_interface_set_property_contract_allows_contravariant_type() {
    let out = compile_and_run(
        r#"<?php
class Animal {}
class Dog extends Animal {}

interface DogSink {
    public Dog $pet { set; }
}

class Kennel implements DogSink {
    public Animal $pet;
}

$kennel = new Kennel();
$kennel->pet = new Dog();
echo $kennel->pet instanceof Animal;
"#,
    );
    assert_eq!(out, "1");
}

/// Verifies an abstract class can defer interface property implementation to a concrete child.
/// Fixture: interface `HasName` with `string $name { get; set; }`, abstract `NamedBase implements HasName`,
/// concrete `Product extends NamedBase` with a default field initializer.
/// Asserts reading the property on the concrete child resolves via the abstract's interface contract.
#[test]
fn test_abstract_class_can_defer_interface_property_to_child() {
    let out = compile_and_run(
        r#"<?php
interface HasName {
    public string $name { get; set; }
}

abstract class NamedBase implements HasName {
}

class Product extends NamedBase {
    public string $name = "widget";
}

$product = new Product();
echo $product->name;
"#,
    );
    assert_eq!(out, "widget");
}

/// Verifies a PHP 8.3+ static interface method: an interface may declare a `static` method,
/// and an implementing class satisfies it with a static method, dispatched by class.
/// Fixture: interface `Previewable` with `static previews(): array`, final `C` implementing it.
#[test]
fn test_static_interface_method() {
    let out = compile_and_run(
        r#"<?php
interface Previewable {
    public static function previews(): array;
}

final class C implements Previewable {
    public static function previews(): array {
        return ['a', 'b', 'c'];
    }
}

echo implode(',', C::previews());
"#,
    );
    assert_eq!(out, "a,b,c");
}

/// Verifies a concrete child satisfies a static interface method when the interface is
/// implemented by an abstract parent class, and `#[\Override]` on the child's static
/// implementation resolves through the parent's inherited interfaces.
#[test]
fn test_static_interface_method_via_abstract_parent() {
    let out = compile_and_run(
        r#"<?php
interface Previewable {
    public static function previews(): array;
}

abstract class Base implements Previewable {
}

class C extends Base {
    #[\Override]
    public static function previews(): array {
        return ['x', 'y'];
    }
}

echo implode(',', C::previews());
"#,
    );
    assert_eq!(out, "x,y");
}

/// Verifies `#[\Override]` is accepted on a static interface-method implementation
/// (the override target is the interface's static method, matched via `InterfaceInfo.static_methods`).
#[test]
fn test_override_on_static_interface_method() {
    let out = compile_and_run(
        r#"<?php
interface Previewable {
    public static function previews(): array;
}

final class C implements Previewable {
    #[\Override]
    public static function previews(): array {
        return ['a', 'b'];
    }
}

echo implode(',', C::previews());
"#,
    );
    assert_eq!(out, "a,b");
}

/// Regression: an interface method with >=5 integer register arguments dispatched through the
/// interface vtable must preserve the 5th/6th SysV argument registers (r8, r9).
///
/// `put(string $k, string $v)` on an interface-typed receiver marshals `this=rdi`, `$k=rsi/rdx`,
/// `$v=rcx/r8` — so `$v`'s length is the 5th register argument, held in r8. The x86-64
/// interface-dispatch scan previously used r8 as its per-entry scratch and r9 as the target
/// interface id (both argument registers), silently clobbering the 5th/6th arguments. The stored
/// value's length came out as a leftover interface id, so `strlen()` of the read-back string was
/// garbage. The receiver is an interface-typed function parameter so the call is a true virtual
/// dispatch (not devirtualized to a direct call).
#[test]
fn test_interface_dispatch_preserves_fifth_register_argument() {
    let out = compile_and_run(
        r#"<?php
interface Box {
    public function put(string $k, string $v): Box;
    public function get(string $k): string;
}

final class Crate implements Box {
    /** @var array<string, string> */
    private array $data = [];

    public function put(string $k, string $v): Box {
        $clone = clone $this;
        $clone->data[$k] = $v;
        return $clone;
    }

    public function get(string $k): string {
        return $this->data[$k];
    }
}

function run(Box $box): string {
    $c = $box->put('a', 'xyz');
    return strlen($c->get('a')) . ':' . $c->get('a');
}

echo run(new Crate());
"#,
    );
    assert_eq!(out, "3:xyz");
}

/// Regression (#622 fix b): PHP locals are not type-locked, so reassigning a variable to a
/// SUPERTYPE value widens it. A PSR-7 fluent wither declared `@return static` (self-returning
/// interface method) reassigned onto its own receiver must be permitted, not rejected as
/// "cannot reassign $m from Message to MessageLike". Mirrors SseResponseWriter /
/// SuperglobalRequestExtractor `$response = $response->with...()`.
#[test]
fn test_interface_wither_reassignment_to_declared_supertype() {
    let out = compile_and_run(
        r#"<?php
interface MessageLike {
    /** @return static */
    public function withHeader(string $name, string $value): MessageLike;
    public function header(string $name): string;
}

final class Message implements MessageLike {
    /** @var array<string, string> */
    private array $headers = [];

    public function withHeader(string $name, string $value): MessageLike {
        $clone = clone $this;
        $clone->headers[$name] = $value;
        return $clone;
    }

    public function header(string $name): string {
        return $this->headers[$name] ?? '';
    }
}

function decorate(MessageLike $m): MessageLike {
    $m = $m->withHeader('X-A', 'alpha');
    $m = $m->withHeader('X-B', 'beta');
    return $m;
}

$m = decorate(new Message());
echo $m->header('X-A'), ',', $m->header('X-B');
"#,
    );
    assert_eq!(out, "alpha,beta");
}

/// Regression (#622 fix a): a self-returning interface method carrying `@return static`
/// (declared `: MessageLike`) called on a SUB-interface receiver resolves to the RECEIVER's
/// interface, not the declared one. So `$r = $r->withHeader(...)` on a RequestLike keeps
/// RequestLike, and RequestLike-only methods (`target()`) stay callable — the PSR-7
/// ServerRequestInterface wither shape (HttpErrorHandlerMiddleware::process etc.).
#[test]
fn test_interface_return_static_resolves_to_subtype_receiver() {
    let out = compile_and_run(
        r#"<?php
interface MessageLike {
    /** @return static */
    public function withHeader(string $name, string $value): MessageLike;
    public function header(string $name): string;
}

interface RequestLike extends MessageLike {
    public function target(): string;
}

final class Request implements RequestLike {
    /** @var array<string, string> */
    private array $headers = [];

    public function withHeader(string $name, string $value): MessageLike {
        $clone = clone $this;
        $clone->headers[$name] = $value;
        return $clone;
    }

    public function header(string $name): string {
        return $this->headers[$name] ?? '';
    }

    public function target(): string {
        return '/home';
    }
}

function decorate(RequestLike $r): RequestLike {
    $r = $r->withHeader('X-A', 'alpha');
    return $r;
}

$r = decorate(new Request());
echo $r->header('X-A'), ',', $r->target();
"#,
    );
    assert_eq!(out, "alpha,/home");
}

/// Regression (#632): a method declared on an INTERFACE, called on a receiver whose declared type
/// is a CLASS that does not declare the method, dispatches through the interface vtable when the
/// receiver is narrowed to the interface by an inline `instanceof` guard. codegen previously
/// resolved the call against the declared class and failed with "unknown method". The checker
/// records the narrowed receiver type by call span (CheckResult::narrowed_call_receivers) and IR
/// lowering retypes the receiver operand (a pure object-pointer reinterpretation) so codegen routes
/// to interface dispatch. Covers the ternary, if-return, and if-accumulator guard shapes.
#[test]
fn test_instanceof_narrowed_receiver_dispatches_via_interface() {
    let out = compile_and_run(
        r#"<?php
interface HasWeight { public function weight(): int; }
abstract class Block {}
final class Heavy extends Block implements HasWeight { public function weight(): int { return 5; } }
final class Plain extends Block {}

function ternaryWeight(Block $block): int {
    return $block instanceof HasWeight ? $block->weight() : 0;
}
function ifReturnWeight(Block $block): int {
    if ($block instanceof HasWeight) { return $block->weight(); }
    return 0;
}
function ifAccumWeight(Block $block): int {
    $weight = 0;
    if ($block instanceof HasWeight) { $weight = $block->weight(); }
    return $weight;
}

echo ternaryWeight(new Heavy()), ternaryWeight(new Plain());
echo ifReturnWeight(new Heavy()), ifReturnWeight(new Plain());
echo ifAccumWeight(new Heavy()), ifAccumWeight(new Plain());
"#,
    );
    assert_eq!(out, "505050");
}

/// Regression (#607): an interface method with NO return type declaration must infer `Mixed`,
/// not `Int`.
///
/// A body-less method has no statements to infer a return type from, so the type is genuinely
/// UNKNOWN — PHP's `mixed`. The checker previously fell through to
/// `infer_return_type_syntactic(&method.body)`, which walked the EMPTY body, collected zero
/// return statements, and landed on that helper's "no returns" default of `Int`. That `Int` then
/// poisons every caller: `is_string($x)` cannot narrow an `Int`, so passing the value to a
/// `string` parameter fails to type-check.
///
/// PSR-7's untyped `getAttribute()` is the canonical victim (a resource resolver):
///     $type = $request->getAttribute('type');
///     if (!is_string($type) || !$registry->has($type)) { ... }
/// -> "Method Registry::has parameter $entityTypeName expects Str, got Int".
///
/// This mirrors the unhinted-value-parameter -> `Mixed` rule already applied to params.
#[test]
fn test_interface_method_without_return_type_infers_mixed_not_int() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
interface Req {
    public function getAttribute(string $name, mixed $default = null);
}
final class Rq implements Req {
    public function getAttribute(string $name, mixed $default = null) {
        return $name === 'type' ? 'user' : $default;
    }
}
final class Reg {
    public function has(string $n): bool { return $n !== ''; }
    public function get(string $n): string { return 'G:' . $n; }
}
function resolve(Reg $r, Req $req, string $key): string {
    $type = $req->getAttribute($key);
    if (!is_string($type) || !$r->has($type)) {
        return 'null';
    }
    return $r->get($type);
}
echo resolve(new Reg(), new Rq(), 'type'), ';';
echo resolve(new Reg(), new Rq(), 'missing');
"#,
    );
    assert_eq!(out, "G:user;null");
}

/// Regression (#646): a boxed `Mixed` flowing into a STRING-backed enum's `tryFrom`/`from` must
/// coerce to the backing string and scan, not hard-error "backing input PHP type Mixed".
///
/// Real-world callers all guard with `is_string($raw)` before `Enum::tryFrom($raw)` (the value IS a
/// string at runtime), but the guard narrows the checker env, not the codegen value type, so the
/// lowering still saw Mixed. Coerced via the same `load_value_as_string_to_regs` the 42 other
/// string builtins use on a Mixed operand — consistent with the codebase, and the scan returns the
/// matching case or null exactly as PHP. 16 of 1082 survey roots hit this, all passing `--check`.
#[test]
fn test_string_backed_enum_tryfrom_accepts_mixed_string_input() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
enum Status: string { case Running = 'running'; case Stopped = 'stopped'; }
final class Reg {
    public static function fromRaw(mixed $raw): ?Status {
        if (!is_string($raw)) { return null; }
        return Status::tryFrom($raw);
    }
}
echo Reg::fromRaw('running')?->value ?? 'none', ';';
echo Reg::fromRaw('nope')?->value ?? 'none', ';';
echo Reg::fromRaw(42)?->value ?? 'none', ';';
echo Reg::fromRaw('stopped')?->value ?? 'none';
"#,
    );
    assert_eq!(out, "running;none;none;stopped");
}
