//! Purpose:
//! Lowers EIR buffer allocation and direct buffer opcodes for the ASM backend.
//! Public Buffer values are generation-safe scalar handles resolved by the runtime.
//!
//! Called from:
//! - `crate::codegen::lower_inst::lower_instruction()`.
//!
//! Key details:
//! - Only a validated descriptor may supply payload, length, or stride metadata.
//! - Buffer get/set preserve PHP source evaluation order while spilling call-clobbered values.
//! - The resolved get/set forms take a descriptor from a hoisted `buffer_resolve`. They test it
//!   for zero at the access, so a stale handle still reports the use-after-free there, and they
//!   touch only the result register plus x9-x11 (AArch64) or r10/r11 (x86_64), which keeps them
//!   volatile-safe for the register allocator.

use crate::codegen::abi;
use crate::codegen::platform::Arch;
use crate::codegen::{CodegenIrError, Result};
use crate::ir::{Immediate, Instruction, LocalSlotId, Op, ValueDef, ValueId};
use crate::types::PhpType;

use super::super::context::FunctionContext;
use super::{expect_operand, store_if_result};

/// Lowers `BufferNew` by passing length and element stride to `__rt_buffer_new`.
pub(super) fn lower_buffer_new(ctx: &mut FunctionContext<'_>, inst: &Instruction) -> Result<()> {
    ensure_arg_count(inst, "buffer_new", 1)?;
    let len = expect_operand(inst, 0)?;
    let result_ty = result_buffer_type(inst)?;
    let stride = buffer_stride(ctx, &result_ty)?;
    require_int(ctx.load_value_to_result(len)?, "buffer_new length")?;
    match ctx.emitter.target.arch {
        Arch::AArch64 => {
            ctx.emitter.instruction(&format!("mov x1, #{}", stride));           // pass the fixed element stride to the buffer allocation helper
        }
        Arch::X86_64 => {
            ctx.emitter.instruction(&format!("mov rdi, {}", stride));           // pass the fixed element stride while keeping the length in rax
        }
    }
    abi::emit_call_label(ctx.emitter, "__rt_buffer_new");
    store_if_result(ctx, inst)
}

/// Lowers `buffer_len(buffer)` by delegating handle resolution and length access to the runtime.
pub(super) fn lower_buffer_len(ctx: &mut FunctionContext<'_>, inst: &Instruction) -> Result<()> {
    ensure_arg_count(inst, "buffer_len", 1)?;
    let buffer = expect_operand(inst, 0)?;
    require_buffer(ctx.load_value_to_result(buffer)?, "buffer_len")?;
    abi::emit_call_label(ctx.emitter, "__rt_buffer_len");
    store_if_result(ctx, inst)
}

/// Lowers `buffer_free(buffer)` by releasing non-null local handles and nulling the owner slot.
pub(super) fn lower_buffer_free(ctx: &mut FunctionContext<'_>, inst: &Instruction) -> Result<()> {
    ensure_arg_count(inst, "buffer_free", 1)?;
    let buffer = expect_operand(inst, 0)?;
    let slot = source_load_local_slot(ctx, buffer)?.ok_or_else(|| {
        CodegenIrError::unsupported("buffer_free argument that is not a local load")
    })?;
    require_buffer(ctx.load_value_to_result(buffer)?, "buffer_free")?;
    match ctx.emitter.target.arch {
        Arch::AArch64 => {
            let already_freed = ctx.next_label("buffer_free_already_freed");
            ctx.emitter.instruction(&format!("cbz x0, {}", already_freed));     // treat a repeated free of the nulled owner slot as a no-op
            abi::emit_call_label(ctx.emitter, "__rt_buffer_free");
            ctx.emitter.label(&already_freed);
        }
        Arch::X86_64 => {
            let already_freed = ctx.next_label("buffer_free_already_freed");
            ctx.emitter.instruction("test rax, rax");                           // distinguish the nulled owner slot from a live generation-safe handle
            ctx.emitter.instruction(&format!("jz {}", already_freed));          // repeated frees are defined as an idempotent no-op
            abi::emit_call_label(ctx.emitter, "__rt_buffer_free");
            ctx.emitter.label(&already_freed);
        }
    }
    let offset = ctx.local_offset(slot)?;
    abi::emit_store_zero_to_local_slot(ctx.emitter, offset);
    emit_void_result(ctx);
    store_if_result(ctx, inst)
}

/// Lowers `BufferGet` by resolving its handle, checking descriptor bounds, and loading a scalar element.
pub(super) fn lower_buffer_get(ctx: &mut FunctionContext<'_>, inst: &Instruction) -> Result<()> {
    ensure_arg_count(inst, "buffer_get", 2)?;
    let buffer = expect_operand(inst, 0)?;
    let index = expect_operand(inst, 1)?;
    let buffer_ty = ctx.load_value_to_result(buffer)?;
    let elem_ty = require_buffer(buffer_ty, "buffer_get")?;
    abi::emit_push_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
    require_int(ctx.load_value_to_result(index)?, "buffer_get index")?;
    abi::emit_push_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
    let address_reg = materialize_resolved_element_address_for_get(ctx);
    load_element_value(ctx, &elem_ty, address_reg)?;
    store_if_result(ctx, inst)
}

/// Lowers `BufferSet` by resolving its handle after source-order buffer, index, and RHS evaluation.
pub(super) fn lower_buffer_set(ctx: &mut FunctionContext<'_>, inst: &Instruction) -> Result<()> {
    ensure_arg_count(inst, "buffer_set", 3)?;
    let buffer = expect_operand(inst, 0)?;
    let index = expect_operand(inst, 1)?;
    let value = expect_operand(inst, 2)?;
    let buffer_ty = ctx.load_value_to_result(buffer)?;
    let elem_ty = require_buffer(buffer_ty, "buffer_set")?;
    abi::emit_push_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
    require_int(ctx.load_value_to_result(index)?, "buffer_set index")?;
    abi::emit_push_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
    materialize_set_value(ctx, value, &elem_ty)?;
    push_buffer_set_value(ctx, &elem_ty)?;
    let address_reg = materialize_resolved_element_address_for_set(ctx);
    pop_buffer_set_value(ctx, &elem_ty)?;
    abi::emit_release_temporary_stack(ctx.emitter, 32);
    store_element_value(ctx, &elem_ty, address_reg)?;
    store_if_result(ctx, inst)
}

/// Lowers `BufferResolve` by asking the runtime for the handle's descriptor, or 0 when stale.
pub(super) fn lower_buffer_resolve(ctx: &mut FunctionContext<'_>, inst: &Instruction) -> Result<()> {
    ensure_arg_count(inst, "buffer_resolve", 1)?;
    let buffer = expect_operand(inst, 0)?;
    require_buffer(ctx.load_value_to_result(buffer)?, "buffer_resolve")?;
    abi::emit_call_label(ctx.emitter, "__rt_buffer_try_resolve");
    store_if_result(ctx, inst)
}

/// Lowers `BufferGetResolved` through a hoisted descriptor: zero test, bounds check, then the
/// element load (or the packed record address).
pub(super) fn lower_buffer_get_resolved(
    ctx: &mut FunctionContext<'_>,
    inst: &Instruction,
) -> Result<()> {
    ensure_arg_count(inst, "buffer_get_resolved", 2)?;
    let descriptor = expect_operand(inst, 0)?;
    let index = expect_operand(inst, 1)?;
    let address_reg = emit_resolved_element_address(ctx, descriptor, index)?;
    load_element_value(ctx, &inst.result_php_type.codegen_repr(), address_reg)?;
    store_if_result(ctx, inst)
}

/// Lowers `BufferSetResolved` through a hoisted descriptor: zero test and bounds check first,
/// then the stored value is materialized straight into the result register and written.
pub(super) fn lower_buffer_set_resolved(
    ctx: &mut FunctionContext<'_>,
    inst: &Instruction,
) -> Result<()> {
    ensure_arg_count(inst, "buffer_set_resolved", 3)?;
    let descriptor = expect_operand(inst, 0)?;
    let index = expect_operand(inst, 1)?;
    let value = expect_operand(inst, 2)?;
    let address_reg = emit_resolved_element_address(ctx, descriptor, index)?;
    let value_ty = ctx.load_value_to_result(value)?.codegen_repr();
    match value_ty {
        PhpType::Float | PhpType::Int | PhpType::Bool | PhpType::Pointer(_) | PhpType::Resource(_) => {
            store_element_value(ctx, &value_ty, address_reg)?;
        }
        other => {
            return Err(CodegenIrError::unsupported(format!(
                "buffer_set_resolved value PHP type {:?}",
                other
            )))
        }
    }
    store_if_result(ctx, inst)
}

/// Checks a hoisted descriptor and index and returns the register holding the element address.
///
/// AArch64 uses x10 (index), x11 (descriptor, then address) and x9; x86_64 uses r10 (index,
/// then scaled offset) and r11 (descriptor, then address). Operands are loaded before any of
/// those registers is reused, and the fault paths never return.
fn emit_resolved_element_address(
    ctx: &mut FunctionContext<'_>,
    descriptor: ValueId,
    index: ValueId,
) -> Result<&'static str> {
    let live = ctx.next_label("buffer_descriptor_live");
    let non_negative = ctx.next_label("buffer_index_non_negative");
    let bounds_ok = ctx.next_label("buffer_index_in_bounds");
    match ctx.emitter.target.arch {
        Arch::AArch64 => {
            require_int(ctx.load_value_to_reg(index, "x10")?, "buffer_get_resolved index")?;
            ctx.load_value_to_reg(descriptor, "x11")?;
            ctx.emitter.instruction(&format!("cbnz x11, {}", live));            // a hoisted descriptor of zero means the handle was stale
            ctx.emitter.instruction("b __rt_buffer_use_after_free");            // report the use-after-free at the access, like the unresolved form
            ctx.emitter.label(&live);
            ctx.emitter.instruction("cmp x10, #0");                             // reject negative buffer indexes before touching the payload
            ctx.emitter.instruction(&format!("b.ge {}", non_negative));         // continue once the requested index is non-negative
            ctx.emitter.instruction("b __rt_buffer_bounds_fail");               // abort immediately on a negative buffer index
            ctx.emitter.label(&non_negative);
            ctx.emitter.instruction("ldr x9, [x11, #8]");                       // validated descriptor logical length
            ctx.emitter.instruction("cmp x10, x9");                             // compare the requested index against the length
            ctx.emitter.instruction(&format!("b.lo {}", bounds_ok));            // continue once the requested index is within bounds
            ctx.emitter.instruction("b __rt_buffer_bounds_fail");               // abort immediately on an out-of-range buffer index
            ctx.emitter.label(&bounds_ok);
            ctx.emitter.instruction("ldr x9, [x11, #16]");                      // element stride in bytes
            ctx.emitter.instruction("ldr x11, [x11]");                          // payload base address
            ctx.emitter.instruction("madd x11, x10, x9, x11");                  // payload base plus index multiplied by stride
            Ok("x11")
        }
        Arch::X86_64 => {
            require_int(ctx.load_value_to_reg(index, "r10")?, "buffer_get_resolved index")?;
            ctx.load_value_to_reg(descriptor, "r11")?;
            ctx.emitter.instruction("test r11, r11");                           // a hoisted descriptor of zero means the handle was stale
            ctx.emitter.instruction(&format!("jnz {}", live));                  // continue with a live descriptor
            ctx.emitter.instruction("jmp __rt_buffer_use_after_free");          // report the use-after-free at the access, like the unresolved form
            ctx.emitter.label(&live);
            ctx.emitter.instruction("cmp r10, 0");                              // reject negative buffer indexes before touching the payload
            ctx.emitter.instruction(&format!("jge {}", non_negative));          // continue once the requested index is non-negative
            ctx.emitter.instruction("jmp __rt_buffer_bounds_fail");             // abort immediately on a negative buffer index
            ctx.emitter.label(&non_negative);
            ctx.emitter.instruction("cmp r10, QWORD PTR [r11 + 8]");            // compare the requested index against the logical length
            ctx.emitter.instruction(&format!("jb {}", bounds_ok));              // continue once the requested unsigned index is within bounds
            ctx.emitter.instruction("jmp __rt_buffer_bounds_fail");             // abort immediately on an out-of-range buffer index
            ctx.emitter.label(&bounds_ok);
            ctx.emitter.instruction("imul r10, QWORD PTR [r11 + 16]");          // scale the requested index by the element stride in bytes
            ctx.emitter.instruction("add r10, QWORD PTR [r11]");                // add the payload base address
            Ok("r10")
        }
    }
}

/// Returns the declared buffer type for a `BufferNew` result.
fn result_buffer_type(inst: &Instruction) -> Result<PhpType> {
    match inst.result {
        Some(_) => match inst.result_php_type.codegen_repr() {
            PhpType::Buffer(elem) => Ok(PhpType::Buffer(elem)),
            other => Err(CodegenIrError::unsupported(format!(
                "buffer_new result PHP type {:?}",
                other
            ))),
        },
        None => Err(CodegenIrError::invalid_module(
            "buffer_new instruction missing result".to_string(),
        )),
    }
}

/// Returns the fixed runtime stride for supported buffer element types.
fn buffer_stride(ctx: &FunctionContext<'_>, buffer_ty: &PhpType) -> Result<usize> {
    match buffer_ty.codegen_repr() {
        PhpType::Buffer(elem) => match elem.codegen_repr() {
            PhpType::Int
            | PhpType::Float
            | PhpType::Bool
            | PhpType::Pointer(_)
            | PhpType::Resource(_) => Ok(8),
            PhpType::Packed(name) => ctx
                .module
                .packed_class_infos
                .get(&name)
                .map(|info| info.total_size)
                .ok_or_else(|| CodegenIrError::unsupported(format!(
                    "unknown packed buffer element type {}",
                    name
                ))),
            other => Err(CodegenIrError::unsupported(format!(
                "buffer_new element PHP type {:?}",
                other
            ))),
        },
        other => Err(CodegenIrError::unsupported(format!(
            "buffer_new PHP type {:?}",
            other
        ))),
    }
}

/// Verifies a value is represented as a generation-safe scalar Buffer handle.
fn require_buffer(ty: PhpType, name: &str) -> Result<PhpType> {
    match ty.codegen_repr() {
        PhpType::Buffer(elem_ty) => Ok(*elem_ty),
        other => Err(CodegenIrError::unsupported(format!(
            "{} for PHP type {:?}",
            name,
            other
        ))),
    }
}

/// Returns the local slot loaded by a buffer operand when it came from `load_local`.
fn source_load_local_slot(ctx: &FunctionContext<'_>, value: ValueId) -> Result<Option<LocalSlotId>> {
    let Some(value_ref) = ctx.function.value(value) else {
        return Err(CodegenIrError::missing_entry("value", value.as_raw()));
    };
    let ValueDef::Instruction { inst, .. } = value_ref.def else {
        return Ok(None);
    };
    let Some(inst_ref) = ctx.function.instruction(inst) else {
        return Err(CodegenIrError::missing_entry("instruction", inst.as_raw()));
    };
    if inst_ref.op == Op::LoadLocal {
        if let Some(Immediate::LocalSlot(slot)) = inst_ref.immediate {
            return Ok(Some(slot));
        }
    }
    Ok(None)
}

/// Resolves the stacked Buffer handle and returns a bounds-checked payload address for a read.
fn materialize_resolved_element_address_for_get(ctx: &mut FunctionContext<'_>) -> &'static str {
    match ctx.emitter.target.arch {
        Arch::AArch64 => {
            abi::emit_load_temporary_stack_slot(ctx.emitter, "x0", 16);
            abi::emit_call_label(ctx.emitter, "__rt_buffer_resolve");
            abi::emit_load_temporary_stack_slot(ctx.emitter, "x10", 0);
            abi::emit_release_temporary_stack(ctx.emitter, 32);
            emit_checked_address_arm64(ctx, "x0", "x10");
        }
        Arch::X86_64 => {
            abi::emit_load_temporary_stack_slot(ctx.emitter, "rax", 16);
            abi::emit_call_label(ctx.emitter, "__rt_buffer_resolve");
            abi::emit_load_temporary_stack_slot(ctx.emitter, "r10", 0);
            abi::emit_release_temporary_stack(ctx.emitter, 32);
            emit_checked_address_x86_64(ctx, "rax", "r10");
        }
    }
    match ctx.emitter.target.arch {
        Arch::AArch64 => "x11",
        Arch::X86_64 => "r11",
    }
}

/// Resolves the stacked Buffer handle and returns a bounds-checked payload address for a write.
/// The RHS remains at the top of the temporary stack until after resolver and bounds checks.
fn materialize_resolved_element_address_for_set(ctx: &mut FunctionContext<'_>) -> &'static str {
    match ctx.emitter.target.arch {
        Arch::AArch64 => {
            abi::emit_load_temporary_stack_slot(ctx.emitter, "x0", 32);
            abi::emit_call_label(ctx.emitter, "__rt_buffer_resolve");
            abi::emit_load_temporary_stack_slot(ctx.emitter, "x10", 16);
            emit_checked_address_arm64(ctx, "x0", "x10");
        }
        Arch::X86_64 => {
            abi::emit_load_temporary_stack_slot(ctx.emitter, "rax", 32);
            abi::emit_call_label(ctx.emitter, "__rt_buffer_resolve");
            abi::emit_load_temporary_stack_slot(ctx.emitter, "r10", 16);
            emit_checked_address_x86_64(ctx, "rax", "r10");
        }
    }
    match ctx.emitter.target.arch {
        Arch::AArch64 => "x11",
        Arch::X86_64 => "r11",
    }
}

/// Emits ARM64 descriptor bounds checks and computes its payload element address.
fn emit_checked_address_arm64(ctx: &mut FunctionContext<'_>, descriptor_reg: &str, index_reg: &str) {
    let non_negative = ctx.next_label("buffer_index_non_negative");
    let bounds_ok = ctx.next_label("buffer_index_in_bounds");
    ctx.emitter.instruction(&format!("cmp {}, #0", index_reg));                 // reject negative buffer indexes before touching the payload
    ctx.emitter.instruction(&format!("b.ge {}", non_negative));                 // continue once the requested index is non-negative
    ctx.emitter.instruction("b __rt_buffer_bounds_fail");                       // abort immediately on a negative buffer index
    ctx.emitter.label(&non_negative);
    abi::emit_load_from_address(ctx.emitter, "x12", descriptor_reg, 8);
    ctx.emitter.instruction(&format!("cmp {}, x12", index_reg));                // compare requested index against validated descriptor logical length
    ctx.emitter.instruction(&format!("b.lo {}", bounds_ok));                    // continue once the requested index is within bounds
    ctx.emitter.instruction("b __rt_buffer_bounds_fail");                       // abort immediately on an out-of-range buffer index
    ctx.emitter.label(&bounds_ok);
    abi::emit_load_from_address(ctx.emitter, "x11", descriptor_reg, 0);
    abi::emit_load_from_address(ctx.emitter, "x12", descriptor_reg, 16);
    ctx.emitter.instruction(&format!("madd x11, {}, x12, x11", index_reg));     // compute payload base plus index multiplied by descriptor stride
}

/// Emits x86_64 descriptor bounds checks and computes its payload element address.
fn emit_checked_address_x86_64(ctx: &mut FunctionContext<'_>, descriptor_reg: &str, index_reg: &str) {
    let non_negative = ctx.next_label("buffer_index_non_negative");
    let bounds_ok = ctx.next_label("buffer_index_in_bounds");
    ctx.emitter.instruction(&format!("cmp {}, 0", index_reg));                  // reject negative buffer indexes before touching the payload
    ctx.emitter.instruction(&format!("jge {}", non_negative));                  // continue once the requested index is non-negative
    ctx.emitter.instruction("jmp __rt_buffer_bounds_fail");                     // abort immediately on a negative buffer index
    ctx.emitter.label(&non_negative);
    abi::emit_load_from_address(ctx.emitter, "rcx", descriptor_reg, 8);
    ctx.emitter.instruction(&format!("cmp {}, rcx", index_reg));                // compare requested index against validated descriptor logical length
    ctx.emitter.instruction(&format!("jb {}", bounds_ok));                      // continue once the requested unsigned index is within bounds
    ctx.emitter.instruction("jmp __rt_buffer_bounds_fail");                     // abort immediately on an out-of-range buffer index
    ctx.emitter.label(&bounds_ok);
    abi::emit_load_from_address(ctx.emitter, "r11", descriptor_reg, 0);
    abi::emit_load_from_address(ctx.emitter, "rcx", descriptor_reg, 16);
    ctx.emitter.instruction(&format!("imul {}, rcx", index_reg));               // scale the requested index by the element stride in bytes
    ctx.emitter.instruction(&format!("add r11, {}", index_reg));                // compute payload base plus index multiplied by descriptor stride
}

/// Pushes the materialized BufferSet RHS so resolver and bounds code cannot clobber it.
fn push_buffer_set_value(ctx: &mut FunctionContext<'_>, elem_ty: &PhpType) -> Result<()> {
    match elem_ty.codegen_repr() {
        PhpType::Float => {
            abi::emit_push_float_reg(ctx.emitter, abi::float_result_reg(ctx.emitter));
            Ok(())
        }
        PhpType::Int | PhpType::Bool | PhpType::Pointer(_) | PhpType::Resource(_) => {
            abi::emit_push_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
            Ok(())
        }
        other => Err(CodegenIrError::unsupported(format!(
            "buffer_set stack spill for PHP type {:?}",
            other
        ))),
    }
}

/// Restores the BufferSet RHS after resolver and bounds code have produced the payload address.
fn pop_buffer_set_value(ctx: &mut FunctionContext<'_>, elem_ty: &PhpType) -> Result<()> {
    match elem_ty.codegen_repr() {
        PhpType::Float => {
            abi::emit_pop_float_reg(ctx.emitter, abi::float_result_reg(ctx.emitter));
            Ok(())
        }
        PhpType::Int | PhpType::Bool | PhpType::Pointer(_) | PhpType::Resource(_) => {
            abi::emit_pop_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
            Ok(())
        }
        other => Err(CodegenIrError::unsupported(format!(
            "buffer_set stack restore for PHP type {:?}",
            other
        ))),
    }
}

/// Loads a scalar element from the checked payload address.
fn load_element_value(ctx: &mut FunctionContext<'_>, elem_ty: &PhpType, address_reg: &str) -> Result<()> {
    match elem_ty.codegen_repr() {
        PhpType::Float => {
            abi::emit_load_from_address(ctx.emitter, abi::float_result_reg(ctx.emitter), address_reg, 0);
            Ok(())
        }
        PhpType::Int | PhpType::Bool | PhpType::Pointer(_) | PhpType::Resource(_) => {
            abi::emit_load_from_address(ctx.emitter, abi::int_result_reg(ctx.emitter), address_reg, 0);
            Ok(())
        }
        PhpType::Packed(_) => {
            let result_reg = abi::int_result_reg(ctx.emitter);
            if result_reg != address_reg {
                match ctx.emitter.target.arch {
                    Arch::AArch64 => {
                        ctx.emitter.instruction(
                            &format!("mov {}, {}", result_reg, address_reg)
                        );                                                      // return the checked packed-element address as the packed receiver pointer
                    }
                    Arch::X86_64 => {
                        ctx.emitter.instruction(
                            &format!("mov {}, {}", result_reg, address_reg)
                        );                                                      // return the checked packed-element address as the packed receiver pointer
                    }
                }
            }
            Ok(())
        }
        other => Err(CodegenIrError::unsupported(format!(
            "buffer_get element PHP type {:?}",
            other
        ))),
    }
}

/// Materializes a scalar value suitable for direct storage into a buffer element.
fn materialize_set_value(
    ctx: &mut FunctionContext<'_>,
    value: crate::ir::ValueId,
    elem_ty: &PhpType,
) -> Result<()> {
    let value_ty = ctx.load_value_to_result(value)?;
    match (elem_ty.codegen_repr(), value_ty.codegen_repr()) {
        (PhpType::Float, PhpType::Float) => Ok(()),
        (PhpType::Int, PhpType::Int | PhpType::Bool)
        | (PhpType::Bool, PhpType::Int | PhpType::Bool)
        | (PhpType::Pointer(_), PhpType::Pointer(_))
        | (PhpType::Resource(_), PhpType::Resource(_)) => Ok(()),
        (expected, actual) => Err(CodegenIrError::unsupported(format!(
            "buffer_set element {:?} from PHP type {:?}",
            expected,
            actual
        ))),
    }
}

/// Stores the current scalar result into the checked payload address.
fn store_element_value(ctx: &mut FunctionContext<'_>, elem_ty: &PhpType, address_reg: &str) -> Result<()> {
    match elem_ty.codegen_repr() {
        PhpType::Float => {
            abi::emit_store_to_address(ctx.emitter, abi::float_result_reg(ctx.emitter), address_reg, 0);
            Ok(())
        }
        PhpType::Int | PhpType::Bool | PhpType::Pointer(_) | PhpType::Resource(_) => {
            abi::emit_store_to_address(ctx.emitter, abi::int_result_reg(ctx.emitter), address_reg, 0);
            Ok(())
        }
        other => Err(CodegenIrError::unsupported(format!(
            "buffer_set element PHP type {:?}",
            other
        ))),
    }
}

/// Materializes the EIR void/null sentinel for a `buffer_free()` result.
fn emit_void_result(ctx: &mut FunctionContext<'_>) {
    abi::emit_load_int_immediate(
        ctx.emitter,
        abi::int_result_reg(ctx.emitter),
        0x7fff_ffff_ffff_fffe,
    );
}

/// Verifies a value is a concrete integer.
fn require_int(ty: PhpType, name: &str) -> Result<()> {
    match ty.codegen_repr() {
        PhpType::Int => Ok(()),
        other => Err(CodegenIrError::unsupported(format!(
            "{} PHP type {:?}",
            name,
            other
        ))),
    }
}

/// Verifies a buffer opcode received the expected number of operands.
fn ensure_arg_count(inst: &Instruction, name: &str, expected: usize) -> Result<()> {
    if inst.operands.len() == expected {
        return Ok(());
    }
    Err(CodegenIrError::invalid_module(format!(
        "{} expected {} args, got {}",
        name,
        expected,
        inst.operands.len()
    )))
}
