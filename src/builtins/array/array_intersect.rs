//! Purpose:
//! Home of the PHP `array_intersect` builtin: its single-source registry declaration and semantic target.
//!
//! Called from:
//! - Checker, EIR, optimizer, ownership, and callable consumers through `crate::builtins::registry`.
//!
//! Key details:
//! - The PHP golden signature is `variadic(&["array"], "arrays")` (one regular `array`
//!   param plus a variadic `arrays`). The legacy CHECK arm required exactly 2 arguments,
//!   so `min_args: 2, max_args: 2` reproduce that enforcement in `check_arity` only;
//!   `function_sig` and the parity gate keep the variadic shape from the golden.
//! - PHP compares the values as `(string) $a === (string) $b` and keeps each survivor's key.
//!   The native helpers compare raw slots (refusing strings and boxed values) and renumber
//!   their result, so `check` retargets every call on two array operands, a declared `array`
//!   included, to the key-set prelude's `__elephc_array_intersect()`, which answers a declared
//!   `array`. Without the prelude (or for a non-array operand) the legacy rule stands: the first
//!   argument must be an indexed or associative array, and the result preserves its type.

use crate::builtins::spec::BuiltinCheckCtx;
use crate::errors::CompileError;
use crate::types::PhpType;

builtin! {
    contract: "array_intersect",
    check: check,
    semantics: crate::builtins::semantics::runtime_fn_semantics(
        crate::ir::RuntimeFnId::ArrayIntersect,
    ),
}

/// Retargets a call on two arrays to the prelude, or validates the first argument is an array
/// and returns its (preserved) type.
///
/// Arity (exactly 2 args) is pre-validated by `check_arity`. The arguments are re-inferred
/// here to drive the return type; the registry already inferred every argument once for side
/// effects.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    let ty1 = cx.checker.infer_type(&cx.args[0], cx.env)?;
    let ty2 = cx.checker.infer_type(&cx.args[1], cx.env)?;
    let array = crate::key_set_prelude::is_array_operand;
    if array(&ty1) && array(&ty2) {
        let helper = crate::key_set_prelude::VALUE_INTERSECT;
        if let Some(ty) = crate::key_set_prelude::retarget(cx, helper) {
            return Ok(ty);
        }
    }
    if !matches!(ty1, PhpType::Array(_) | PhpType::AssocArray { .. }) {
        return Err(CompileError::new(
            cx.span,
            &format!("{}() first argument must be array", cx.name),
        ));
    }
    Ok(ty1)
}
