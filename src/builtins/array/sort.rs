//! Purpose:
//! Home of the PHP `sort` builtin: its declaration, type-check hook, and lowering.
//!
//! Called from:
//! - The builtin registry (declaration), the type checker (check hook), and the EIR
//!   backend (lower hook), all via `crate::builtins::registry`.
//!
//! Key details:
//! - The golden signature is `first_param_ref(fixed(["array"]))`: exactly 1 argument,
//!   the `array` param is by-reference. The `ref` marker is mandatory — it is what makes
//!   by-reference mutation lower correctly (ir_lower reads `ref_params` from the registry sig).
//! - `check` requires the argument be an Array or AssocArray, returning Void.
//! - `lower` is a thin wrapper over the shared `arrays::lower_sort` emitter.

use crate::builtins::spec::BuiltinCheckCtx;
use crate::codegen::context::FunctionContext;
use crate::codegen::CodegenIrError;
use crate::errors::CompileError;
use crate::ir::Instruction;
use crate::types::PhpType;

builtin! {
    name: "sort",
    area: Array,
    params: [ref array: Mixed],
    min_args: 1,
    max_args: 2,
    returns: Void,
    check: check,
    lower: lower,
    summary: "Sorts an array in ascending order.",
    php_manual: "https://www.php.net/manual/en/function.sort.php",
}

/// Validates the argument types for a `sort` call.
///
/// Requires the first argument be an indexed or associative array. The optional second
/// `$flags` argument (a sort-comparator selector, default `SORT_REGULAR`) is pre-typed as
/// `Int` by the registry; its supported values are enforced at lowering. Arity (1 or 2) is
/// pre-validated by the registry. Returns `Ok(PhpType::Void)` on success.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    let ty = cx.checker.infer_type(&cx.args[0], cx.env)?;
    if !matches!(ty, PhpType::Array(_) | PhpType::AssocArray { .. }) {
        return Err(CompileError::new(cx.span, &format!("{}() argument must be array", cx.name)));
    }
    if let Some(flags_arg) = cx.args.get(1) {
        cx.checker.infer_type(flags_arg, cx.env)?;
    }
    Ok(PhpType::Void)
}

/// Lowers a `sort` call by dispatching to the shared array emitter.
fn lower(ctx: &mut FunctionContext, inst: &Instruction) -> Result<(), CodegenIrError> {
    crate::codegen::lower_inst::builtins::arrays::lower_sort(ctx, inst)
}
