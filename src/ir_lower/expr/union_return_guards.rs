//! Purpose:
//! Checks and binds a value returned through a DECLARED union or nullable scalar type, the
//! run-time verification php-src performs at the return boundary.
//!
//! Called from:
//! - `crate::ir_lower::stmt::control_exit::lower_return()`, before the return storage coercion.
//!
//! Key details:
//! - Applies only to a declaration with a scalar member, and only when the returned value's
//!   static type does not already prove membership.
//! - A strict file accepts exact members plus the int-to-float widening; a weak file applies
//!   PHP's union coercion order (exact, int, float, string, bool) through the shared bindings.
//! - A `false` member never coerces: `array|false` refuses `true`, `0` and `""` as php-src does.
//! - A refusal throws `TypeError` "f(): Return value must be of type T, U returned", with T as
//!   php-src prints the union and U the runtime type, a bool named by its value.

use super::*;
use super::enum_argument_guards::{
    accepts_type, bind_implicit_integer, concat, runtime_debug_type, throw_message,
};

/// Verifies `value` against the declared union return type and returns the bound value.
///
/// The value is staged in a synthetic local so every predicate and conversion reads it once
/// evaluated; frame cleanup releases that local on the normal and the unwinding path. A value
/// the declaration already proves is returned untouched.
pub(in crate::ir_lower) fn bind_declared_union_return(
    ctx: &mut LoweringContext<'_, '_>,
    value: LoweredValue,
    span: Span,
) -> LoweredValue {
    let declared = ctx.return_php_type.clone();
    if !ctx.return_type_is_declared
        || !union_return_needs_check(&declared, &ctx.builder.value_php_type(value.value))
    {
        return value;
    }
    let strict = crate::source::current_strict_types();
    let name = ctx.declare_synthetic_php_local(PhpType::Mixed);
    store_value_into_temp(ctx, &name, PhpType::Mixed, value, span);
    let staged = Expr::new(ExprKind::Variable(name), span);
    let accepted = if strict {
        crate::types::param_binding::runtime_strict_param_acceptance(&declared, &staged)
    } else {
        accepts_type(&declared, &staged, span)
    }
    .expect("union_return_needs_check admits only members with an acceptance predicate");
    let accepted = lower_expr(ctx, &accepted);
    let ok = ctx.builder.create_named_block("return.union.ok", Vec::new());
    let bad = ctx.builder.create_named_block("return.union.bad", Vec::new());
    ctx.builder.terminate(Terminator::CondBr {
        cond: accepted.value, then_target: ok, then_args: Vec::new(),
        else_target: bad, else_args: Vec::new(),
    });
    ctx.builder.position_at_end(bad);
    let literal = |text: String| Expr::new(ExprKind::StringLiteral(text), span);
    let prefix = format!(
        "{}(): Return value must be of type {}, ",
        ctx.diagnostic_owner_name(),
        crate::types::diagnostic_names::php_type_name(&declared)
    );
    let message = concat(literal(prefix),
        concat(runtime_debug_type(&staged, span), literal(" returned".to_string()), span), span);
    throw_message(ctx, "TypeError", message, span);
    ctx.builder.position_at_end(ok);
    bind_implicit_integer(ctx, &declared, &staged, span);
    // A union whose only scalar is `false` (`array|false`) converts nothing: the guard
    // accepted exact members only, so the staged value returns as it is.
    let binding = crate::types::param_binding::runtime_scalar_param_binding(&declared, &staged)
        .unwrap_or(staged);
    lower_expr(ctx, &binding)
}

/// Returns whether a return of `source` through `declared` needs the run-time union check.
///
/// `declared` must pass [`PhpType::checks_scalar_union`]. The check is skipped when each member
/// of `source` is an exact member already: an `int` into `int|string` needs nothing, an `int`
/// into `float|string` must still widen.
fn union_return_needs_check(declared: &PhpType, source: &PhpType) -> bool {
    let PhpType::Union(members) = declared else {
        return false;
    };
    if !declared.checks_scalar_union() {
        return false;
    }
    let proven = |member: &PhpType| match member {
        PhpType::Int | PhpType::Float | PhpType::Str | PhpType::Bool | PhpType::False
        | PhpType::Void => members.contains(member),
        PhpType::Array(_) | PhpType::AssocArray { .. } => members.iter()
            .any(|declared| matches!(declared, PhpType::Array(_) | PhpType::AssocArray { .. })),
        PhpType::Object(_) => members.iter().any(|declared| matches!(declared, PhpType::Object(_))),
        _ => false,
    };
    match source {
        PhpType::Union(sources) => !sources.iter().all(proven),
        other => !proven(other),
    }
}
