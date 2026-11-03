//! Purpose:
//! Checks and binds the arguments of an instance call whose overrides bind a parameter in other
//! storage than the receiver's declared signature, by the implementation the receiver's
//! runtime class runs.
//!
//! Called from:
//! - `name_method_argument_callee()` in the sibling scalar argument slice, which records the
//!   dispatch when no single implementation can be named.
//! - `coerce_operands_to_params()` in the sibling call argument slice, in parameter order.
//!
//! Key details:
//! - PHP checks an argument against the parameters of the body that runs, so a widened
//!   override (`int` to `int|string`) accepts what its parent refuses, and refuses what neither
//!   accepts naming its own class.
//! - Each concrete class with such an override is one `instanceof` test, most derived first.
//!   The other classes run the fallback when they share one implementation, and are tested
//!   one by one too when they do not.
//! - The bound argument reaches the call boxed as `mixed`, and codegen's runtime-class
//!   dispatch hands each implementation its own storage.

use super::*;
use super::scalar_argument_guards::{bind_scalar_argument, checked_scalar_parameter, implementation_callee};
use crate::ir_lower::context::{ArgumentDispatch, ArgumentDispatchBranch};

/// Returns the runtime-class dispatch of an instance call of `method` on `receiver` of declared
/// class or interface `class`, or `None` when every override binds the declared storage.
pub(super) fn method_argument_dispatch(
    ctx: &LoweringContext<'_, '_>,
    receiver: crate::ir::ValueId,
    class: &str,
    method: &str,
) -> Option<ArgumentDispatch> {
    let class = class.trim_start_matches('\\');
    let key = php_symbol_key(method);
    let overrides = crate::types::diverging_overrides(ctx.classes, ctx.interfaces, class, &key);
    if overrides.is_empty() {
        return None;
    }
    let mut params = overrides
        .iter()
        .flat_map(|diverging| diverging.params.iter().copied())
        .collect::<Vec<_>>();
    params.sort_unstable();
    params.dedup();
    let mut branches = Vec::with_capacity(overrides.len());
    for diverging in &overrides {
        branches.push(implementation_branch(ctx, &diverging.class_name, method)?);
    }
    let others = other_classes(ctx, class, &key, &overrides);
    let mut implementations = others
        .iter()
        .map(|name| {
            ctx.classes[name].method_impl_classes.get(&key).cloned().unwrap_or_else(|| name.clone())
        })
        .collect::<Vec<_>>();
    implementations.sort();
    implementations.dedup();
    let fallback = match implementations.as_slice() {
        [implementation] => Some(implementation_branch(ctx, implementation, method)?),
        _ => {
            // Several bodies run for the other classes: test each class too, so every body
            // checks by its own declaration. A class only `eval` declares stays unchecked.
            for name in &others {
                branches.push(implementation_branch(ctx, name, method)?);
            }
            None
        }
    };
    let fallback = fallback.filter(|_| !ctx.has_eval_barrier());
    branches.sort_by_key(|branch| std::cmp::Reverse(class_depth(ctx, &branch.class_name)));
    Some(ArgumentDispatch { receiver, params, branches, fallback })
}

/// Returns argument `index` checked and bound by the implementation the receiver's runtime
/// class runs, boxed as `mixed`.
///
/// The bound value is staged in a synthetic local each branch stores; frame cleanup releases it
/// on the normal and the unwinding path, as for the other checked boundaries.
pub(super) fn bind_dispatched_argument(
    ctx: &mut LoweringContext<'_, '_>,
    dispatch: &ArgumentDispatch,
    index: usize,
    value: crate::ir::ValueId,
) -> LoweredValue {
    let source = ctx.builder.value_php_type(value);
    let staged = ctx.declare_synthetic_php_local(PhpType::Mixed);
    let merge = ctx.builder.create_named_block("dispatch.argument.bound", Vec::new());
    for branch in &dispatch.branches {
        let class_data = ctx.intern_class_name(&branch.class_name);
        let selected = ctx.emit_value(
            Op::InstanceOf,
            vec![dispatch.receiver],
            Some(Immediate::Data(class_data)),
            PhpType::Bool,
            Op::InstanceOf.default_effects(),
            None,
        );
        let override_block = ctx.builder.create_named_block("dispatch.argument.override", Vec::new());
        let next = ctx.builder.create_named_block("dispatch.argument.next", Vec::new());
        ctx.builder.terminate(Terminator::CondBr {
            cond: selected.value,
            then_target: override_block,
            then_args: Vec::new(),
            else_target: next,
            else_args: Vec::new(),
        });
        ctx.builder.position_at_end(override_block);
        stage_branch_argument(ctx, Some(branch), index, value, &source, &staged);
        branch_to(ctx, merge);
        ctx.builder.position_at_end(next);
    }
    stage_branch_argument(ctx, dispatch.fallback.as_ref(), index, value, &source, &staged);
    branch_to(ctx, merge);
    ctx.builder.position_at_end(merge);
    lower_expr(ctx, &Expr::new(ExprKind::Variable(staged), Span::dummy()))
}

/// Returns whether some implementation of `dispatch` checks argument `index` of `source` type.
pub(super) fn dispatched_argument_needs_check(
    dispatch: &ArgumentDispatch,
    index: usize,
    source: &PhpType,
) -> bool {
    dispatch.params.contains(&index)
        && dispatch
            .branches
            .iter()
            .chain(dispatch.fallback.as_ref())
            .any(|branch| checked_scalar_parameter(&branch.sig, index, source).is_some())
}

/// Stores argument `index`, checked and bound by `branch` when it declares a checked type, into
/// the staging local `staged`.
fn stage_branch_argument(
    ctx: &mut LoweringContext<'_, '_>,
    branch: Option<&ArgumentDispatchBranch>,
    index: usize,
    value: crate::ir::ValueId,
    source: &PhpType,
    staged: &str,
) {
    let bound = branch.and_then(|branch| {
        let declared = checked_scalar_parameter(&branch.sig, index, source)?;
        bind_scalar_argument(ctx, &branch.callee, &branch.sig, index, &declared, value)
    });
    let bound = bound.unwrap_or_else(|| lowered_value_from_id(ctx, value));
    store_value_into_temp(ctx, staged, PhpType::Mixed, bound, Span::dummy());
}

/// Returns the branch for concrete class `class_name`, which runs its own `method`.
fn implementation_branch(
    ctx: &LoweringContext<'_, '_>,
    class_name: &str,
    method: &str,
) -> Option<ArgumentDispatchBranch> {
    let key = php_symbol_key(method);
    let info = ctx.classes.get(class_name)?;
    let sig = info.methods.get(&key)?.clone();
    let implementation = info
        .method_impl_classes
        .get(&key)
        .map_or(class_name, String::as_str);
    Some(ArgumentDispatchBranch {
        class_name: class_name.to_string(),
        callee: implementation_callee(ctx, implementation, method)?,
        sig,
    })
}

/// Returns the concrete classes reachable from `receiver` that declare `key` without a
/// diverging override.
fn other_classes(
    ctx: &LoweringContext<'_, '_>,
    receiver: &str,
    key: &str,
    overrides: &[crate::types::DivergingOverride],
) -> Vec<String> {
    let is_interface = !ctx.classes.contains_key(receiver);
    let mut others = ctx
        .classes
        .iter()
        .filter(|(name, info)| {
            !info.is_abstract
                && info.methods.contains_key(key)
                && !overrides.iter().any(|diverging| &diverging.class_name == *name)
                && if is_interface {
                    info.interfaces.iter().any(|interface| interface == receiver)
                } else {
                    name.as_str() == receiver
                        || crate::types::class_inherits_from(ctx.classes, name, receiver)
                }
        })
        .map(|(name, _)| name.clone())
        .collect::<Vec<_>>();
    others.sort();
    others
}

/// Returns how many ancestors `class_name` has.
fn class_depth(ctx: &LoweringContext<'_, '_>, class_name: &str) -> usize {
    let mut depth = 0;
    let mut current = ctx.classes.get(class_name).and_then(|info| info.parent.as_deref());
    while let Some(parent) = current {
        depth += 1;
        if depth > ctx.classes.len() {
            break;
        }
        current = ctx.classes.get(parent).and_then(|info| info.parent.as_deref());
    }
    depth
}
