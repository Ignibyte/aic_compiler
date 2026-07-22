//! Purpose:
//! Home of the PHP `array_merge` builtin: its declaration, type-check hook, and lowering.
//!
//! Called from:
//! - The builtin registry (declaration), the type checker (check hook), and the EIR
//!   backend (lower hook), all via `crate::builtins::registry`.
//!
//! Key details:
//! - The PHP golden signature is `variadic(&[], "arrays")` (min=0). The legacy CHECK
//!   arm requires exactly 2 arguments. `min_args: 2, max_args: 2` reproduce that
//!   enforcement in `check_arity` only; `function_sig` and the parity gate keep the
//!   variadic shape from the golden.
//! - `check` validates that the first argument is an indexed or associative array and
//!   returns the merged result type. The return type logic mirrors the legacy checker:
//!   when the first operand is an empty array (element type `Void`), the result adopts
//!   the second operand's element type if it is a scalar-merge type.
//! - Arity is pre-validated by `check_arity`; the hook can assume exactly 2 args.

use crate::builtins::spec::BuiltinCheckCtx;
use crate::codegen::context::FunctionContext;
use crate::codegen::CodegenIrError;
use crate::errors::CompileError;
use crate::ir::Instruction;
use crate::parser::ast::ExprKind;
use crate::types::PhpType;

builtin! {
    name: "array_merge",
    area: Array,
    params: [],
    variadic: "arrays",
    returns: Mixed,
    check: check,
    lower: lower,
    summary: "Merges the elements of one or more arrays.",
    php_manual: "https://www.php.net/manual/en/function.array-merge.php",
}

/// Validates every argument is an array and returns the left-folded merged result type.
///
/// PHP's `array_merge(array ...$arrays)` accepts zero or more arrays (arity is pre-validated
/// by `check_arity` as `min 0`, unbounded). The hook infers each argument, requiring it be
/// array-typed (or `Mixed`, the runtime-trust posture), and left-folds `array_merge_return_type`
/// over the operands — reproducing the legacy 2-arg result rule for `N == 2` and extending it
/// to any N. Zero arguments yield the empty-indexed-array type.
///
/// A single trailing spread argument (`array_merge(...$xs)`) contributes its source's element
/// type: `...$xs` where `$xs: array<array<T>>` merges each inner `array<T>`, so the fold sees
/// `array<T>` (see [`array_merge_spread_contribution`]). The spread length is runtime data, so
/// the lowering folds it at runtime; here the checker only needs the merged element type.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    let mut merged: Option<PhpType> = None;
    for arg in cx.args {
        // A Mixed argument (e.g. an indexed read `$this->headers[$k]` on an `array`-hinted
        // property, or any value the checker knows only as Mixed) is coerced/enforced at runtime,
        // the same trust posture as the Mixed→narrower type-compat boundary. A statically
        // non-array, non-Mixed argument is still rejected.
        let contribution = if let ExprKind::Spread(inner) = &arg.kind {
            let spread_ty = cx.checker.infer_type(inner, cx.env)?;
            array_merge_spread_contribution(spread_ty, cx.span)?
        } else {
            let ty = cx.checker.infer_type(arg, cx.env)?;
            if !matches!(ty, PhpType::Array(_) | PhpType::AssocArray { .. } | PhpType::Mixed) {
                return Err(CompileError::new(
                    cx.span,
                    "array_merge() argument must be array",
                ));
            }
            ty
        };
        merged = Some(match merged {
            None => contribution,
            Some(acc) => array_merge_return_type(acc, contribution),
        });
    }
    Ok(merged.unwrap_or_else(|| PhpType::Array(Box::new(PhpType::Void))))
}

/// Returns the per-element merge contribution of a spread source `array_merge(...$xs)`.
///
/// `...$xs` spreads `$xs`'s elements as successive merge operands, so a source typed
/// `array<array<T>>` contributes `array<T>` (one representative inner array) to the fold.
/// A `Mixed` (or Mixed-keyed) source contributes `Mixed` — the runtime-trust posture, matching
/// a Mixed positional operand. A source that is statically a non-array, non-Mixed value is
/// rejected: it cannot be spread into array_merge.
fn array_merge_spread_contribution(
    spread_ty: PhpType,
    span: crate::span::Span,
) -> Result<PhpType, CompileError> {
    match spread_ty {
        PhpType::Array(elem) => Ok((*elem).clone()),
        PhpType::AssocArray { value, .. } => Ok((*value).clone()),
        PhpType::Mixed => Ok(PhpType::Mixed),
        _ => Err(CompileError::new(
            span,
            "array_merge() spread argument must be an array of arrays",
        )),
    }
}

/// Lowers an `array_merge` call by delegating to the shared array-merge emitter.
fn lower(ctx: &mut FunctionContext, inst: &Instruction) -> Result<(), CodegenIrError> {
    crate::codegen::lower_inst::builtins::arrays::lower_array_merge(ctx, inst)
}

/// Infers the return type for `array_merge(first, second)`.
///
/// When `first` is an empty indexed array (element type `Void`), the merged result
/// adopts `second`'s element type if it is a scalar-merge-compatible type; otherwise
/// the result keeps `first`'s type. For non-empty indexed arrays, the left operand
/// type is returned unchanged (matching legacy checker behavior).
fn array_merge_return_type(first: PhpType, second: PhpType) -> PhpType {
    match first {
        PhpType::Array(elem) if is_empty_array_element_type(elem.as_ref()) => match second {
            PhpType::Array(right) if is_scalar_merge_element_type(right.as_ref()) => {
                PhpType::Array(right)
            }
            _ => PhpType::Array(elem),
        },
        other => other,
    }
}

/// Returns true for the element sentinel used by statically empty indexed arrays.
fn is_empty_array_element_type(ty: &PhpType) -> bool {
    matches!(ty.codegen_repr(), PhpType::Void)
}

/// Returns true for element types that the scalar merge runtime helper copies safely.
fn is_scalar_merge_element_type(ty: &PhpType) -> bool {
    matches!(
        ty.codegen_repr(),
        PhpType::Int
        | PhpType::Bool
        | PhpType::False
        | PhpType::Float
        | PhpType::Callable
        | PhpType::Void
    )
}
