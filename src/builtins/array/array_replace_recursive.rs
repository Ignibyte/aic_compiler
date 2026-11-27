//! Purpose:
//! Home of the PHP `array_replace_recursive` builtin: its single-source registry declaration and semantic target.
//!
//! Called from:
//! - Checker, EIR, optimizer, ownership, and callable consumers through `crate::builtins::registry`.
//!
//! Key details:
//! - The PHP golden signature is `fixed(&["array", "replacements"])` (two required
//!   params, no variadic), matching the registry signature. The
//!   param-derived bounds already require exactly 2 arguments, so no `min_args`/
//!   `max_args` override is needed; `check_arity` owns the arity contract.
//! - `check` sends a declared `array`, an empty array or a list of non-scalars to the
//!   key-set prelude's PHP helper; otherwise both arguments must be associative arrays or
//!   indexed arrays of scalars, and the result is the two-input hash result type. A
//!   check hook is required because the return type depends on the inferred arguments.

use crate::builtins::spec::BuiltinCheckCtx;
use crate::errors::CompileError;
use crate::types::PhpType;

builtin! {
    contract: "array_replace_recursive",
    check: check,
    semantics: crate::builtins::semantics::runtime_fn_semantics(
        crate::ir::RuntimeFnId::ArrayReplaceRecursive,
    ),
}

/// Validates both arguments are hash-compatible arrays and returns the merged hash type.
///
/// Arity (exactly 2 args) is pre-validated by `check_arity`. Both arguments are
/// re-inferred here to drive the return type; the registry already inferred every
/// argument once for side effects. Each operand must be an associative array or an
/// indexed array of scalars; the result widens key/value to `Mixed` when the operands
/// disagree, via `PhpType::two_input_hash_result`.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    let ty1 = cx.checker.infer_type(&cx.args[0], cx.env)?;
    let ty2 = cx.checker.infer_type(&cx.args[1], cx.env)?;
    let accepted = crate::key_set_prelude::natively_hashable;
    let array = crate::key_set_prelude::is_array_or_mixed_operand;
    if !(accepted(&ty1) && accepted(&ty2)) && array(&ty1) && array(&ty2) {
        if let Some(ty) = crate::key_set_prelude::retarget(cx, crate::key_set_prelude::REPLACE_RECURSIVE) {
            return Ok(ty);
        }
    }
    if !accepted(&ty1) || !accepted(&ty2) {
        return Err(CompileError::new(
            cx.span,
            &format!(
                "{}() arguments must be associative arrays or indexed arrays of scalars",
                cx.name
            ),
        ));
    }
    Ok(PhpType::two_input_hash_result(&ty1, &ty2))
}
