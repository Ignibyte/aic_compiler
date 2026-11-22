//! Purpose:
//! Verifies a boxed `Mixed` value at a DECLARED `array`, `?array` or class-typed parameter or
//! return boundary, the run-time check php-src performs where the static type is only `mixed`.
//!
//! Called from:
//! - `emit_literal_declared_parameter_guards()` in the sibling direct-call slice, before the
//!   call's arguments are materialized.
//! - `crate::codegen::lower_inst::lower_instruction()` for `Op::ReturnBoundaryMixedGuard`.
//!
//! Key details:
//! - The guard only reads the boxed value. Accepted values keep their ordinary path: an array or
//!   nullable boundary keeps the cell, an exact class unboxes it afterwards.
//! - The declared type is known at compile time, so the verdict for each runtime tag is decided
//!   here and the emitted code is a plain tag dispatch. An object is decided by its runtime class
//!   id against every class in the module that satisfies the declaration.
//! - Refusals match php-src's wording: a bool is named by value (`true given`), an object by its
//!   runtime class (`stdClass given`), every other type by its type word.

use crate::codegen::abi;
use crate::ir::{Instruction, ValueId};
use crate::types::MixedBoundary;

use super::super::context::FunctionContext;
use super::{exceptions, objects, runtime_class_messages};
use crate::codegen::{CodegenIrError, Result};

/// Throws the boundary's `TypeError` unless the boxed `value` satisfies `boundary`.
///
/// `prefix` is the message up to the runtime type word (`"f(): Argument #1 ($a) must be of type
/// array, "`); `verb` is what follows it, `"given"` for a parameter and `"returned"` for a return.
pub(super) fn emit_mixed_boundary_guard(
    ctx: &mut FunctionContext<'_>,
    value: ValueId,
    boundary: &MixedBoundary,
    prefix: &str,
    verb: &str,
) -> Result<()> {
    let class_ids = objects::compatible_class_ids(ctx, &boundary.classes);
    let accepted = ctx.next_label("mixed_boundary_ok");
    let object_arm = ctx.next_label("mixed_boundary_object");
    let bool_arm = ctx.next_label("mixed_boundary_bool");

    super::call_operands::load_value_to_first_int_arg(ctx, value)?;
    abi::emit_call_label(ctx.emitter, "__rt_mixed_unbox");
    let tag_reg = abi::int_result_reg(ctx.emitter);
    let mut refuse_arms: Vec<(String, String)> = Vec::new();
    for (tag, name) in objects::RUNTIME_TAGS.iter() {
        let is_array = matches!(tag, 4 | 5);
        if (is_array && boundary.arrays) || (*tag == NULL_TAG && boundary.null) {
            super::enums::emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &accepted);
        } else if *tag == objects::OBJECT_TAG {
            super::enums::emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &object_arm);
        } else if *tag == objects::BOOL_TAG {
            super::enums::emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &bool_arm);
        } else {
            let label = ctx.next_label(&format!("mixed_boundary_reject_{name}"));
            super::enums::emit_mixed_tag_branch(ctx, tag_reg, i64::from(*tag), &label);
            refuse_arms.push((label, format!("{prefix}{name} {verb}")));
        }
    }
    // A tag outside the table never reaches the unbox helper (tag 7 is a nested box it peels),
    // so this fall-through is a defensive refusal rather than a live path.
    exceptions::emit_type_error(ctx, &format!("{prefix}mixed {verb}"));
    for (label, message) in refuse_arms {
        ctx.emitter.label(&label);
        exceptions::emit_type_error(ctx, &message);
    }

    // php-src names a refused bool by value, never as `bool`.
    ctx.emitter.label(&bool_arm);
    let payload_reg = crate::codegen_support::mixed_unbox_payload_reg(ctx.emitter.target);
    let result_reg = abi::int_result_reg(ctx.emitter);
    abi::emit_reg_move(ctx.emitter, result_reg, payload_reg);
    let false_arm = ctx.next_label("mixed_boundary_false");
    abi::emit_branch_if_int_result_zero(ctx.emitter, &false_arm);
    exceptions::emit_type_error(ctx, &format!("{prefix}true {verb}"));
    ctx.emitter.label(&false_arm);
    exceptions::emit_type_error(ctx, &format!("{prefix}false {verb}"));

    // An object passes by class; a refused one is named by its runtime class.
    ctx.emitter.label(&object_arm);
    if boundary.any_object {
        abi::emit_jump(ctx.emitter, &accepted);
    }
    objects::emit_accept_object_classes(ctx.emitter, &class_ids, &accepted);
    let payload_reg = crate::codegen_support::mixed_unbox_payload_reg(ctx.emitter.target);
    let result_reg = abi::int_result_reg(ctx.emitter);
    abi::emit_reg_move(ctx.emitter, result_reg, payload_reg);
    runtime_class_messages::emit_runtime_class_name_to_string_result(ctx, result_reg, "object");
    runtime_class_messages::emit_concat_static_prefix(ctx, prefix);
    runtime_class_messages::emit_concat_static_suffix(ctx, &format!(" {verb}"));
    abi::emit_call_label(ctx.emitter, "__rt_str_persist");
    exceptions::emit_type_error_from_string_result(ctx);

    ctx.emitter.label(&accepted);
    Ok(())
}

/// Lowers `Op::ReturnBoundaryMixedGuard` against the function's declared return type.
pub(super) fn lower_return_boundary_mixed_guard(
    ctx: &mut FunctionContext<'_>,
    inst: &Instruction,
) -> Result<()> {
    let value = *inst.operands.first().ok_or_else(|| {
        CodegenIrError::unsupported("return_boundary_mixed_guard without operand".to_string())
    })?;
    let Some(crate::ir::Immediate::Data(data_id)) = inst.immediate else {
        return Err(CodegenIrError::unsupported(
            "return_boundary_mixed_guard without a TypeError message prefix".to_string(),
        ));
    };
    let Some(boundary) = ctx.function.return_php_type.mixed_boundary() else {
        return Err(CodegenIrError::unsupported(format!(
            "return_boundary_mixed_guard on a {} return",
            ctx.function.return_php_type
        )));
    };
    let prefix = ctx
        .module
        .data
        .strings
        .get(data_id.as_raw() as usize)
        .ok_or_else(|| CodegenIrError::missing_entry("data string", data_id.as_raw()))?
        .to_string();
    emit_mixed_boundary_guard(ctx, value, &boundary, &prefix, "returned")
}

/// The runtime tag of a boxed `null`.
const NULL_TAG: u8 = 8;
