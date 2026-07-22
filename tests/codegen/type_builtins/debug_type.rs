//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of `get_debug_type()` — the
//! stdlib-prelude polyfill whose `get_class($mixed)` call once made the ENTIRE stdlib prelude
//! un-compilable natively.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout against PHP.

use super::*;

/// Regression (#638): `get_debug_type()` must NATIVELY COMPILE.
///
/// The stdlib prelude's polyfill read:
///
///     function get_debug_type(mixed $value): string {
///         if (is_object($value)) { return get_class($value); }   // <-- un-lowerable
///         ...
///
/// `get_class()`'s lowering (`lower_class_name_lookup`) has no `PhpType::Mixed` arm and hard-errors
/// with "get_class for PHP type Mixed". And the value CANNOT be narrowed out of Mixed: elephc has no
/// "any object" PhpType — only `Object(ClassName)` — so neither `is_object()` nor even an `object`
/// type hint yields an object type. The call could therefore never lower.
///
/// Because the shared stdlib bundle injects all its functions together, that ONE call made every
/// program that triggers the prelude fail to compile natively — 114 of 1082 roots in the conformance survey,
/// the single largest native gap. It is also why `explode`/`strtr`/`arity_ext`/`substr_compare` were
/// each split into their own one-function preludes purely to avoid dragging this bundle in (see their
/// module docs).
///
/// Fix: use `$value::class`, which is identical for an object (the `is_object()` guard has just
/// established that) but desugars to the `__elephc_class_name_of` intrinsic, whose Mixed arm routes
/// to the `__rt_class_name_of` runtime helper.
///
/// NOTE: `--check` passed this all along — only a native compile ever saw it.
#[test]
fn test_get_debug_type_compiles_natively_for_every_value_kind() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
final class Foo {}
echo get_debug_type(new Foo()), ';';
echo get_debug_type(null), ';';
echo get_debug_type(true), ';';
echo get_debug_type(1), ';';
echo get_debug_type(1.5), ';';
echo get_debug_type('s'), ';';
echo get_debug_type([1, 2]);
"#,
    );
    assert_eq!(out, "Foo;null;bool;int;float;string;array");
}

/// Regression (#638): the underlying capability — a dynamic `$expr::class` on a boxed `Mixed`
/// receiver resolves the object's class name at runtime, and a non-object receiver never reaches
/// the intrinsic because the `is_object()` guard short-circuits first.
///
/// This is the construct `get_debug_type` now relies on, pinned independently of the prelude so a
/// regression in either surfaces distinctly.
#[test]
fn test_dynamic_class_on_mixed_receiver_resolves_at_runtime() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
final class A {}
final class B {}
function f(mixed $v): string {
    if (is_object($v)) {
        return $v::class;
    }
    return 'not-object';
}
echo f(new A()), ';', f(new B()), ';', f(1), ';', f('s');
"#,
    );
    assert_eq!(out, "A;B;not-object;not-object");
}

/// Regression (#644): a boxed `Mixed` value converting to a nullable scalar (`?int`) at a
/// runtime-call boundary must lower (unbox + reorder into the tagged-scalar registers), preserving
/// both an int payload and null.
///
/// The `runtime_call` conversion block handled Int/Bool/Void sources into a TaggedScalar result but
/// omitted the `Mixed | Union` arm that the canonical `coerce_loaded_value_to_tagged_scalar` already
/// has, so a `mixed` value flowing to a `?int` return/param/slot failed with
/// "runtime_call from Mixed to TaggedScalar" — 44 of 1082 survey roots, all passing `--check`.
#[test]
fn test_mixed_value_converts_to_nullable_int_at_runtime_call() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function m(mixed $v): mixed { return $v; }
function f(mixed $v): ?int { return m($v); }
echo f(5) ?? -1, ';';
echo f(null) ?? -1, ';';
echo f(-7) ?? -1;
"#,
    );
    assert_eq!(out, "5;-1;-7");
}
