//! Purpose:
//! Regression coverage for `self`, `parent` and `static` receivers inside closures and arrow
//! functions declared in class methods.
//!
//! Called from:
//! - The callables codegen integration suite.
//!
//! Key details:
//! - PHP binds these receivers lexically, so a closure resolves them against the class of the
//!   method that declares it, whether the closure is static or not.
//! - A closure's EIR function is named `__eir_closure_*`, not `Class::method`; codegen reads its
//!   lexical class scope instead of the name.

use crate::support::*;

/// A static arrow function calls a private static method through `self::`.
#[test]
fn test_static_arrow_function_calls_self_static_method() {
    let out = compile_and_run(r#"<?php
final class LexicalDoubler {
    public function all(array $values): array {
        return array_map(static fn (int $n): int => self::twice($n), $values);
    }
    private static function twice(int $n): int { return $n * 2; }
}
echo implode(",", (new LexicalDoubler())->all([1, 2, 3])), "\n";
"#);
    assert_eq!(out, "2,4,6\n");
}

/// A non-static closure reads `self::` constants and static properties and calls `self::` methods.
#[test]
fn test_closure_reads_self_constant_property_and_method() {
    let out = compile_and_run(r#"<?php
final class LexicalCounter {
    private const int STEP = 10;
    private static int $base = 5;
    public function run(): string {
        $add = function (int $n): int { return self::$base + self::STEP + self::offset($n); };
        return $add(1) . "," . $add(2);
    }
    private static function offset(int $n): int { return $n * 100; }
}
echo (new LexicalCounter())->run(), "\n";
"#);
    assert_eq!(out, "115,215\n");
}

/// A closure in a subclass method calls the parent's static method through `parent::`.
#[test]
fn test_closure_calls_parent_static_method() {
    let out = compile_and_run(r#"<?php
class LexicalGreeterBase {
    protected static function greet(string $name): string { return "hello " . $name; }
}
final class LexicalGreeter extends LexicalGreeterBase {
    protected static function greet(string $name): string { return "child " . $name; }
    public function names(array $names): array {
        return array_map(static fn (string $name): string => parent::greet($name), $names);
    }
}
echo implode(";", (new LexicalGreeter())->names(["a", "b"])), "\n";
"#);
    assert_eq!(out, "hello a;hello b\n");
}

/// A closure declared in a static method resolves `self::` when that method is called statically.
#[test]
fn test_closure_in_static_method_calls_self() {
    let out = compile_and_run(r#"<?php
final class LexicalSquares {
    public static function of(array $values): array {
        $square = fn (int $n): int => self::square($n);
        return array_map($square, $values);
    }
    private static function square(int $n): int { return $n * $n; }
}
echo implode(",", LexicalSquares::of([2, 3])), "\n";
"#);
    assert_eq!(out, "4,9\n");
}

/// `static::` in a closure follows the class the declaring method was called on.
#[test]
fn test_closure_static_receiver_follows_called_class() {
    let out = compile_and_run(r#"<?php
class LexicalNamed {
    public static function label(): string { return "base"; }
    public static function labels(): array {
        return array_map(static fn (int $n): string => static::label() . $n, [1, 2]);
    }
}
final class LexicalNamedChild extends LexicalNamed {
    public static function label(): string { return "child"; }
}
echo implode(",", LexicalNamed::labels()), "\n";
echo implode(",", LexicalNamedChild::labels()), "\n";
"#);
    assert_eq!(out, "base1,base2\nchild1,child2\n");
}
