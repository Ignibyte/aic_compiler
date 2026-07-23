//! Purpose:
//! Integration tests for flow-sensitive type narrowing on `is_*` / `instanceof` guards.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Fixtures exercise scalar narrowing (functions and methods), negated guards, the early-return
//!   idiom, `instanceof` narrowing with method dispatch on a runtime-Mixed receiver, `if`/`elseif`
//!   chains, and `: never`-function divergence that keeps the complement after an exhaustive chain.
//!   The guarded variables are untyped parameters that are unions at runtime (heterogeneous calls),
//!   so these tests depend on both the union parameter inference and the narrowing. Outputs match PHP.

use super::*;

/// Verifies the literal `false` subtype remains callable and uses the normal boolean runtime
/// representation while `int|false` narrows to int after a divergent strict-false guard.
#[test]
fn test_literal_false_type_and_strict_guard_runtime() {
    let out = compile_and_run(
        r#"<?php
        function onlyFalse(false $value): string { return $value === false ? "false" : "bad"; }
        function returnFalse(): false { return false; }
        function requireInt(int|false $value): int {
            if ($value === false) { throw new Exception("false"); }
            return $value;
        }
        echo onlyFalse(returnFalse()), ":", requireInt(9);
        "#,
    );
    assert_eq!(out, "false:9");
}

/// Verifies a builtin `int|false` result keeps the literal-false member precise, allowing the
/// standard divergent guard to leave a plain integer on the fallthrough path.
#[test]
fn test_builtin_false_sentinel_narrows_to_success_type() {
    let out = compile_and_run(
        r#"<?php
        function requirePosition(string $haystack, string $needle): int {
            $position = strpos($haystack, $needle);
            if ($position === false) { throw new Exception("not found"); }
            return $position;
        }
        echo requirePosition("abcdef", "cd");
        "#,
    );
    assert_eq!(out, "2");
}

/// Verifies an unrelated local assignment preserves a stable property narrowing in the same
/// branch; invalidation is scoped to the receiver that was actually rebound.
#[test]
fn test_property_narrowing_survives_unrelated_local_assignment() {
    let out = compile_and_run(
        r#"<?php
        final class NarrowedValue { public function __construct(public string $text) {} }
        final class NarrowedBox {
            public function __construct(public ?NarrowedValue $value) {}
        }
        function readBox(NarrowedBox $box): NarrowedValue {
            if (!$box->value instanceof NarrowedValue) { throw new Exception("missing"); }
            $unrelated = 1;
            return $box->value;
        }
        echo readBox(new NarrowedBox(new NarrowedValue("ok")))->text;
        "#,
    );
    assert_eq!(out, "ok");
}

/// Verifies `is_int` narrowing in a function: the then-branch uses the value as an int and the
/// else-branch as a string, with the parameter being `int|string` across the two call sites.
#[test]
fn test_is_int_narrowing_function_then_else() {
    let out = compile_and_run(
        r#"<?php
        function f($x): void {
            if (is_int($x)) { echo "int:", $x, "\n"; } else { echo "str:", $x, "\n"; }
        }
        f(5);
        f("hi");
        "#,
    );
    assert_eq!(out, "int:5\nstr:hi\n");
}

/// Verifies `is_int` narrowing on an instance-method parameter feeding a typed `int` property:
/// the narrowed value is stored into `int $a`, while non-int calls are ignored.
#[test]
fn test_is_int_narrowing_method_into_typed_property() {
    let out = compile_and_run(
        r#"<?php
        class Bar {
            public int $a = 0;
            public function set($x): void { if (is_int($x)) { $this->a = $x; } }
        }
        $o = new Bar();
        $o->set(7);
        $o->set("ignored");
        echo $o->a;
        "#,
    );
    assert_eq!(out, "7");
}

/// Verifies a negated guard (`!is_int`) narrows the else-path (fallthrough) to int.
#[test]
fn test_negated_is_int_guard_narrows_fallthrough() {
    let out = compile_and_run(
        r#"<?php
        function f($x): string { if (!is_int($x)) { return "notint"; } return "int:" . $x; }
        echo f(5), "|", f("hi");
        "#,
    );
    assert_eq!(out, "int:5|notint");
}

/// Verifies `is_string` narrowing lets the guarded value be used by a string builtin.
#[test]
fn test_is_string_narrowing_allows_strlen() {
    let out = compile_and_run(
        r#"<?php
        function f($x): int { if (is_string($x)) { return strlen($x); } return -1; }
        echo f("abc"), "|", f(5);
        "#,
    );
    assert_eq!(out, "3|-1");
}

/// Verifies the early-return idiom: a guard with no `else` whose body always returns narrows the
/// statements after the `if` to the complement type.
#[test]
fn test_early_return_guard_narrows_remainder() {
    let out = compile_and_run(
        r#"<?php
        function f($x): string { if (!is_string($x)) { return "no"; } return "len" . strlen($x); }
        echo f("abc"), "|", f(5);
        "#,
    );
    assert_eq!(out, "len3|no");
}

/// Verifies `instanceof` narrowing lets a method be called on the narrowed object, dispatched on
/// the runtime class of a value that is `Foo|int` at runtime.
#[test]
fn test_instanceof_narrowing_allows_method_call() {
    let out = compile_and_run(
        r#"<?php
        class Foo { public function ts(): int { return 42; } }
        function g($x): int { if ($x instanceof Foo) { return $x->ts(); } return -1; }
        echo g(new Foo()), "|", g(5);
        "#,
    );
    assert_eq!(out, "42|-1");
}

/// Verifies `instanceof` narrowing picks one class out of a union of two object classes and
/// dispatches its method correctly, with the non-matching class taking the else-path.
#[test]
fn test_instanceof_narrowing_two_object_union() {
    let out = compile_and_run(
        r#"<?php
        class A { public function name(): string { return "A"; } }
        class B { public function name(): string { return "B"; } }
        function pick($x): string { if ($x instanceof A) { return $x->name(); } return "notA"; }
        echo pick(new A()), "|", pick(new B());
        "#,
    );
    assert_eq!(out, "A|notA");
}

/// Verifies the full overload pattern: an `is_int` guard stores the int into a typed property,
/// while the else-branch calls a method on the narrowed object (dispatched on its runtime class).
#[test]
fn test_overload_pattern_int_or_object() {
    let out = compile_and_run(
        r#"<?php
        class Foo { public function ts(): int { return 42; } }
        class Bar {
            public int $a = 0;
            public int $b = 0;
            public function set($x): void {
                if (is_int($x)) { $this->a = $x; } else { $this->b = $x->ts(); }
            }
        }
        $o = new Bar();
        $o->set(5);
        $o->set(new Foo());
        echo $o->a, "|", $o->b;
        "#,
    );
    assert_eq!(out, "5|42");
}

/// Tests flow-sensitive narrowing across an `if` / `elseif` / `else` chain.
/// Each clause should see the appropriate narrowed (or complement) type.
#[test]
fn test_elseif_narrowing_chain() {
    let out = compile_and_run(
        r#"<?php
        function describe($x): string {
            if (is_int($x)) {
                return "int:" . ($x + 1);
            } elseif (is_string($x)) {
                return "str:" . $x;
            }
            return "other";
        }
        echo describe(41), "|", describe("hi"), "|", describe(3.14);
        "#,
    );
    assert_eq!(out, "int:42|str:hi|other");
}

/// Tests that a branch ending in a `: never` function call lets the code after an
/// if/elseif chain (with no final else) keep the complement type. After the chain `$x`
/// must be narrowed to `int`, which the `: int` return type requires; without `never`
/// detection `$x` would still be `mixed` and `return $x` would be rejected (a `mixed`
/// value does not satisfy an `int` return). The `mixed` parameter avoids the method-call
/// route, which would not distinguish the cases since `mixed` receivers dispatch
/// dynamically.
#[test]
fn test_elseif_chain_with_never_divergence() {
    let out = compile_and_run(
        r#"<?php
        function fail(): never {
            throw new \Exception("boom");
        }

        function classify(mixed $x): int {
            if (is_string($x)) {
                return 0;
            } elseif (!is_int($x)) {
                fail();
            }
            // All clauses diverge and there is no else, so reaching here means every guard
            // was false => $x must be an int.
            return $x;
        }

        echo classify("hi"), "|", classify(41);
        "#,
    );
    assert_eq!(out, "0|41");
}

/// Regression: a non-diverging clause *before* a diverging type guard must not leave the
/// variable narrowed after the `if`. The `$flag` branch reaches the trailing statement
/// without ever evaluating the `instanceof` guard, so `$x` must stay `mixed` (where `+` is
/// allowed) rather than being narrowed to `Box` (where arithmetic is rejected). A rule that
/// kept the complement whenever only the last clause diverged would fail to compile here.
#[test]
fn test_narrowing_not_kept_when_earlier_clause_falls_through() {
    let out = compile_and_run(
        r#"<?php
        class Box {}
        function f(mixed $x, bool $flag): void {
            if ($flag) {
                echo "";
            } elseif (!($x instanceof Box)) {
                return;
            }
            echo $x + 1;
        }
        f(41, true);
        "#,
    );
    assert_eq!(out, "42");
}

/// Regression: when different clauses narrow different variables, every narrowed variable
/// must be restored after the `if`, not only the first. The non-diverging `$x` clause means
/// the trailing statement can run with `$y` unconstrained, so `$y` must stay `mixed` rather
/// than leaking the `Box` narrowing from its diverging clause. A single-slot restore would
/// leave `$y` as `Box` and reject the arithmetic.
#[test]
fn test_narrowing_restores_all_narrowed_variables() {
    let out = compile_and_run(
        r#"<?php
        class Box {}
        function f(mixed $x, mixed $y): void {
            if ($x instanceof Box) {
                echo "";
            } elseif (!($y instanceof Box)) {
                return;
            }
            echo $y + 1;
        }
        f(new Box(), 7);
        "#,
    );
    assert_eq!(out, "8");
}

/// An `is_array()` guard on a literal-indexed element of a genuinely-array root narrows that
/// element for the guarded flow (#695 D3): `$decoded['packages']` reads as the narrowed array
/// inside the branch, so `count()` compiles and runs. Negated-guard early-return shape mirrors
/// AIC's `SystemStatusReport`. The un-guarded path stays un-narrowed (`no-packages`), and a
/// non-array root (`not-array`) never reaches the element guard.
#[test]
fn test_array_element_guard_narrows_literal_indexed_read() {
    let out = compile_and_run(
        r#"<?php
        function report(string $json): string {
            $decoded = json_decode($json, true);
            if (!is_array($decoded)) { return 'not-array'; }
            if (!is_array($decoded['packages'])) { return 'no-packages'; }
            return 'n=' . count($decoded['packages']);
        }
        echo report('{"packages":{"a":1,"b":2}}'), '|', report('{"packages":3}'), '|', report('"x"');
        "#,
    );
    assert_eq!(out, "n=2|no-packages|not-array");
}

/// UNSOUNDNESS GUARD (#695 D3): an element WRITE after the guard purges the element
/// refinement, so the re-read must behave exactly like PHP — never reuse the stale "is an
/// array" fact. The root here is honest `Array(Mixed)` (json_decode), so a stale refinement
/// would be the ONLY way `is_array($d['k'])` could wrongly hold after `$d['k'] = 5`.
#[test]
fn test_array_element_refinement_purged_on_element_write() {
    let out = compile_and_run(
        r#"<?php
        function stale2(string $json): string {
            $d = json_decode($json, true);
            if (!is_array($d)) { return 'x'; }
            if (!is_array($d['k'])) { return 'no'; }
            $n1 = count($d['k']);
            $d['k'] = 5;
            $after = is_array($d['k']) ? 'arr' : 'int';
            return 'n1=' . $n1 . ',after=' . $after;
        }
        echo stale2('{"k":[1,2]}');
        "#,
    );
    assert_eq!(out, "n1=2,after=int");
}

/// The guard-false path of an element guard stays un-narrowed and takes the PHP branch
/// (#695 D3) — a non-array element under a genuinely-array root.
#[test]
fn test_array_element_guard_false_path_unnarrowed() {
    let out = compile_and_run(
        r#"<?php
        function guardFalse(array $a): string {
            if (!is_array($a['k'])) { return 'no:' . (is_int($a['k']) ? 'int' : 'other'); }
            return 'yes';
        }
        echo guardFalse(['k' => 9]);
        "#,
    );
    assert_eq!(out, "no:int");
}
