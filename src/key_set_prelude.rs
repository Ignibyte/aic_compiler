//! Purpose:
//! Declares `__elephc_array_intersect_key()` and `__elephc_array_diff_key()`, ordinary PHP
//! functions that take any PHP array, and injects them when a program calls
//! `array_intersect_key()` or `array_diff_key()`.
//!
//! Called from:
//! - `crate::pipeline::compile()` and the integration-test frontend, before name resolution.
//! - `crate::builtins::array::array_intersect_key` and `array_diff_key`, which name the helpers
//!   a call is retargeted to.
//!
//! Key details:
//! - The native helpers take two hashes. A declared `array` (a class constant, a parameter, a
//!   return), a boxed value such as an `array_filter()` result, or a list reaches them in another
//!   storage shape, so the builtin check records a call-site retarget in
//!   `Checker::generic_call_sites` and lowering calls the prelude function instead, which
//!   handles every shape through ordinary PHP.
//! - Nothing in the source names the prelude functions, so the pipeline roots each one a
//!   retarget names (`retarget_roots`) before declaration reachability, as it roots generic
//!   instantiations.

mod detect;

use crate::parser::ast::Program;

/// The prelude function `array_intersect_key()` is retargeted to.
pub(crate) const INTERSECT: &str = "__elephc_array_intersect_key";
/// The prelude function `array_diff_key()` is retargeted to.
pub(crate) const DIFF: &str = "__elephc_array_diff_key";

/// The prelude source, ordinary PHP.
const SOURCE: &str = include_str!("key_set_prelude/key_sets.php");

/// Parses the prelude declarations.
fn declarations() -> Program {
    let tokens = crate::lexer::tokenize(SOURCE).expect("valid key-set prelude tokens");
    crate::parser::parse(&tokens).expect("valid key-set prelude")
}

/// Prepends the prelude when the program calls `array_intersect_key()` or `array_diff_key()`.
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
        .filter(|name| name.as_str() == INTERSECT || name.as_str() == DIFF)
        .cloned()
        .collect();
    roots.sort();
    roots.dedup();
    roots.into_iter()
}
