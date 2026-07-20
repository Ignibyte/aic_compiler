//! Purpose:
//! Home of the PHP `array_filter` builtin: its declaration, type-check hook, and lowering.
//!
//! Called from:
//! - The builtin registry (declaration), the type checker (check hook), and the EIR
//!   backend (lower hook), all via `crate::builtins::registry`.
//!
//! Key details:
//! - The PHP golden signature is `optional(&["array","callback","mode"], 1, &[null, 0])`.
//!   The legacy CHECK arm required 2 or 3 arguments (`args.len() < 2 || args.len() > 3`),
//!   so `min_args: 2` reproduces that enforcement in `check_arity`; the derived max of 3
//!   from the optional signature already matches.
//! - `check` validates the first argument is an indexed array, builds callback dummy args
//!   based on the static mode value, and validates the callback signature. The return type
//!   preserves the input array element type.
//! - `lower` is a thin wrapper over the shared `arrays::lower_array_filter` emitter.

use crate::builtins::spec::{BuiltinCheckCtx, DefaultSpec};
use crate::codegen::context::FunctionContext;
use crate::codegen::CodegenIrError;
use crate::errors::CompileError;
use crate::ir::Instruction;
use crate::types::PhpType;

builtin! {
    name: "array_filter",
    area: Array,
    params: [array: Mixed, callback: Mixed = DefaultSpec::Null, mode: Mixed = DefaultSpec::Int(0)],
    min_args: 2,
    returns: Mixed,
    check: check,
    lower: lower,
    summary: "Filters elements of an array using a callback function.",
    php_manual: "https://www.php.net/manual/en/function.array-filter.php",
}

/// Returns the filtered array type for an `array_filter` call.
///
/// Validates the first argument is an indexed array, builds the callback dummy args
/// based on the optional mode argument, and validates the callback. Arity (2 or 3 args)
/// is pre-validated by `check_arity`.
fn check(cx: &mut BuiltinCheckCtx) -> Result<PhpType, CompileError> {
    for arg in cx.args {
        cx.checker.infer_type(arg, cx.env)?;
    }
    let arr_ty = cx.checker.infer_type(&cx.args[0], cx.env)?;
    match arr_ty {
        // A concrete indexed array. Its keys are genuinely integer, so validate the callback with
        // the strict positional-key dummies — a `fn (string $key)` USE_KEY predicate really is a
        // mismatch here, and (crucially) the indexed filter runtime cannot walk a hash, so this arm
        // must not be relaxed to admit possibly-hash sources.
        PhpType::Array(elem_ty) => {
            let arr_ty = PhpType::Array(elem_ty.clone());
            let dummy_args = crate::types::checker::builtins::array_filter_callback_dummy_args(
                &arr_ty,
                cx.args.get(2),
                cx.span,
            );
            crate::types::checker::builtins::check_callback_builtin_call(
                cx.checker,
                &cx.args[1],
                &dummy_args,
                cx.span,
                cx.env,
                "array_filter() callback",
            )?;
            Ok(PhpType::Array(elem_ty))
        }
        // A Mixed/Union source (a bare `array` value the checker could not type as a concrete
        // array). The filtered result is Array(Mixed); the backend routes it through the Mixed-source
        // unbox path (which loudly aborts on a runtime hash rather than mis-filtering). Validate the
        // value-mode callback against a Mixed element via the object-aware synthetic binding.
        PhpType::Mixed | PhpType::Union(_) => {
            let (dummy_arg, elem_binding) =
                crate::types::checker::builtins::comparator_dummy_arg_for_elem(
                    &PhpType::Mixed,
                    cx.span,
                );
            let dummy_args = vec![dummy_arg];
            let mut env_with_elem;
            let cb_env: &crate::types::TypeEnv = match &elem_binding {
                Some((binding_name, binding_ty)) => {
                    env_with_elem = cx.env.clone();
                    env_with_elem.insert(binding_name.clone(), binding_ty.clone());
                    &env_with_elem
                }
                None => cx.env,
            };
            crate::types::checker::builtins::check_callback_builtin_call(
                cx.checker,
                &cx.args[1],
                &dummy_args,
                cx.span,
                cb_env,
                "array_filter() callback",
            )?;
            Ok(PhpType::Array(Box::new(PhpType::Mixed)))
        }
        // An associative-array (hash) source — a bare `array` with possibly-string keys. Preserve
        // the associative result type; the backend walks it via `__rt_hash_filter`, which handles
        // the hash layout and preserves keys.
        PhpType::AssocArray { key, value } => {
            validate_filter_callback(cx, value.as_ref())?;
            Ok(PhpType::AssocArray { key, value })
        }
        _ => Err(CompileError::new(
            cx.span,
            "array_filter() first argument must be array",
        )),
    }
}

/// Validates an `array_filter` predicate against mode-aware dummy arguments. The value (USE_VALUE /
/// USE_BOTH) argument uses the source's element/value type; the key (USE_KEY / USE_BOTH) argument
/// uses `Mixed`, because a PHP array key is int-or-string at the runtime boundary — a typed
/// `fn (string $key)` must not be rejected "got Int" on an array the compiler could only type as
/// `array<int, …>` (phpdoc generics are discarded). Synthetic-variable bindings (from
/// `comparator_dummy_arg_for_elem`) let a typed parameter validate against these non-literal types.
fn validate_filter_callback(
    cx: &mut BuiltinCheckCtx,
    value_ty: &PhpType,
) -> Result<(), CompileError> {
    use crate::types::checker::builtins::{
        comparator_dummy_arg_for_elem, static_array_filter_mode_value,
    };
    let mode = cx.args.get(2).and_then(static_array_filter_mode_value);
    let (dummy_args, bindings) = match mode {
        // ARRAY_FILTER_USE_KEY: the predicate receives only the (Mixed) key.
        Some(2) => {
            let (arg, bind) = comparator_dummy_arg_for_elem(&PhpType::Mixed, cx.span);
            (vec![arg], vec![bind])
        }
        // ARRAY_FILTER_USE_BOTH: the predicate receives (value, Mixed key).
        Some(1) => {
            let (value_arg, value_bind) = comparator_dummy_arg_for_elem(value_ty, cx.span);
            let (key_arg, key_bind) = comparator_dummy_arg_for_elem(&PhpType::Mixed, cx.span);
            (vec![value_arg, key_arg], vec![value_bind, key_bind])
        }
        // ARRAY_FILTER_USE_VALUE (default): the predicate receives only the value.
        _ => {
            let (arg, bind) = comparator_dummy_arg_for_elem(value_ty, cx.span);
            (vec![arg], vec![bind])
        }
    };
    let mut env_with_bindings;
    let cb_env: &crate::types::TypeEnv = if bindings.iter().any(Option::is_some) {
        env_with_bindings = cx.env.clone();
        for (binding_name, binding_ty) in bindings.iter().flatten() {
            env_with_bindings.insert(binding_name.clone(), binding_ty.clone());
        }
        &env_with_bindings
    } else {
        cx.env
    };
    crate::types::checker::builtins::check_callback_builtin_call(
        cx.checker,
        &cx.args[1],
        &dummy_args,
        cx.span,
        cb_env,
        "array_filter() callback",
    )?;
    Ok(())
}

/// Lowers an `array_filter` call by dispatching to the shared array emitter.
fn lower(ctx: &mut FunctionContext, inst: &Instruction) -> Result<(), CodegenIrError> {
    crate::codegen::lower_inst::builtins::arrays::lower_array_filter(ctx, inst)
}
