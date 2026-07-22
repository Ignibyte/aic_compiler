//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of namespaces, including namespace use function and global builtin resolution, namespace class can call global extern function, and namespace class can call pointer builtins without global prefix.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Multi-file fixtures exercise include/require resolution, temporary project layout, and native binary output.

use crate::support::*;

/// Verifies `use function` aliasing and global builtin resolution inside a namespaced file.
/// Uses a two-namespace fixture: `Demo\Util\render` aliased as `paint` and global `strlen`.
/// Checks that the alias resolves correctly and global builtins are accessible without prefix.
#[test]
fn test_namespace_use_function_and_global_builtin_resolution() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\Util;
function render($value) { echo $value; }

namespace Demo\App;
use function Demo\Util\render as paint;

paint("A");
echo strlen("bc");
"#,
    );
    assert_eq!(out, "A2");
}

/// Verifies that a class inside a namespace can call a global `extern function` without
/// a namespace prefix. Regression test: extern functions must resolve globally regardless
/// of the enclosing namespace context.
#[test]
fn test_namespace_class_can_call_global_extern_function() {
    let out = compile_and_run(
        r#"<?php
extern function getpid(): int;

namespace Demo\App;

class Probe {
    public function ok(): int {
        return getpid() > 0 ? 1 : 0;
    }
}

$probe = new Probe();
echo $probe->ok();
"#,
    );
    assert_eq!(out, "1");
}

/// Verifies that pointer builtins (`ptr_null`, `ptr_is_null`) are accessible inside a
/// namespaced class method without a global prefix. Regression test for builtin resolution
/// within namespace scope.
#[test]
fn test_namespace_class_can_call_pointer_builtins_without_global_prefix() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\App;

class Probe {
    public function ok(): int {
        $p = ptr_null();
        return ptr_is_null($p);
    }
}

$probe = new Probe();
echo $probe->ok();
"#,
    );
    assert_eq!(out, "1");
}

/// Verifies that string builtins (`strlen`) are accessible inside a namespaced class method
/// without a global prefix. Regression test for builtin resolution within namespace scope.
#[test]
fn test_namespace_class_can_call_string_builtin_without_global_prefix() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\App;

class Probe {
    public function ok(): int {
        return strlen("hello");
    }
}

$probe = new Probe();
echo $probe->ok();
"#,
    );
    assert_eq!(out, "5");
}

/// Verifies that a declared return type (`Box`) resolves to the same-namespace class
/// inside a typed local variable declaration (`Box $box = ...`). Checks both return-type
/// resolution and typed local variable initialization.
#[test]
fn test_namespace_resolves_class_type_hints_in_functions_and_typed_locals() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\App;

class Box {
    public $value;

    public function __construct() {
        $this->value = 7;
    }
}

function make_box(): Box {
    return new Box();
}

Box $box = make_box();
echo $box->value;
"#,
    );
    assert_eq!(out, "7");
}

/// Verifies that `buffer<Vertex>` works correctly when `Vertex` is a packed class declared
/// in the same namespace. Tests buffer element access with typed buffer slots.
#[test]
fn test_namespace_resolves_packed_class_types_inside_buffers() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\App;

packed class Vertex {
    public int $x;
    public int $y;
}

class Probe {
    public function run(): int {
        buffer<Vertex> $points = buffer_new<Vertex>(1);
        $points[0]->x = 3;
        $points[0]->y = 4;
        return $points[0]->x + $points[0]->y;
    }
}

$probe = new Probe();
echo $probe->run();
"#,
    );
    assert_eq!(out, "7");
}

/// Verifies that property types converge across a chain of classes: `Box.items` holds a
/// `buffer<Point>` written by `Loader.load()`, and `Game.run()` reads `items[0]->x`.
/// Regression test for post-pass type convergence across class boundaries.
#[test]
fn test_method_post_pass_converges_property_types_across_classes() {
    let out = compile_and_run(
        r#"<?php
packed class Point {
    public int $x;
}

class Box {
    public $items;

    public function __construct() {
        $this->items = 0;
    }
}

class Loader {
    public function load(): Box {
        $box = new Box();
        buffer<Point> $items = buffer_new<Point>(1);
        $items[0]->x = 7;
        $box->items = $items;
        return $box;
    }
}

class Game {
    public $box;

    public function __construct() {
        $this->box = 0;
    }

    public function run(): int {
        $loader = new Loader();
        $this->box = $loader->load();
        return $this->box->items[0]->x;
    }
}

$game = new Game();
echo $game->run();
"#,
    );
    assert_eq!(out, "7");
}

/// Verifies that a forward class reference in a method return type (`load(): Item`) resolves
/// even when `Item` is defined after the method. Both classes are in the global namespace.
#[test]
fn test_forward_class_reference_in_method_return_type() {
    let out = compile_and_run(
        r#"<?php
class Loader {
    public function load(): Item {
        return new Item();
    }
}

class Item {
    public $value;

    public function __construct() {
        $this->value = 9;
    }
}

$loader = new Loader();
$item = $loader->load();
echo $item->value;
"#,
    );
    assert_eq!(out, "9");
}

/// Verifies that property array access (`$this->items[0]`) works correctly when `items`
/// is an array property initialized in the constructor. Regression test for property lookup
/// followed by subscript in the same expression.
#[test]
fn test_property_array_access_after_property_lookup() {
    let out = compile_and_run(
        r#"<?php
class Bag {
    public $items;

    public function __construct() {
        $this->items = [10, 20, 30];
    }

    public function first(): int {
        return $this->items[0];
    }
}

$bag = new Bag();
echo $bag->first();
"#,
    );
    assert_eq!(out, "10");
}

/// Verifies that a typed parameter (`string $text`) is correctly available inside the
/// method body for use in a subsequent call (`strlen($text)`). All types are in the
/// global namespace.
#[test]
fn test_typed_method_param_is_available_with_declared_type_in_body() {
    let out = compile_and_run(
        r#"<?php
class Reader {
    public function len(string $text): int {
        return strlen($text);
    }
}

$reader = new Reader();
echo $reader->len("doom");
"#,
    );
    assert_eq!(out, "4");
}

/// Verifies that a typed constructor parameter (`string $bytes`) is not overwritten by
/// untyped property inference. The property `$bytes` should be set from the parameter,
/// and `$this->bytes` should remain accessible in the method body. Regression test for
/// constructor parameter vs. property name collision.
#[test]
fn test_typed_constructor_param_is_not_overwritten_by_untyped_property_inference() {
    let out = compile_and_run(
        r#"<?php
class Blob {
    public $bytes;

    public function __construct(string $bytes) {
        $this->bytes = $bytes;
    }

    public function len(): int {
        return strlen($this->bytes);
    }
}

$blob = new Blob("doom");
echo $blob->len();
"#,
    );
    assert_eq!(out, "4");
}

/// Verifies that `require` inside a namespace preserves the required file's namespace
/// context (`Demo\Lib`) while the main file uses `use Demo\Lib\User`. Multi-file fixture
/// with `main.php` and `lib.php`; regression test for include-time namespace context.
#[test]
fn test_namespace_include_preserves_class_namespace_context() {
    let out = compile_and_run_files(
        &[
            (
                "main.php",
                r#"<?php
namespace Demo\App;
require "lib.php";

use Demo\Lib\User;

$user = new User();
echo $user->label();
"#,
            ),
            (
                "lib.php",
                r#"<?php
namespace Demo\Lib;

class User {
    public function label() {
        return "ok";
    }
}
"#,
            ),
        ],
        "main.php",
    );
    assert_eq!(out, "ok");
}

/// Verifies that `use const` aliasing and fully-qualified constant paths both resolve
/// correctly in the same namespace (`Demo\Values\ANSWER` aliased as `ANSWER` and accessed
/// via `\Demo\Values\ANSWER`). Checks both resolution paths produce the same value.
#[test]
fn test_namespace_use_const_and_fully_qualified_constant_resolution() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\Values;
const ANSWER = 42;

namespace Demo\App;
use const Demo\Values\ANSWER;

echo ANSWER;
echo \Demo\Values\ANSWER;
"#,
    );
    assert_eq!(out, "4242");
}

/// Verifies that fully-qualified `function_exists()` sees a namespaced function,
/// while `call_user_func` with a short name still resolves through the current
/// namespace's callback lookup.
#[test]
fn test_namespace_callback_string_literals_resolve_current_namespace() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\Callbacks;

function triple($value) {
    return $value * 3;
}

echo function_exists("Demo\\Callbacks\\triple");
echo call_user_func("triple", 4);
"#,
    );
    assert_eq!(out, "112");
}

/// Verifies that fully-qualified callback strings (`"Demo\\Support\\format_user"`) resolve
/// absolutely and that `call_user_func_array` works with the same path. Uses a namespaced
/// class method and `use` import; checks both single-argument and array-argument forms.
#[test]
fn test_namespace_fully_qualified_callback_strings_are_absolute() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\Support;

class User {
    public function badge() {
        return "ok";
    }
}

function format_user(User $user) {
    return "[" . $user->badge() . "]";
}

namespace Demo\App;

use Demo\Support\User;

echo function_exists("Demo\\Support\\format_user");
echo call_user_func("Demo\\Support\\format_user", new User());
echo call_user_func_array("Demo\\Support\\format_user", [new User()]);
"#,
    );
    assert_eq!(out, "1[ok][ok]");
}

/// Verifies that group use syntax (`use Demo\Lib\{User, function render as paint, const ANSWER}`)
/// resolves class, function, and const imports correctly within a namespaced file.
/// Checks static method call, const access, and aliased function call all produce expected output.
#[test]
fn test_namespace_group_use_resolves_class_function_and_const() {
    let out = compile_and_run(
        r#"<?php
namespace Demo\Lib;

const ANSWER = 7;

function render($value) {
    return "<" . $value . ">";
}

class User {
    public static function label() {
        return "ok";
    }
}

namespace Demo\App;

use Demo\Lib\{User, function render as paint, const ANSWER};

echo User::label();
echo ANSWER;
echo paint("x");
"#,
    );
    assert_eq!(out, "ok7<x>");
}

/// EC-12 (#495): a `new <ImportedAlias>` nested inside a NAMED ARGUMENT resolves the alias —
/// the resolver's expression walk previously had no NamedArg arm, so the value expression
/// escaped rewriting entirely ("Undefined class: Url" on the ward-component-catalog
/// `new self(label: ..., url: new Url('/'))` previews pattern). Byte-parity vs PHP 8.5.
#[test]
fn test_named_argument_value_resolves_imported_alias() {
    let out = compile_and_run(
        r#"<?php

namespace App\Url;

final class Url {
    public function __construct(public string $p) {}
}

namespace App\C;

use App\Url\Url;

final class K {
    public function __construct(public string $label, public Url $t) {}

    public static function mk(): K {
        return new self(label: 'x', t: new Url('/'));
    }
}

namespace Main;

echo \App\C\K::mk()->t->p;
"#,
    );
    assert_eq!(out, "/");
}

/// Regression: `use const` / `use function` of a name the lexer eagerly tokenizes as a
/// dedicated constant token — `DIRECTORY_SEPARATOR`, `PHP_OS`, `STDERR` — must parse. These
/// tokens were missing from `token_as_import_name`, so the import raised "Expected imported
/// name after 'use'", failing the whole file (the ward-theme/ward-manifest Emitters that lead
/// their file with `use const DIRECTORY_SEPARATOR;`).
#[test]
fn test_use_const_special_token_imports_parse() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
namespace App;

use function bin2hex;
use const DIRECTORY_SEPARATOR;
use const PHP_OS;

final class Emitter {
    public function path(string $dir, string $file): string {
        return $dir . DIRECTORY_SEPARATOR . $file;
    }
    public function hex(): string {
        return bin2hex("AZ");
    }
}

namespace Main;

$e = new \App\Emitter();
echo $e->hex(), "|", $e->path("a", "b");
"#,
    );
    assert_eq!(out, "415a|a/b");
}

/// Regression: `PHP_VERSION` and `PHP_BINARY` magic constants — eagerly tokenized like
/// `PHP_EOL`/`DIRECTORY_SEPARATOR` and folded to string literals at parse time. `PHP_VERSION`
/// folds to the compiler package version so PHP's `PHP_VERSION === phpversion()` invariant holds;
/// `PHP_BINARY` folds to `""` (an AOT binary has no separate interpreter path). Both must also be
/// importable via `use const`. Before this they raised "Undefined constant", failing the whole
/// file for ward-status-admin `SystemStatusReport` (`use const PHP_VERSION;`) and gate-scripts
/// `InfectionGate` (`use const PHP_BINARY;`).
#[test]
fn test_php_version_and_php_binary_constants() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
namespace App;

use const PHP_VERSION;
use const PHP_BINARY;

final class Report {
    public function versionMatchesPhpversion(): bool {
        return PHP_VERSION === phpversion();
    }
    public function binaryIsEmpty(): bool {
        return PHP_BINARY === "";
    }
}

namespace Main;

$r = new \App\Report();
echo $r->versionMatchesPhpversion() ? "1" : "0";
echo $r->binaryIsEmpty() ? "1" : "0";
"#,
    );
    assert_eq!(out, "11");
}

/// Regression (#625): a `use`-imported alias referenced inside the ARGUMENTS of a dynamic
/// `new $class(...)` must be name-resolved. `ExprKind::NewDynamic` had no name-resolver arm, so it
/// fell through to the catch-all and its arguments kept their short names —
/// `new $class(new Props($x))` reported "Undefined class: Props". This is the real-world
/// `new $class(new LayoutNodeProps(...))` and `new $controlClass(variant: ButtonVariant::Primary)`
/// shape (resolved fine as a plain `new`, only the dynamic-new form escaped resolution).
#[test]
fn test_dynamic_new_resolves_use_aliases_in_arguments() {
    let out = compile_and_run(
        r#"<?php
namespace Cat {
    final class Props { public function __construct(public int $x) {} }
    final class Widget { public function __construct(public Props $p) {} }
}
namespace App {
    use Cat\Props;
    use Cat\Widget;
    function make(string $class, int $x): object {
        return new $class(new Props($x));
    }
    echo make(Widget::class, 7)->p->x;
}
"#,
    );
    assert_eq!(out, "7");
}

/// Regression (#644): `new $class(...)` enumerates every arg-count-matching class as a
/// dynamic-new candidate and emits a constructor branch for each. A candidate whose
/// constructor param cannot receive the actual argument — here an `Html` object passed
/// where a sibling class declares `?int` at the same position — forced an impossible
/// `Object -> TaggedScalar` arg materialization and failed the whole module at codegen.
/// Such a candidate is only reachable by a call that is a PHP `TypeError`, so it must be
/// excluded; the intended target (whose param accepts the object) compiles and runs.
#[test]
fn test_dynamic_new_excludes_type_incompatible_candidate() {
    let out = compile_and_run(
        r#"<?php
final class Html { public function __construct(public string $s) {} }
final class WithHtml { public function __construct(public string $a, public Html $h) {} }
final class WithNullableInt { public function __construct(public string $a, public ?int $n) {} }
final class Factory {
    public function make(string $cls, Html $h): object { return new $cls("x", $h); }
}
$f = new Factory();
echo $f->make(WithHtml::class, new Html("deep"))->h->s;
"#,
    );
    assert_eq!(out, "deep");
}

/// Regression (#625): a `use`-imported alias used as a TYPED VARIADIC parameter
/// (`Outcome ...$xs`) must be name-resolved. The name resolver resolved regular parameter
/// types and return types but cloned `variadic_type` unresolved in all three declaration
/// forms (free function, class method via `..method.clone()`, and closure), so
/// `aggregate(StepOutcome ...$outcomes)` reported "Unknown type: StepOutcome" while the same
/// alias as a return type resolved fine.
#[test]
fn test_variadic_parameter_type_resolves_use_alias() {
    let out = compile_and_run(
        r#"<?php
namespace Cat {
    enum Outcome: string { case A = 'a'; case B = 'b'; }
}
namespace App {
    use Cat\Outcome;
    function agg(Outcome ...$xs): int { return count($xs); }
    final class Bag {
        public function agg(Outcome ...$xs): int { return count($xs); }
    }
    $c = function (Outcome ...$xs): int { return count($xs); };
    echo agg(Outcome::A, Outcome::B), (new Bag())->agg(Outcome::A), $c(Outcome::A, Outcome::B);
}
"#,
    );
    assert_eq!(out, "212");
}

/// Regression (#647): a leading `\` before a PHP global CONSTANT is a namespace no-op
/// (`\PHP_INT_MAX` === `PHP_INT_MAX`) and must parse.
///
/// These constants lex as DEDICATED tokens (`Token::PhpIntMax`, `Token::MPi`, `Token::True`, ...),
/// not as `Token::Identifier`. The parser routed a leading `\` through `parse_named_expr` ->
/// `parse_name`, which consumes the backslash and then demands an identifier part it never finds,
/// erroring "Expected name". A leading `\` before a FUNCTION (`\strlen(...)`) always worked, because
/// that IS an identifier — only the dedicated-token constants broke. nikic/php-parser's
/// `String_.php:125` (`\PHP_INT_MAX` in a ternary) hit this; 6 survey roots bundle that file.
///
/// Also asserts the negative: a leading `\` before a real function/name still routes normally.
#[test]
fn test_leading_backslash_on_global_constant_is_a_namespace_noop() {
    let out = compile_and_run(
        r#"<?php
declare(strict_types=1);
function pick(int $d): int {
    return \is_int($d) ? $d : \PHP_INT_MAX;
}
echo pick(5), ';';
echo \PHP_INT_MAX, ';';
echo (int) (\M_PI * 100), ';';
echo \true ? 'T' : 'F', \false ? 'T' : 'F', ';';
echo \strlen('abc'), ';';
echo \count([1, 2, 3]);
"#,
    );
    assert_eq!(out, "5;9223372036854775807;314;TF;3;3");
}
