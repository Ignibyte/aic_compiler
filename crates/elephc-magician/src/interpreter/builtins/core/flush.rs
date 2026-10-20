//! Purpose:
//! Implements PHP `flush()` for eval execution.
//!
//! Called from:
//! - `crate::interpreter::builtins::core` direct and by-value dispatch.
//!
//! Key details:
//! - Runs the compiled runtime's `flush()` through the bridge, as in AOT code: writes are
//!   unbuffered, so it only freezes the response headers and, under `--web`, sends them.
//!   `flush()` never touches the `ob_*` buffers.

use super::super::super::*;

eval_builtin! {
    contract: "flush",
    area: Core,
    direct: Core,
    values: Core,
}

/// Evaluates a direct zero-argument `flush()` call.
pub(in crate::interpreter) fn eval_builtin_flush(
    args: &[EvalExpr],
    values: &mut impl RuntimeValueOps,
) -> Result<RuntimeCellHandle, EvalStatus> {
    if !args.is_empty() {
        return Err(EvalStatus::RuntimeFatal);
    }
    values.flush()?;
    values.null()
}

/// Evaluates `flush()` from an already materialized empty argument list.
pub(in crate::interpreter) fn eval_flush_values_result(
    evaluated_args: &[RuntimeCellHandle],
    values: &mut impl RuntimeValueOps,
) -> Result<RuntimeCellHandle, EvalStatus> {
    if !evaluated_args.is_empty() {
        return Err(EvalStatus::RuntimeFatal);
    }
    values.flush()?;
    values.null()
}
