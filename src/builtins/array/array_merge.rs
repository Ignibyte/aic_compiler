//! Purpose:
//! Home of the PHP `array_merge` builtin: its single-source registry declaration and semantic target.
//!
//! Called from:
//! - Checker, EIR, optimizer, ownership, and callable consumers through `crate::builtins::registry`.
//!
//! Key details:
//! - The PHP golden signature is `variadic(&[], "arrays")` (min=0). The legacy CHECK
//!   arm requires exactly 2 arguments. `min_args: 2, max_args: 2` reproduce that
//!   enforcement in `check_arity` only; `function_sig` and the parity gate keep the
//!   variadic shape from the golden.
//! - Both operands must be arrays. A PHP array declaration keeps a boxed result that
//!   accepts packed and associative source layouts without specializing to one caller.
//! - Concrete operands retain their existing result inference and indexed fast path.
//! - Arity is pre-validated by `check_arity`; the hook can assume exactly 2 args.

use crate::builtins::spec::BuiltinCheckCtx;
use crate::errors::CompileError;
use crate::types::PhpType;

builtin! {
    contract: "array_merge",
    check: check,
    semantics: crate::builtins::semantics::runtime_fn_semantics(
        crate::ir::RuntimeFnId::ArrayMerge,
    ),
}

/// Validates both array operands and keeps the boxed result contract when either layout is dynamic.
///
/// Arity (exactly 2 args) is pre-validated by `check_arity`. The hook re-infers both
/// argument types to derive the precise result type: when the left operand is an empty
/// indexed array (element type `Void`), the result adopts the right operand's element
/// type if it is a scalar-merge-compatible type.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    let ty1 = cx.checker.infer_type(&cx.args[0], cx.env)?;
    let ty2 = cx.checker.infer_type(&cx.args[1], cx.env)?;
    if !ty1.is_php_array() && !matches!(ty1, PhpType::Array(_) | PhpType::AssocArray { .. }) {
        return Err(CompileError::new(
            cx.span,
            "array_merge() first argument must be array",
        ));
    }
    if !ty2.is_php_array() && !matches!(ty2, PhpType::Array(_) | PhpType::AssocArray { .. }) {
        return Err(CompileError::new(
            cx.span,
            "array_merge() second argument must be array",
        ));
    }
    if !takes_indexed_fast_path(&ty1, &ty2) {
        return Ok(PhpType::php_array());
    }
    Ok(array_merge_return_type(ty1, ty2))
}

/// Returns whether `array_merge(first, second)` takes the concrete indexed helper: both operands
/// packed, with pointer-sized elements of one type (or one of them statically empty). Anything
/// else (a boxed or associative operand, string elements, two element types) merges through the
/// boxed path into a boxed array, which accepts packed and associative storage alike. The checker,
/// the EIR result type and the backend decide by this one rule.
pub(crate) fn takes_indexed_fast_path(first: &PhpType, second: &PhpType) -> bool {
    let element = |ty: &PhpType| match ty.codegen_repr() {
        PhpType::Array(element) => {
            let element = element.codegen_repr();
            (matches!(
                element,
                PhpType::Int
                    | PhpType::Bool
                    | PhpType::Float
                    | PhpType::Callable
                    | PhpType::Void
                    | PhpType::Never
            ) || element.is_refcounted())
            .then_some(element)
        }
        _ => None,
    };
    let (Some(first), Some(second)) = (element(first), element(second)) else {
        return false;
    };
    first == second
        || matches!(first, PhpType::Never | PhpType::Void)
        || matches!(second, PhpType::Never | PhpType::Void)
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
