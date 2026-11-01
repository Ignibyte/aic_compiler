//! Purpose:
//! Verifies a `mixed` argument bound to a DECLARED `int`, `float` or `bool` parameter of a user
//! callee, or to a declared `string` one from a strict caller, the run-time check php-src
//! performs when the callee receives it.
//!
//! Called from:
//! - `crate::codegen::lower_inst::lower_instruction()` for `Op::ScalarArgumentGuard`.
//!
//! Key details:
//! - The guard only reads the value: the call's ordinary argument conversion follows it, so
//!   an accepted value is converted, warned about and deprecated exactly as before.
//! - A strict caller accepts the declared type, plus an int for `float`; `string` is checked
//!   for strict callers only. A weak caller also
//!   accepts what PHP coerces: a bool, a finite float inside the int range or an int-coercible
//!   numeric string for `int`; a bool or a numeric string for `float`; any scalar for `bool`.
//! - The operand may be a raw scalar: constant folding and counter scalarization retype it
//!   under the op, so the raw path gives each scalar the answer its runtime tag gets.
//! - Refusals match php-src's wording: a bool is named by value, an object by its runtime
//!   class, every other type by its type word, then `" given"`.

use crate::codegen::abi;
use crate::codegen::platform::Arch;
use crate::ir::{Instruction, ScalarArgumentCheck, ScalarGuardTarget};
use crate::types::PhpType;

use super::super::context::FunctionContext;
use super::enums::{emit_mixed_tag_branch, emit_string_result_to_int_checked};
use super::mixed_narrowing::{emit_float_bits_to_float_result, emit_float_result_fits_i64_or_jump};
use super::{exceptions, objects, runtime_class_messages};
use crate::codegen::{CodegenIrError, Result};

/// How a guard answers one runtime type.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Accept,
    Refuse,
    /// A weak `int` parameter takes a float only when it is finite and inside the int range.
    FloatFitsInt,
    /// A weak `int` parameter takes a string PHP's int coercion accepts.
    IntString,
    /// A weak `float` parameter takes a numeric string.
    NumericString,
}

/// Lowers `Op::ScalarArgumentGuard`.
pub(super) fn lower_scalar_argument_guard(
    ctx: &mut FunctionContext<'_>,
    inst: &Instruction,
) -> Result<()> {
    let value = *inst.operands.first().ok_or_else(|| {
        CodegenIrError::unsupported("scalar_argument_guard without operand".to_string())
    })?;
    let Some(crate::ir::Immediate::ScalarArgumentCheck(check)) = inst.immediate else {
        return Err(CodegenIrError::unsupported(
            "scalar_argument_guard without a scalar argument check".to_string(),
        ));
    };
    let prefix = ctx
        .module
        .data
        .strings
        .get(check.prefix.as_raw() as usize)
        .ok_or_else(|| CodegenIrError::missing_entry("data string", check.prefix.as_raw()))?
        .to_string();
    if ctx.value_php_type(value)?.codegen_repr() == PhpType::Mixed {
        super::call_operands::load_value_to_first_int_arg(ctx, value)?;
        emit_boxed_guard(ctx, check, &prefix);
        return Ok(());
    }
    let loaded = ctx.load_value_to_result(value)?.codegen_repr();
    emit_raw_guard(ctx, check, &loaded, &prefix)
}

/// Returns how a parameter checked by `check` answers the runtime type `tag`.
fn verdict(check: ScalarArgumentCheck, tag: u8) -> Verdict {
    let scalar = matches!(tag, 0..=3);
    match (check.target, check.strict, tag) {
        (ScalarGuardTarget::Int, _, 0) | (ScalarGuardTarget::Float, _, 0 | 2) => Verdict::Accept,
        (ScalarGuardTarget::Bool, _, 3) | (ScalarGuardTarget::Str, _, 1) => Verdict::Accept,
        (_, true, _) => Verdict::Refuse,
        (ScalarGuardTarget::Int, false, 3) | (ScalarGuardTarget::Float, false, 3) => Verdict::Accept,
        (ScalarGuardTarget::Int, false, 2) => Verdict::FloatFitsInt,
        (ScalarGuardTarget::Int, false, 1) => Verdict::IntString,
        (ScalarGuardTarget::Float, false, 1) => Verdict::NumericString,
        (ScalarGuardTarget::Bool, false, _) if scalar => Verdict::Accept,
        _ => Verdict::Refuse,
    }
}

/// Checks a boxed `Mixed` operand already loaded into the first integer argument register.
fn emit_boxed_guard(ctx: &mut FunctionContext<'_>, check: ScalarArgumentCheck, prefix: &str) {
    let accepted = ctx.next_label("scalar_argument_ok");
    let object_arm = ctx.next_label("scalar_argument_object");
    let bool_arm = ctx.next_label("scalar_argument_bool");
    let result_reg = abi::int_result_reg(ctx.emitter);
    abi::emit_call_label(ctx.emitter, "__rt_mixed_unbox");
    let tag_reg = abi::int_result_reg(ctx.emitter);
    let mut arms: Vec<(String, u8, Verdict, &str)> = Vec::new();
    for (tag, name) in objects::RUNTIME_TAGS.iter() {
        let verdict = verdict(check, *tag);
        if verdict == Verdict::Accept {
            emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &accepted);
        } else if *tag == objects::OBJECT_TAG {
            emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &object_arm);
        } else if *tag == objects::BOOL_TAG {
            emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &bool_arm);
        } else {
            let label = ctx.next_label(&format!("scalar_argument_{name}"));
            emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &label);
            arms.push((label, *tag, verdict, name));
        }
    }
    // A tag outside the table never reaches here (the unbox helper peels nested boxes).
    exceptions::emit_type_error(ctx, &format!("{prefix}mixed given"));
    let (lo_reg, hi_reg) = match ctx.emitter.target.arch {
        Arch::AArch64 => ("x1", "x2"),
        Arch::X86_64 => ("rdi", "rdx"),
    };
    for (label, _, verdict, name) in arms {
        ctx.emitter.label(&label);
        let refused = format!("{prefix}{name} given");
        match verdict {
            Verdict::FloatFitsInt => {
                let fail = ctx.next_label("scalar_argument_float_range");
                emit_float_bits_to_float_result(ctx, lo_reg);
                emit_float_result_fits_i64_or_jump(ctx, &fail);
                abi::emit_jump(ctx.emitter, &accepted);
                ctx.emitter.label(&fail);
                exceptions::emit_type_error(ctx, &refused);
            }
            Verdict::IntString | Verdict::NumericString => {
                let (string_ptr_reg, string_len_reg) = abi::string_result_regs(ctx.emitter);
                abi::emit_reg_move(ctx.emitter, string_ptr_reg, lo_reg);
                abi::emit_reg_move(ctx.emitter, string_len_reg, hi_reg);
                emit_numeric_string_check(ctx, verdict, &accepted, &refused);
            }
            Verdict::Accept | Verdict::Refuse => exceptions::emit_type_error(ctx, &refused),
        }
    }

    // php-src names a refused bool by value, never as `bool`.
    ctx.emitter.label(&bool_arm);
    let payload_reg = crate::codegen_support::mixed_unbox_payload_reg(ctx.emitter.target);
    abi::emit_reg_move(ctx.emitter, result_reg, payload_reg);
    emit_refuse_bool_by_value(ctx, prefix);

    // Every scalar target refuses an object, named by its runtime class.
    ctx.emitter.label(&object_arm);
    let payload_reg = crate::codegen_support::mixed_unbox_payload_reg(ctx.emitter.target);
    abi::emit_reg_move(ctx.emitter, result_reg, payload_reg);
    runtime_class_messages::emit_runtime_class_name_to_string_result(ctx, result_reg, "object");
    runtime_class_messages::emit_concat_static_prefix(ctx, prefix);
    runtime_class_messages::emit_concat_static_suffix(ctx, " given");
    abi::emit_call_label(ctx.emitter, "__rt_str_persist");
    exceptions::emit_type_error_from_string_result(ctx);

    ctx.emitter.label(&accepted);
}

/// Checks a string in the string-result registers and jumps to `accepted`, or throws `refused`.
fn emit_numeric_string_check(
    ctx: &mut FunctionContext<'_>,
    verdict: Verdict,
    accepted: &str,
    refused: &str,
) {
    let invalid = ctx.next_label("scalar_argument_string_invalid");
    if verdict == Verdict::IntString {
        emit_string_result_to_int_checked(ctx, &invalid);
        abi::emit_jump(ctx.emitter, accepted);
        ctx.emitter.label(&invalid);
        // The probe leaves its 16-byte string save on the stack on the refusing path.
        abi::emit_release_temporary_stack(ctx.emitter, 16);
    } else {
        // `__rt_str_to_number` reports in the int-result register whether the WHOLE string
        // is numeric, PHP whitespace allowed on either side.
        abi::emit_call_label(ctx.emitter, "__rt_str_to_number");
        abi::emit_branch_if_int_result_zero(ctx.emitter, &invalid);
        abi::emit_jump(ctx.emitter, accepted);
        ctx.emitter.label(&invalid);
    }
    exceptions::emit_type_error(ctx, refused);
}

/// Throws `"<prefix>true given"` or `"<prefix>false given"` for the bool in the int-result
/// register.
fn emit_refuse_bool_by_value(ctx: &mut FunctionContext<'_>, prefix: &str) {
    let false_arm = ctx.next_label("scalar_argument_false");
    abi::emit_branch_if_int_result_zero(ctx.emitter, &false_arm);
    exceptions::emit_type_error(ctx, &format!("{prefix}true given"));
    ctx.emitter.label(&false_arm);
    exceptions::emit_type_error(ctx, &format!("{prefix}false given"));
}

/// Checks an operand folding or scalarization left raw, already in its result registers.
fn emit_raw_guard(
    ctx: &mut FunctionContext<'_>,
    check: ScalarArgumentCheck,
    loaded: &PhpType,
    prefix: &str,
) -> Result<()> {
    let tag = match loaded {
        PhpType::Int => 0,
        PhpType::Str => 1,
        PhpType::Float => 2,
        PhpType::Bool => 3,
        PhpType::Void => 8,
        other => {
            return Err(CodegenIrError::unsupported(format!(
                "scalar_argument_guard on a raw {other:?} operand"
            )));
        }
    };
    let name = objects::RUNTIME_TAGS.iter().find(|(known, _)| *known == tag).map_or("mixed", |(_, name)| name);
    let refused = format!("{prefix}{name} given");
    let accepted = ctx.next_label("scalar_argument_raw_ok");
    match verdict(check, tag) {
        Verdict::Accept => return Ok(()),
        Verdict::Refuse if tag == objects::BOOL_TAG => emit_refuse_bool_by_value(ctx, prefix),
        Verdict::Refuse => exceptions::emit_type_error(ctx, &refused),
        Verdict::FloatFitsInt => {
            let fail = ctx.next_label("scalar_argument_raw_float_range");
            emit_float_result_fits_i64_or_jump(ctx, &fail);
            abi::emit_jump(ctx.emitter, &accepted);
            ctx.emitter.label(&fail);
            exceptions::emit_type_error(ctx, &refused);
        }
        verdict @ (Verdict::IntString | Verdict::NumericString) => {
            emit_numeric_string_check(ctx, verdict, &accepted, &refused);
        }
    }
    ctx.emitter.label(&accepted);
    Ok(())
}
