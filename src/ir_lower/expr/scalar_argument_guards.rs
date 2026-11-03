//! Purpose:
//! Checks and binds a `mixed` argument passed to a DECLARED scalar or scalar-union parameter of
//! a user function, method or constructor, the run-time verification php-src performs when the
//! callee receives it.
//!
//! Called from:
//! - `coerce_operands_to_params()` in the sibling call argument slice, after every argument is
//!   evaluated, in parameter order.
//! - `coerce_scalar_arg_to_param_storage()`, which leaves a checked argument unconverted.
//!
//! Key details:
//! - Only a user callee named on the call's argument scope is checked; builtin, extern and
//!   dynamic calls keep their own binding rules.
//! - The caller's file decides strict or weak binding, as in php-src.
//! - A method is named only when exactly one implementation can run: an override may widen a
//!   parameter and php-src names the class whose body runs, which the caller cannot know. When
//!   an override binds a parameter in other storage, the sibling `dispatched_argument_guards`
//!   checks that argument by each implementation in turn.
//! - A refusal throws `TypeError` "f(): Argument #N ($x) must be of type T, U given", without
//!   php's "called in" clause, like the other argument guards.

use super::*;
use super::union_return_guards::{bind_checked_scalar_boundary, union_boundary_needs_check};
use crate::ir::{ScalarArgumentCheck, ScalarGuardTarget};
use crate::ir_lower::context::ArgumentCallee;

/// Returns the declared type parameter `index` checks `source` against, or `None` when the
/// argument needs no run-time check.
pub(super) fn checked_scalar_parameter(
    sig: &FunctionSig,
    index: usize,
    source: &PhpType,
) -> Option<PhpType> {
    if !sig.declared_params.get(index).copied().unwrap_or(false)
        || sig.ref_params.get(index).copied().unwrap_or(false)
    {
        return None;
    }
    let (_, declared) = sig.params.get(index)?;
    let needs_check = match declared {
        PhpType::Int | PhpType::Float | PhpType::Str | PhpType::Bool => {
            matches!(source.codegen_repr(), PhpType::Mixed | PhpType::Union(_))
        }
        PhpType::Union(_) if declared.checks_scalar_union() => {
            union_boundary_needs_check(declared, source)
        }
        _ => false,
    };
    needs_check.then(|| declared.clone())
}

/// Checks the argument `value` against `declared`, and returns the bound value when the check
/// converts it, or `None` when the call's ordinary argument conversion follows.
///
/// A declared `int`, `float` or `bool`, and a strict caller's `string`, take
/// `Op::ScalarArgumentGuard`, one instruction that only reads the value: a loop counter the
/// optimizer keeps unboxed stays unboxed. A weak caller's `string` or a union stages the value
/// and binds it through PHP's coercion order.
pub(super) fn bind_scalar_argument(
    ctx: &mut LoweringContext<'_, '_>,
    callee: &ArgumentCallee,
    sig: &FunctionSig,
    index: usize,
    declared: &PhpType,
    value: crate::ir::ValueId,
) -> Option<LoweredValue> {
    let position = sig.params.iter().take(index).filter(|(name, _)| {
        name != crate::func_args::HIDDEN_ARGC_PARAM && name != crate::func_args::HIDDEN_ARGS_PARAM
    }).count() + 1;
    let prefix = format!(
        "{}(): Argument #{position} (${}) must be of type {}, ",
        callee.display,
        sig.params[index].0,
        crate::types::diagnostic_names::php_type_name(declared)
    );
    let target = match declared {
        PhpType::Int => Some(ScalarGuardTarget::Int),
        PhpType::Float => Some(ScalarGuardTarget::Float),
        PhpType::Bool => Some(ScalarGuardTarget::Bool),
        // A strict caller's `string` takes a string only, so the read-only guard covers it and
        // the value needs no staging: the staged binding lost response headers on the compiled
        // AIC site (AIC #223). A weak caller also accepts a `Stringable` object, so it stages.
        PhpType::Str if crate::source::current_strict_types() => Some(ScalarGuardTarget::Str),
        _ => None,
    };
    let Some(target) = target else {
        let lowered = lowered_value_from_id(ctx, value);
        return Some(bind_checked_scalar_boundary(
            ctx, declared, lowered, prefix, " given", Span::dummy(),
        ));
    };
    let check = ScalarArgumentCheck {
        prefix: ctx.intern_string(&prefix),
        target,
        strict: crate::source::current_strict_types(),
    };
    ctx.emit_void(
        Op::ScalarArgumentGuard,
        vec![value],
        Some(Immediate::ScalarArgumentCheck(check)),
        Op::ScalarArgumentGuard.default_effects(),
        None,
    );
    None
}

/// Names a user function callee.
pub(super) fn function_callee(name: &str) -> ArgumentCallee {
    let name = name.split('<').next().unwrap_or(name).trim_start_matches('\\');
    ArgumentCallee { display: name.to_string() }
}

/// Names the user function or closure a statically resolved callable runs, or `None` for a
/// closure with no recorded label.
pub(super) fn callable_function_callee(
    ctx: &LoweringContext<'_, '_>,
    function_name: &str,
) -> Option<ArgumentCallee> {
    if function_name.starts_with("__eir_closure_") {
        return ctx.closure_display_name(function_name)
            .map(|display| ArgumentCallee { display });
    }
    Some(function_callee(function_name))
}

/// Names a user method callee, or `None` when no single implementation is known.
///
/// `late_bound` is true for an instance call and for `static::`: the receiver class and every
/// concrete descendant (every concrete class declaring the method, for an interface) can run,
/// and the callee is named only when they all run the same body. `self::`, `parent::` and a
/// named class run the receiver's own implementation.
pub(super) fn method_callee(
    ctx: &LoweringContext<'_, '_>,
    receiver: &str,
    method: &str,
    is_static: bool,
    late_bound: bool,
) -> Option<ArgumentCallee> {
    let receiver = receiver.trim_start_matches('\\');
    let key = php_symbol_key(method);
    let implementation = |info: &crate::types::ClassInfo, name: &str| {
        let (methods, impls) = if is_static {
            (&info.static_methods, &info.static_method_impl_classes)
        } else {
            (&info.methods, &info.method_impl_classes)
        };
        methods.contains_key(&key)
            .then(|| impls.get(&key).cloned().unwrap_or_else(|| name.to_string()))
    };
    let class_receiver = ctx.classes.get(receiver);
    if class_receiver.is_none() && !ctx.interfaces.contains_key(receiver) {
        return None;
    }
    let implementations = if late_bound {
        let mut found = ctx.classes.iter()
            .filter(|(name, info)| !info.is_abstract && match class_receiver {
                Some(_) => name.as_str() == receiver
                    || crate::types::class_inherits_from(ctx.classes, name, receiver),
                None => true,
            })
            .filter_map(|(name, info)| implementation(info, name))
            .collect::<Vec<_>>();
        found.sort();
        found.dedup();
        found
    } else {
        class_receiver.and_then(|info| implementation(info, receiver)).into_iter().collect()
    };
    let [class] = implementations.as_slice() else {
        return None;
    };
    implementation_callee(ctx, class, method)
}

/// Names the user method `class` implements, or `None` for an internal class.
pub(super) fn implementation_callee(
    ctx: &LoweringContext<'_, '_>,
    class: &str,
    method: &str,
) -> Option<ArgumentCallee> {
    let key = php_symbol_key(method);
    let info = ctx.classes.get(class)?;
    if info.is_internal {
        return None;
    }
    let declared = info.method_decls.iter()
        .find(|decl| php_symbol_key(&decl.name) == key)
        .map_or(method, |decl| decl.name.as_str());
    Some(ArgumentCallee { display: format!("{class}::{declared}") })
}

/// Names the callee of an instance method call on `object`, unless it dispatches through
/// `__call`, targets an enum (whose arguments have their own guards) or has no single
/// implementation.
pub(super) fn name_method_argument_callee(
    ctx: &mut LoweringContext<'_, '_>,
    object: crate::ir::ValueId,
    dispatch_method: &str,
    method: &str,
) {
    if dispatch_method != method {
        return;
    }
    let object_ty = ctx.builder.value_php_type(object);
    let Some((class, _)) = singular_object_class(&object_ty) else {
        return;
    };
    if ctx.enums.contains_key(class.trim_start_matches('\\')) {
        return;
    }
    if let Some(callee) = method_callee(ctx, class, method, false, true) {
        name_argument_callee(ctx, callee);
    } else if let Some(dispatch) =
        super::dispatched_argument_guards::method_argument_dispatch(ctx, object, class, method)
    {
        name_argument_dispatch(ctx, dispatch);
    }
}
