//! Purpose:
//! Home of the PHP `array_diff_key` builtin: its single-source registry declaration and semantic target.
//!
//! Called from:
//! - Checker, EIR, optimizer, ownership, and callable consumers through `crate::builtins::registry`.
//!
//! Key details:
//! - The PHP golden signature is `variadic(&["array"], "arrays")` (one regular `array`
//!   param plus a variadic `arrays`). The legacy CHECK arm required exactly 2 arguments,
//!   so `min_args: 2, max_args: 2` reproduce that enforcement in `check_arity` only;
//!   `function_sig` and the parity gate keep the variadic shape from the golden.
//! - `check` reproduces the legacy rule: the first argument must be an indexed or
//!   associative array, and the result preserves that first-operand type. A check hook
//!   is required because the return type depends on the inferred first-argument type.

use crate::builtins::spec::BuiltinCheckCtx;
use crate::errors::CompileError;
use crate::types::PhpType;

builtin! {
    contract: "array_diff_key",
    check: check,
    semantics: crate::builtins::semantics::runtime_fn_semantics(
        crate::ir::RuntimeFnId::ArrayDiffKey,
    ),
}

/// Validates the first argument is an array and returns its (preserved) type.
///
/// Arity (exactly 2 args) is pre-validated by `check_arity`. The first argument is
/// re-inferred here to drive the return type; the registry already inferred every
/// argument once for side effects. The result preserves the first-operand array shape.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    let ty1 = cx.checker.infer_type(&cx.args[0], cx.env)?;
    let ty2 = cx.checker.infer_type(&cx.args[1], cx.env)?;
    // Two concrete hashes take the native helper. Any other array operand (a declared `array`, a
    // boxed `array_filter()` result, a list) retargets the call to the key-set prelude, which
    // takes any PHP array and answers a declared `array`.
    let hash = |ty: &PhpType| matches!(ty.codegen_repr(), PhpType::AssocArray { .. });
    let array = |ty: &PhpType| {
        ty.is_php_array() || matches!(ty.codegen_repr(), PhpType::Array(_) | PhpType::AssocArray { .. })
    };
    if !(hash(&ty1) && hash(&ty2))
        && array(&ty1)
        && array(&ty2)
        && cx.checker.functions.contains_key(crate::key_set_prelude::DIFF)
    {
        let site = (cx.checker.current_loop_storage_scope.clone(), cx.span);
        cx.checker
            .generic_call_sites
            .insert(site, crate::key_set_prelude::DIFF.to_string());
        return Ok(PhpType::php_array());
    }
    if !matches!(ty1, PhpType::Array(_) | PhpType::AssocArray { .. }) {
        return Err(CompileError::new(
            cx.span,
            &format!("{}() first argument must be array", cx.name),
        ));
    }
    Ok(ty1)
}
