//! Purpose:
//! Declares `__elephc_array_intersect_key()` and `__elephc_array_diff_key()`, and the value-set
//! helpers `__elephc_array_diff()`, `__elephc_array_intersect()`, `__elephc_array_unique()`,
//! `__elephc_array_diff_assoc()` and `__elephc_array_intersect_assoc()`, and the replace helpers
//! `__elephc_array_replace()`, `__elephc_array_replace_recursive()` and
//! `__elephc_array_merge_recursive()`: ordinary PHP functions
//! that take any PHP array, injected when a program calls one of the builtins they stand in for.
//!
//! Called from:
//! - `crate::pipeline::compile()` and the integration-test frontend, before name resolution.
//! - The builtin checks of `array_intersect_key`, `array_diff_key`, `array_diff`,
//!   `array_intersect`, `array_unique`, `array_diff_assoc`, `array_intersect_assoc`,
//!   `array_replace`, `array_replace_recursive` and `array_merge_recursive`, which retarget a
//!   call through [`retarget`].
//!
//! Key details:
//! - The native helpers take two hashes. A declared `array` (a class constant, a parameter, a
//!   return), a boxed value such as an `array_filter()` result, or a list reaches them in another
//!   storage shape, so the builtin check records a call-site retarget in
//!   `Checker::generic_call_sites` and lowering calls the prelude function instead, which
//!   handles every shape through ordinary PHP.
//! - The value-set builtins compare values as `(string) $a === (string) $b` and keep each
//!   survivor's key. The native `array_diff()` / `array_intersect()` helpers renumber their result
//!   and compare raw slots, and the native `_assoc` helper leaks on x86_64, so every call of these
//!   four with array operands is retargeted; `array_unique()` keeps its native helpers for the
//!   element types they compare by value.
//! - `array_replace()` and `array_replace_recursive()` go to their helpers only where the native
//!   hash helpers refuse an operand (see [`natively_hashable`]): a declared `array`, an empty
//!   array, a list of non-scalars. Every `array_merge_recursive()` call with array operands goes
//!   to its helper, since the native one nests an existing list and skips object conversion.
//! - The retarget tests the prelude's DECLARATION (`fn_decls`), not a resolved signature:
//!   signatures resolve lazily, so a function body checked before the prelude's (a parameter
//!   declared `array`) would otherwise find no helper and fall back to the refusal.
//! - Nothing in the source names the prelude functions, so the pipeline roots each one a
//!   retarget names (`retarget_roots`) before declaration reachability, as it roots generic
//!   instantiations.

mod detect;

use crate::parser::ast::Program;

/// The prelude function `array_intersect_key()` is retargeted to.
pub(crate) const INTERSECT: &str = "__elephc_array_intersect_key";
/// The prelude function `array_diff_key()` is retargeted to.
pub(crate) const DIFF: &str = "__elephc_array_diff_key";

/// The prelude function `array_diff()` is retargeted to.
pub(crate) const VALUE_DIFF: &str = "__elephc_array_diff";
/// The prelude function `array_intersect()` is retargeted to.
pub(crate) const VALUE_INTERSECT: &str = "__elephc_array_intersect";
/// The prelude function `array_unique()` is retargeted to.
pub(crate) const VALUE_UNIQUE: &str = "__elephc_array_unique";
/// The prelude function `array_diff_assoc()` is retargeted to.
pub(crate) const ASSOC_DIFF: &str = "__elephc_array_diff_assoc";
/// The prelude function `array_intersect_assoc()` is retargeted to.
pub(crate) const ASSOC_INTERSECT: &str = "__elephc_array_intersect_assoc";

/// The prelude function `array_replace()` is retargeted to.
pub(crate) const REPLACE: &str = "__elephc_array_replace";
/// The prelude function `array_replace_recursive()` is retargeted to.
pub(crate) const REPLACE_RECURSIVE: &str = "__elephc_array_replace_recursive";
/// The prelude function `array_merge_recursive()` is retargeted to.
pub(crate) const MERGE_RECURSIVE: &str = "__elephc_array_merge_recursive";

/// Every prelude function a call can be retargeted to.
const HELPERS: &[&str] = &[
    INTERSECT,
    DIFF,
    VALUE_DIFF,
    VALUE_INTERSECT,
    VALUE_UNIQUE,
    ASSOC_DIFF,
    ASSOC_INTERSECT,
    REPLACE,
    REPLACE_RECURSIVE,
    MERGE_RECURSIVE,
];

/// The prelude sources, ordinary PHP.
const SOURCES: &[&str] = &[
    include_str!("key_set_prelude/key_sets.php"),
    include_str!("key_set_prelude/value_sets.php"),
    include_str!("key_set_prelude/replace_sets.php"),
];

/// Parses the prelude declarations.
fn declarations() -> Program {
    let mut program = Program::new();
    for source in SOURCES {
        let tokens = crate::lexer::tokenize(source).expect("valid key-set prelude tokens");
        program.extend(crate::parser::parse(&tokens).expect("valid key-set prelude"));
    }
    program
}

/// Whether `ty` is an array the prelude helpers take: a list, a hash or a declared `array`.
pub(crate) fn is_array_operand(ty: &crate::types::PhpType) -> bool {
    ty.is_php_array()
        || matches!(
            ty.codegen_repr(),
            crate::types::PhpType::Array(_) | crate::types::PhpType::AssocArray { .. }
        )
}

/// Whether `ty` is an operand the native two-hash helpers take: a hash, or a list of scalars they
/// convert to one.
pub(crate) fn natively_hashable(ty: &crate::types::PhpType) -> bool {
    matches!(ty, crate::types::PhpType::AssocArray { .. }) || ty.is_scalar_indexed_array()
}

/// Whether `ty` may reach a prelude helper's `array` parameter: an array operand, or a `mixed`
/// value (an element of a heterogeneous constant, say), which the parameter checks at run time
/// with the TypeError PHP raises.
pub(crate) fn is_array_or_mixed_operand(ty: &crate::types::PhpType) -> bool {
    is_array_operand(ty) || ty.codegen_repr() == crate::types::PhpType::Mixed
}

/// Retargets the builtin call `cx` checks to the prelude function `helper`.
///
/// Returns the helper's result type, PHP's declared `array`, or `None` when the program holds no
/// such declaration (the detector did not see the call), in which case the builtin keeps its
/// native rules.
pub(crate) fn retarget(
    cx: &mut crate::builtins::spec::BuiltinCheckCtx,
    helper: &str,
) -> Option<crate::types::PhpType> {
    if !cx.checker.fn_decls.contains_key(helper) {
        return None;
    }
    let site = (cx.checker.current_loop_storage_scope.clone(), cx.span);
    cx.checker.generic_call_sites.insert(site, helper.to_string());
    Some(crate::types::PhpType::php_array())
}

/// Prepends the prelude when the program calls one of the set builtins it stands in for.
pub fn inject_if_used(
    program: Program,
    inventory: &mut crate::optimize::reachability::PreludeInventory,
) -> Program {
    if !detect::program_calls_key_set_builtin(&program) {
        return program;
    }
    let mut combined = declarations();
    inventory.record_program("key_set", &combined);
    combined.extend(program);
    combined
}

/// Returns the prelude functions some call site was retargeted to, so reachability keeps them.
pub fn retarget_roots<'a>(
    call_sites: impl IntoIterator<Item = &'a String>,
) -> impl Iterator<Item = String> {
    let mut roots: Vec<String> = call_sites
        .into_iter()
        .filter(|name| HELPERS.contains(&name.as_str()))
        .cloned()
        .collect();
    roots.sort();
    roots.dedup();
    roots.into_iter()
}
