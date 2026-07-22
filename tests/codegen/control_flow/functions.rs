//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of control flow functions, including function call integer, function call string, and function void.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Compiles a function returning the sum of two integers and verifies the result.
#[test]
fn test_function_call_int() {
    let out = compile_and_run("<?php function add($a, $b) { return $a + $b; } echo add(10, 32);");
    assert_eq!(out, "42");
}

/// Compiles a function returning a concatenated string and verifies the output.
#[test]
fn test_function_call_string() {
    let out = compile_and_run(
        "<?php function greet($name) { return \"Hello, \" . $name; } echo greet(\"World\");",
    );
    assert_eq!(out, "Hello, World");
}

/// Verifies that string concatenation inside a function return is preserved when
/// the returned value is used in further concatenation operations.
#[test]
fn test_function_returned_concat_survives_outer_concat() {
    let out = compile_and_run(
        r#"<?php
function label($name) { return "[" . $name . "]"; }
echo label("title") . "|" . label("slug");
"#,
    );
    assert_eq!(out, "[title]|[slug]");
}

/// Verifies that a function returning a builtin-produced string persists it
/// before the caller starts a new concat expression.
#[test]
fn test_function_returned_builtin_string_survives_caller_concat() {
    let out = compile_and_run(
        r#"<?php
function query_name(): string {
    return urldecode(substr("name=elephc", 5));
}

$name = query_name();
echo $name . "\n";
echo "Hello, " . $name . "!\n";
echo "Hello, " . query_name() . "!\n";
"#,
    );
    assert_eq!(out, "elephc\nHello, elephc!\nHello, elephc!\n");
}

/// Compiles a void function that echoes a value and returns early, then verifies
/// the side effect occurs correctly when the function is called as a statement.
#[test]
fn test_function_void() {
    let out = compile_and_run("<?php function say() { echo \"hi\"; return; } say();");
    assert_eq!(out, "hi");
}

/// Verifies that variables inside a function body do not leak to the outer scope,
/// and that the global variable remains unchanged after the function call.
#[test]
fn test_function_local_scope() {
    let out = compile_and_run(
        "<?php $x = 1; function get_two() { $x = 2; return $x; } echo $x . \" \" . get_two();",
    );
    assert_eq!(out, "1 2");
}

/// Compiles a recursive function computing factorial and verifies correct evaluation
/// of 5! = 120.
#[test]
fn test_function_recursive() {
    let out = compile_and_run(
        "<?php function fact($n) { if ($n <= 1) { return 1; } return $n * fact($n - 1); } echo fact(5);",
    );
    assert_eq!(out, "120");
}

/// Verifies that a function can be called multiple times with different arguments
/// and each call returns the correct independent result.
#[test]
fn test_function_multiple_calls() {
    let out = compile_and_run(
        "<?php function double($x) { return $x * 2; } echo double(3) . \" \" . double(7);",
    );
    assert_eq!(out, "6 14");
}

/// Verifies that the return value of a function can be passed directly as an
/// argument to another function call, with correct evaluation order.
#[test]
fn test_function_as_argument() {
    let out = compile_and_run(
        "<?php function add($a, $b) { return $a + $b; } echo add(add(1, 2), add(3, 4));",
    );
    assert_eq!(out, "10");
}

/// Compiles a function with no parameters that returns a constant integer.
#[test]
fn test_function_no_args() {
    let out = compile_and_run("<?php function answer() { return 42; } echo answer();");
    assert_eq!(out, "42");
}

// --- Logical operators ---

/// EC-8 (#491): `if ($x === false) { throw; } return $x;` narrows an `int|false` value to `int`
/// after the divergent guard, so the `: int` return matches. Byte-parity vs PHP 8.5.
#[test]
fn test_strict_false_guard_narrowing() {
    let out = compile_and_run(
        "<?php final class G { public static function requireInt(int|false $v): int { if ($v === false) { throw new \\RuntimeException('no'); } return $v; } } echo G::requireInt(42), ':', G::requireInt(7);",
    );
    assert_eq!(out, "42:7");
}

/// EC-8 (#491): `if ($x === null) { throw; } return $x;` narrows a nullable value to non-null
/// after the divergent guard (elephc models `?T`'s null as Void), so `?string`→string and
/// `?self`→self. Byte-parity vs PHP 8.5.
#[test]
fn test_strict_null_guard_narrowing() {
    let out = compile_and_run(
        "<?php function req(?string $x): string { if ($x === null) { throw new \\Exception('no'); } return $x; } echo req('hi');",
    );
    assert_eq!(out, "hi");
}

/// EC-8 (#491): `$this->prop instanceof X ? ... : <uses $this->prop>` narrows the PROPERTY in the
/// ternary else-branch (Message|string → string), so `new Message($this->prop)` type-checks.
/// Byte-parity vs PHP 8.5. Exercises property-access flow-narrowing across ternary branches.
#[test]
fn test_property_instanceof_ternary_narrowing() {
    let out = compile_and_run(
        "<?php final class Message { public function __construct(public string $key) {} } final class V { public function __construct(private Message|string $raw) {} public function msg(): Message { return $this->raw instanceof Message ? $this->raw : new Message($this->raw); } } echo (new V('hi'))->msg()->key, ':', (new V(new Message('k')))->msg()->key;",
    );
    assert_eq!(out, "hi:k");
}

/// EC-8 (#491): `if (is_null($x)) { throw; }` narrows ?int → int on the fall-through path — the
/// same complement-stripping as `$x === null` (ward-schema ColumnNode::assertDecimalPrecision).
/// Byte-parity vs PHP 8.5.
#[test]
fn test_is_null_guard_narrowing() {
    let out = compile_and_run(
        "<?php function f(?int $p): int { if (is_null($p)) { throw new \\InvalidArgumentException('null'); } if ($p <= 0) { throw new \\InvalidArgumentException('non-positive'); } return $p; } echo f(5);",
    );
    assert_eq!(out, "5");
}

/// EC-8 (#491): a negated-instanceof throw-guard on a PROPERTY narrows it for the statements
/// after the `if` (ward-forms StoreResult::ref pattern: `?StoredFileRef` → StoredFileRef on the
/// fall-through return). Byte-parity vs PHP 8.5.
#[test]
fn test_property_throw_guard_narrowing() {
    let out = compile_and_run(
        "<?php final class W { public function __construct(public string $v) {} } final class R { public function __construct(private ?W $w) {} public function ref(): W { if (!$this->w instanceof W) { throw new \\LogicException('rejected'); } return $this->w; } } echo (new R(new W('x')))->ref()->v;",
    );
    assert_eq!(out, "x");
}

/// Regression: the POSITIVE `!== null` guard narrows inside its own body.
///
/// This is a real-world code shape — `if ($error !== null) { f($error); }` with a
/// `?string $error` parameter — and before `guard_receiver_and_type` reported comparison
/// polarity only the early-return form (`if ($x === null) { throw; }`) narrowed. Without it a
/// `?string` never satisfied a `string` parameter inside its own null check, which is one of
/// the shapes the any-member union compatibility arm existed to paper over. Covers the
/// reversed-operand form and the double negation, whose polarity must XOR back to `=== null`.
#[test]
fn test_strict_not_null_guard_narrows_in_then_branch() {
    let out = compile_and_run(
        r#"<?php
function alert(string $error): string { return "[$error]"; }
function depth(int $d): string { return "<$d>"; }
function createForm(string $title, ?string $error): string {
    $out = $title;
    if ($error !== null) { $out .= alert($error); }
    return $out;
}
function rev(?string $e): string { return (null !== $e) ? alert($e) : 'none'; }
function dbl(?int $d): string { if (!($d !== null)) { return 'nil'; } return depth($d); }
function ebranch(?string $e): string { if ($e !== null) { return alert($e); } return 'empty'; }
echo createForm('A', null), '|', createForm('B', 'boom'), '|';
echo rev(null), '|', rev('x'), '|';
echo dbl(null), '|', dbl(7), '|';
echo ebranch(null), '|', ebranch('z');
"#,
    );
    assert_eq!(out, "A|B[boom]|none|[x]|nil|<7>|empty|[z]");
}

/// Regression: a bare `assert(<guard>)` statement narrows the rest of the scope.
///
/// `hash_file()` returns `string|false`; an asset hasher asserts it to a string and returns
/// it from a `: string` method. PHP compiles `assert()` out under `zend.assertions=-1`, so this
/// trusts the developer's contract rather than proving it — the same posture PHPStan takes.
#[test]
fn test_assert_guard_narrows_following_statements() {
    let out = compile_and_run(
        r#"<?php
function hashFile(string $algo, string $path): string {
    $hexDigest = hash_file($algo, $path);
    assert(is_string($hexDigest));
    return $hexDigest;
}
$tmp = tempnam(sys_get_temp_dir(), 'ah');
file_put_contents($tmp, 'hello');
echo strlen(hashFile('sha256', $tmp)), ':', hashFile('md5', $tmp);
unlink($tmp);
"#,
    );
    assert_eq!(out, "64:5d41402abc4b2a76b9719d911017c592");
}

/// Regression: a narrowed nullable object (`?T` → `T`) passed to a typed-object parameter is
/// unboxed at the call boundary instead of handing the callee the boxed-Mixed cell pointer.
///
/// A nullable object has codegen repr `Mixed` (a boxed cell), so `$s` from `?SessionInterface`
/// narrowed to `SessionInterface` and passed to a `SessionInterface` parameter previously arrived
/// boxed and was dereferenced as an object → segfault. Covers the `!instanceof || !f($s)` and
/// `instanceof && f($s)` short-circuit shapes (ward-admin CSRF guards) and the plain
/// `=== null` early-return form. Byte-parity vs PHP 8.5.
#[test]
fn test_narrowed_nullable_object_arg_is_unboxed_for_typed_param() {
    let out = compile_and_run(
        r#"<?php
interface I { public function id(): string; }
final class R implements I { public function id(): string { return 'sid'; } }
function tok(I $s, string $t): bool { return $s->id() === $t; }
function orGuard(?I $s, string $t): string {
    if (!$s instanceof I || !tok($s, $t)) { return 'no'; }
    return 'ok:' . $s->id();
}
function andGuard(?I $s, string $t): string {
    if ($s instanceof I && tok($s, $t)) { return 'ok:' . $s->id(); }
    return 'no';
}
function nullGuard(?I $s): string {
    if ($s === null) { return 'no'; }
    return $s->id();
}
echo orGuard(null, 'sid'), '|', orGuard(new R(), 'sid'), '|', orGuard(new R(), 'x'), ';';
echo andGuard(null, 'sid'), '|', andGuard(new R(), 'sid'), ';';
echo nullGuard(null), '|', nullGuard(new R());
"#,
    );
    assert_eq!(out, "no|ok:sid|no;no|ok:sid;no|sid");
}

/// Regression: a `while (is_string($x))` condition narrows `$x` to `string` inside the loop
/// body, so a call taking a `string` parameter type-checks. The body may reassign `$x` to a
/// wider type (here `parentClass()` returns `?string`); the next iteration re-tests the
/// condition, so entry-narrowing is sound. Byte-parity vs PHP 8.5.
#[test]
fn test_while_condition_narrows_loop_body() {
    let out = compile_and_run(
        r#"<?php
interface Reg { public function parentClass(string $c): ?string; }
final class R implements Reg {
    public function parentClass(string $c): ?string { return $c === 'A' ? 'B' : ($c === 'B' ? 'C' : null); }
}
function depth(Reg $reg, ?string $parent): int {
    $n = 0;
    while (is_string($parent)) {
        $n++;
        $parent = $reg->parentClass($parent);
    }
    return $n;
}
echo depth(new R(), 'A'), '|', depth(new R(), 'B'), '|', depth(new R(), null);
"#,
    );
    assert_eq!(out, "3|2|0");
}

/// Regression: a property narrowing survives being read as an argument to a call NESTED inside
/// another call's arguments. `if ($this->p instanceof I) return; ... wrap(fop($this->p))` — the
/// property is read once (before either callee runs) so narrowing it to the complement is sound.
/// The checker previously double-evaluated the args (the nested call's inference purged property
/// narrowings, then the enclosing call re-validated the same node against the purged env) and
/// spuriously rejected `$this->p`. Covers static and instance nesting plus triple nesting.
/// Byte-parity vs PHP 8.5. See #653.
#[test]
fn test_property_narrowing_survives_nested_call_argument() {
    let out = compile_and_run(
        r#"<?php
interface I { public function l(): string; }
final class S {
    public function __construct(private readonly I|string $p) {}
    private static function fop(string $f): string { return "o:$f"; }
    private static function wrap(string $r): string { return "[$r]"; }
    public function g(): string {
        if ($this->p instanceof I) { return 'i'; }
        return self::wrap(self::fop($this->p));
    }
}
final class N {
    public function __construct(private readonly I|string $p) {}
    private function a(string $s): string { return "a$s"; }
    private function b(string $s): string { return "b$s"; }
    public function g(): string {
        if ($this->p instanceof I) { return 'i'; }
        return $this->a($this->b($this->p));
    }
}
echo (new S('x'))->g(), '|', (new N('y'))->g();
"#,
    );
    assert_eq!(out, "[o:x]|aby");
}
