//! Purpose:
//! Integration or regression tests for end-to-end codegen coverage of control flow ternary, including ternary true, ternary false, and ternary integer.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Inline PHP fixtures are compiled to native binaries and assertions compare stdout or expected failures.

use super::*;

/// Tests ternary true branch using equality comparison that evaluates to true.
#[test]
fn test_ternary_true() {
    let out = compile_and_run("<?php echo 1 == 1 ? \"yes\" : \"no\";");
    assert_eq!(out, "yes");
}

/// Tests ternary false branch using equality comparison that evaluates to false.
#[test]
fn test_ternary_false() {
    let out = compile_and_run("<?php echo 1 == 2 ? \"yes\" : \"no\";");
    assert_eq!(out, "no");
}

/// Tests ternary with integer comparison and integer branches, selecting the larger of two values.
#[test]
fn test_ternary_int() {
    let out = compile_and_run("<?php $x = 3; $y = 7; echo $x > $y ? $x : $y;");
    assert_eq!(out, "7");
}

/// Tests ternary with mixed types when array_pop returns null on empty array.
#[test]
fn test_ternary_mixed_types_str_vs_int() {
    let out = compile_and_run(
        "<?php $a = [1]; array_pop($a); $v = array_pop($a); echo is_null($v) ? \"null\" : \"has value\";",
    );
    assert_eq!(out, "null");
}

/// Tests ternary with int condition (0) selecting false branch.
#[test]
fn test_ternary_mixed_types_then_branch_str() {
    let out = compile_and_run("<?php $x = 0; echo $x ? \"yes\" : \"no\";");
    assert_eq!(out, "no");
}

/// Tests ternary with bool true condition selecting int branch over string.
#[test]
fn test_ternary_int_string() {
    let out = compile_and_run(
        r#"<?php
$x = true;
echo $x ? 42 : "none";
"#,
    );
    assert_eq!(out, "42");
}

/// Tests ternary with bool false condition selecting string branch over int (result is "0").
#[test]
fn test_ternary_string_int() {
    let out = compile_and_run(
        r#"<?php
$x = false;
echo $x ? "yes" : 0;
"#,
    );
    assert_eq!(out, "0");
}

/// Tests ternary with bool true condition selecting string branch over another string.
#[test]
fn test_ternary_string_string() {
    let out = compile_and_run(
        r#"<?php
$x = true;
echo $x ? "hello" : "world";
"#,
    );
    assert_eq!(out, "hello");
}

/// Tests ternary with bool true condition selecting int branch over another int.
#[test]
fn test_ternary_int_int() {
    let out = compile_and_run(
        r#"<?php
$x = true;
echo $x ? 1 : 0;
"#,
    );
    assert_eq!(out, "1");
}

/// Tests ternary nested inside string concatenation with int and string result types.
#[test]
fn test_ternary_mixed_in_concat() {
    let out = compile_and_run(
        r#"<?php
$count = 5;
echo "Items: " . ($count > 0 ? $count : "none");
"#,
    );
    assert_eq!(out, "Items: 5");
}

/// Tests ternary with bool false condition selecting string branch over float.
#[test]
fn test_ternary_float_string() {
    let out = compile_and_run(
        r#"<?php
$x = false;
echo $x ? 3.14 : "zero";
"#,
    );
    assert_eq!(out, "zero");
}

/// Tests nested ternary with int condition (0) and string/int branches in both outer and inner ternary.
#[test]
fn test_ternary_nested_mixed() {
    let out = compile_and_run(
        r#"<?php
$a = 0;
echo $a ? "yes" : ($a === 0 ? "zero" : "no");
"#,
    );
    assert_eq!(out, "zero");
}

/// Tests ternary with variable as the true branch result, condition is bool true.
#[test]
fn test_ternary_variable_string() {
    let out = compile_and_run(
        r#"<?php
$name = "Alice";
$greeting = true ? $name : "nobody";
echo $greeting;
"#,
    );
    assert_eq!(out, "Alice");
}

/// Tests ternary with user-defined function call as the true branch result.
#[test]
fn test_ternary_function_result() {
    let out = compile_and_run(
        r#"<?php
function get_name() { return "Bob"; }
echo true ? get_name() : "default";
"#,
    );
    assert_eq!(out, "Bob");
}

/// Tests ternary with comparison expression as condition and int variable vs string variable as branches.
#[test]
fn test_ternary_variable_int_vs_string() {
    let out = compile_and_run(
        r#"<?php
$count = 5;
$label = "none";
echo ($count > 0) ? $count : $label;
"#,
    );
    assert_eq!(out, "5");
}

/// Tests ternary with method call on object as the true branch result.
#[test]
fn test_ternary_method_call_result() {
    let out = compile_and_run(
        r#"<?php
class Box { public $val;
    public function __construct($v) { $this->val = $v; }
    public function get() { return $this->val; }
}
$b = new Box("hello");
echo true ? $b->get() : "fallback";
"#,
    );
    assert_eq!(out, "hello");
}

/// Regression: ternary branches that index string-typed arrays must keep the
/// result string-typed. The branch merge type was inferred syntactically (no
/// element-type lookup), defaulting `$arr[$i]` to int and coercing the chosen
/// string through `str_to_i` to "0" (broke `jdmonthname(.., CAL_MONTH_JEWISH)`).
#[test]
fn test_ternary_string_array_index_branches() {
    let out = compile_and_run(
        r#"<?php
$leap = ["", "Tishri", "Heshvan"];
$reg  = ["", "Apple", "Banana"];
$month = 1;
$isLeap = true;
echo $isLeap ? $leap[$month] : $reg[$month];
echo ",";
echo $isLeap ? $leap[1] : $reg[1];
echo ",";
$isLeap = false;
echo $isLeap ? $leap[2] : $reg[2];
"#,
    );
    assert_eq!(out, "Tishri,Tishri,Banana");
}

/// Regression: ternary branches reading string-typed object properties must
/// keep the result string-typed, mirroring the array-index branch-merge fix.
#[test]
fn test_ternary_string_property_branches() {
    let out = compile_and_run(
        r#"<?php
class Names {
    public string $a = "Tishri";
    public string $b = "Heshvan";
}
$n = new Names();
$pick = true;
echo $pick ? $n->a : $n->b;
"#,
    );
    assert_eq!(out, "Tishri");
}

/// Regression (#635): a ternary whose BOTH arms are function CALLS returning a
/// non-int type must keep the result that type. Previously `branch_merge_result_type`
/// typed call arms syntactically (Int), built an Int result temp, and int-cast each
/// arm — silently returning 0 for strings.
#[test]
fn test_ternary_both_arms_string_function_calls() {
    let out = compile_and_run(
        r#"<?php
function a(): string { return "A"; }
function b(): string { return "B"; }
function pick(bool $flag): string { return $flag ? a() : b(); }
echo pick(true) . "|" . pick(false);
"#,
    );
    assert_eq!(out, "A|B");
}

/// Regression (#635): a ternary merging two METHOD calls that return OBJECTS must
/// build an object-typed result temp, not an Int one — the Int temp raised the EIR
/// backend "int cast for Object" error. Mirrors ComponentRenderer::render()'s
/// instanceof-guarded object dispatch.
#[test]
fn test_ternary_both_arms_object_method_calls() {
    let out = compile_and_run(
        r#"<?php
final class Html { public function __construct(public string $s) {} }
interface Tmpl { public function nm(): string; }
abstract class Comp { abstract public function render(): Html; }
final class Widget extends Comp implements Tmpl {
    public function render(): Html { return new Html("wr"); }
    public function nm(): string { return "wnm"; }
}
final class Plain extends Comp { public function render(): Html { return new Html("plain"); } }
final class R {
    public function go(Comp $c): Html { return $c instanceof Tmpl ? $this->cap($c) : $c->render(); }
    private function cap(Tmpl $t): Html { return new Html("tmpl:" . $t->nm()); }
}
$r = new R();
echo $r->go(new Widget())->s . "|" . $r->go(new Plain())->s;
"#,
    );
    assert_eq!(out, "tmpl:wnm|plain");
}

/// Regression (#648): a ternary arm that is a `??` expression must merge its operand types rather
/// than fall to the syntactic `Int`. `is_array($d) ? ($d['k'] ?? null) : null` fabricated
/// `Union([Int, Void])` — the `?int` TaggedScalar repr — for a value that is really a boxed Mixed,
/// so iterating the result aborted the backend with "iter_start over PHP type TaggedScalar".
/// This is `RequireCheckerAllowlist::parse`'s shape.
#[test]
fn test_ternary_arm_null_coalesce_over_mixed() {
    let out = compile_and_run(
        r#"<?php
function parseit(string $raw): array
{
    $decoded = json_decode($raw, true);
    $entries = is_array($decoded) ? ($decoded['symbol-whitelist'] ?? null) : null;
    if (!is_array($entries)) { return []; }
    $out = [];
    foreach ($entries as $entry) {
        if (!is_string($entry)) { continue; }
        $out[] = $entry;
    }
    return $out;
}
$out = parseit('{"symbol-whitelist":["a","b"]}');
echo count($out), ",", $out[0], "|", count(parseit('{}'));
"#,
    );
    assert_eq!(out, "2,a|0");
}

/// Regression (#648): a ternary arm calling a method on an `instanceof`-narrowed receiver must
/// resolve the callee's return type from the receiver's DECLARED type. Merge typing read the
/// receiver through `codegen_repr()`, which erases `Ident|null` to `Mixed`, so the class could not
/// be named and the arm fell to the syntactic `Int` — merging to TaggedScalar and rejecting the
/// store ("local store from PHP type Str to PHP type TaggedScalar"). BufferingAuditRecorder's shape.
#[test]
fn test_ternary_arm_method_call_on_narrowed_nullable_receiver() {
    let out = compile_and_run(
        r#"<?php
interface Ident { public function subjectId(): string; }
final class U implements Ident { public function subjectId(): string { return 'u1'; } }
interface Prov { public function current(): ?Ident; }
final class P implements Prov { public function current(): ?Ident { return new U(); } }
final class N implements Prov { public function current(): ?Ident { return null; } }
function actor(Prov $p): string {
    $identity = $p->current();
    $actorId = $identity instanceof Ident ? $identity->subjectId() : null;
    return $actorId ?? '(none)';
}
echo actor(new P()), "|", actor(new N());
"#,
    );
    assert_eq!(out, "u1|(none)");
}

/// Regression (#648): the same erasure for a `$this->m()` arm — `$this` has no merge arm, so it
/// fell to the syntactic `Object("")`, whose empty class name resolves no method and dropped the
/// arm to `Int`. Merging with the null arm then rejected the store ("local store from PHP type
/// AssocArray {...} to PHP type TaggedScalar"). ContainerRouteLayoutCatalog's shape.
///
/// The final case is the CONTROL: a genuine `?int` ternary must keep the precise TaggedScalar
/// repr, so the honesty fix must not blanket-widen every merge to Mixed.
#[test]
fn test_ternary_arm_method_call_on_this_and_nullable_int_control() {
    let out = compile_and_run(
        r#"<?php
final class C {
    private function describe(string $route): array { return ['method' => 'GET', 'path' => $route]; }
    public function layoutForRoute(?string $route): ?array
    {
        return $route === null ? null : $this->describe($route);
    }
}
function pick(bool $c): ?int { return $c ? 7 : null; }
$c = new C();
$r = $c->layoutForRoute('/x');
echo $r === null ? 'null' : $r['method'] . $r['path'], "|";
echo $c->layoutForRoute(null) === null ? 'null' : 'arr', "|";
var_dump(pick(true));
var_dump(pick(false));
echo pick(false) ?? 99;
"#,
    );
    assert_eq!(out, "GET/x|null|int(7)\nNULL\n99");
}
