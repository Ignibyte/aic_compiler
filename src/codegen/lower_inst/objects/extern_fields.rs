//! Purpose:
//! Lowers field reads and writes through `ptr<ExternClass>` receivers, the C structs declared
//! with `extern class`.
//!
//! Called from:
//! - `crate::codegen::lower_inst::objects::known_property_reads::lower_prop_get()`.
//! - `crate::codegen::lower_inst::objects::runtime_property_writes::lower_prop_set()`.
//!
//! Key details:
//! - Each field is accessed at its natural C offset with its declared width: integer reads sign
//!   or zero extend into the 64-bit PHP int, integer writes store only the low bytes, `f32`
//!   rounds through an IEEE 754 single, and `bool` is one byte read back as 0 or 1.
//! - A `string` (`char*`) field reads as a borrowed view (pointer plus scanned length) that
//!   lowering immediately copies with `StrPersist`; a write stores a fresh null-terminated copy
//!   on the elephc heap that the struct keeps, like an `extern global string` store.
//! - This is the extern sibling of the packed `ptr<T>` receiver route
//!   (`property_resolution::packed_receiver_class`): the receiver is the struct's raw address, and
//!   every access first aborts on null through `__rt_ptr_check_nonnull`, exactly like
//!   `emit_typed_pointer_receiver_null_check` does for packed receivers.

use super::*;
use crate::types::{ExternAbiType, ExternFieldInfo};

/// Integer scratch register that carries a store payload while the struct pointer is restored.
fn store_scratch_reg(arch: Arch) -> &'static str {
    match arch {
        Arch::AArch64 => "x10",
        Arch::X86_64 => "rcx",
    }
}

/// Looks up the declared C field a `ptr<Class>` access names.
fn resolve_extern_field(
    ctx: &FunctionContext<'_>,
    class_name: &str,
    property: &str,
    inst: &Instruction,
) -> Result<ExternFieldInfo> {
    let class_info = ctx.module.extern_class_infos.get(class_name).ok_or_else(|| {
        CodegenIrError::unsupported(format!("unknown extern class {}", class_name))
    })?;
    class_info
        .fields
        .iter()
        .find(|field| field.name == property)
        .cloned()
        .ok_or_else(|| {
            CodegenIrError::unsupported(format!(
                "{} for missing extern field {}::${}",
                inst.op.name(),
                class_name,
                property
            ))
        })
}

/// Lowers `$ptr->field` for a `ptr<ExternClass>` receiver into a width-aware load.
pub(super) fn lower_extern_prop_get(
    ctx: &mut FunctionContext<'_>,
    inst: &Instruction,
    object: ValueId,
    class_name: &str,
    property: &str,
) -> Result<()> {
    let field = resolve_extern_field(ctx, class_name, property, inst)?;
    // -- null-check the struct pointer --
    ctx.load_value_to_result(object)?;
    abi::emit_call_label(ctx.emitter, "__rt_ptr_check_nonnull");
    // -- load the field at its C width --
    let operand = extern_field_operand(ctx, &field);
    emit_extern_field_load(ctx, &field, &operand)?;
    store_if_result(ctx, inst)
}

/// Lowers `$ptr->field = value` for a `ptr<ExternClass>` receiver into a width-aware store.
pub(super) fn lower_extern_prop_set(
    ctx: &mut FunctionContext<'_>,
    inst: &Instruction,
    object: ValueId,
    value: ValueId,
    class_name: &str,
    property: &str,
) -> Result<()> {
    let field = resolve_extern_field(ctx, class_name, property, inst)?;
    // -- null-check the struct pointer and keep it while the value loads --
    ctx.load_value_to_result(object)?;
    abi::emit_call_label(ctx.emitter, "__rt_ptr_check_nonnull");
    abi::emit_push_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
    let in_float_reg = materialize_extern_field_value(ctx, &field, value)?;
    // -- park the payload and restore the struct pointer --
    let arch = ctx.emitter.target.arch;
    if !in_float_reg {
        let scratch = store_scratch_reg(arch);
        let result = abi::int_result_reg(ctx.emitter);
        ctx.emitter.instruction(&format!("mov {}, {}", scratch, result));       // keep the field payload while the struct pointer is restored
    }
    abi::emit_pop_reg(ctx.emitter, abi::int_result_reg(ctx.emitter));
    // -- store the field at its C width --
    let operand = extern_field_operand(ctx, &field);
    emit_extern_field_store(ctx, &field, &operand)?;
    store_if_result(ctx, inst)
}

/// Loads the value to store into the register its field width is written from.
///
/// Returns `true` when the payload is a double in the float result register, `false` when it
/// is a word in the integer result register.
fn materialize_extern_field_value(
    ctx: &mut FunctionContext<'_>,
    field: &ExternFieldInfo,
    value: ValueId,
) -> Result<bool> {
    let value_ty = ctx.value_php_type(value)?.codegen_repr();
    match (field.abi, value_ty) {
        (ExternAbiType::Float64 | ExternAbiType::Float32, PhpType::Float) => {
            ctx.load_value_to_result(value)?;
            Ok(true)
        }
        (ExternAbiType::Float64 | ExternAbiType::Float32, PhpType::Int) => {
            ctx.load_value_to_result(value)?;
            abi::emit_int_result_to_float_result(ctx.emitter);
            Ok(true)
        }
        (ExternAbiType::Int { .. }, PhpType::Int)
        | (ExternAbiType::Bool, PhpType::Bool)
        | (ExternAbiType::Ptr, PhpType::Pointer(_)) => {
            ctx.load_value_to_result(value)?;
            Ok(false)
        }
        (ExternAbiType::Ptr, PhpType::Void) => {
            abi::emit_load_int_immediate(ctx.emitter, abi::int_result_reg(ctx.emitter), 0);
            Ok(false)
        }
        (ExternAbiType::CStr, PhpType::Str) => {
            ctx.load_value_to_result(value)?;
            // The struct keeps the copy: C owns a `char*` field, so it outlives this statement.
            abi::emit_call_label(ctx.emitter, "__rt_str_to_cstr");
            Ok(false)
        }
        (abi_ty, other) => Err(CodegenIrError::unsupported(format!(
            "extern field {} store of PHP type {:?} into C shape {:?}",
            field.name, other, abi_ty
        ))),
    }
}

/// Returns the memory operand of a field relative to the struct pointer in the result register.
///
/// AArch64 encodes an unsigned offset of up to 4095 elements of the access size; a field past
/// that (only in structs larger than 4 KiB) first moves the base register onto the field.
fn extern_field_operand(ctx: &mut FunctionContext<'_>, field: &ExternFieldInfo) -> String {
    let offset = field.offset;
    match ctx.emitter.target.arch {
        Arch::AArch64 => {
            let size = extern_field_size(field.abi);
            if offset == 0 {
                "[x0]".to_string()
            } else if offset % size == 0 && offset / size <= 4095 {
                format!("[x0, #{}]", offset)
            } else {
                // -- move the base onto a field past the scaled-immediate range --
                abi::emit_load_int_immediate(ctx.emitter, "x9", offset as i64);
                ctx.emitter.instruction("add x0, x0, x9");                      // move the struct pointer onto a field beyond the scaled-immediate range
                "[x0]".to_string()
            }
        }
        Arch::X86_64 => {
            if offset == 0 {
                "[rax]".to_string()
            } else {
                format!("[rax + {}]", offset)
            }
        }
    }
}

/// Returns the byte size of a field's C shape.
fn extern_field_size(abi_ty: ExternAbiType) -> usize {
    match abi_ty {
        ExternAbiType::Int { bits, .. } => usize::from(bits / 8),
        ExternAbiType::Float32 => 4,
        ExternAbiType::Bool => 1,
        ExternAbiType::Float64
        | ExternAbiType::CStr
        | ExternAbiType::Ptr
        | ExternAbiType::Callable
        | ExternAbiType::Void => 8,
    }
}

/// Emits the width-aware load of one field into the canonical result register(s).
fn emit_extern_field_load(
    ctx: &mut FunctionContext<'_>,
    field: &ExternFieldInfo,
    operand: &str,
) -> Result<()> {
    let arch = ctx.emitter.target.arch;
    // -- load the C field into the PHP result register --
    match (arch, field.abi) {
        (Arch::AArch64, ExternAbiType::Int { bits: 64, .. } | ExternAbiType::Ptr) => {
            ctx.emitter.instruction(&format!("ldr x0, {}", operand));           // load the 64-bit C field as the PHP value
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 32, signed: true }) => {
            ctx.emitter.instruction(&format!("ldrsw x0, {}", operand));         // load the C int32 field and sign-extend it into the PHP int
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 32, signed: false }) => {
            ctx.emitter.instruction(&format!("ldr w0, {}", operand));           // load the C uint32 field and zero-extend it into the PHP int
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 16, signed: true }) => {
            ctx.emitter.instruction(&format!("ldrsh x0, {}", operand));         // load the C int16 field and sign-extend it into the PHP int
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 16, signed: false }) => {
            ctx.emitter.instruction(&format!("ldrh w0, {}", operand));          // load the C uint16 field and zero-extend it into the PHP int
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 8, signed: true }) => {
            ctx.emitter.instruction(&format!("ldrsb x0, {}", operand));         // load the C int8 field and sign-extend it into the PHP int
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 8, signed: false }) => {
            ctx.emitter.instruction(&format!("ldrb w0, {}", operand));          // load the C uint8 field and zero-extend it into the PHP int
        }
        (Arch::AArch64, ExternAbiType::Bool) => {
            // -- read the C bool byte as a PHP bool --
            ctx.emitter.instruction(&format!("ldrb w0, {}", operand));          // load the one-byte C bool field
            ctx.emitter.instruction("cmp w0, #0");                              // test whether the C bool byte is set
            ctx.emitter.instruction("cset x0, ne");                             // read any nonzero C bool byte as PHP true
        }
        (Arch::AArch64, ExternAbiType::Float64) => {
            ctx.emitter.instruction(&format!("ldr d0, {}", operand));           // load the C double field as the PHP float
        }
        (Arch::AArch64, ExternAbiType::Float32) => {
            // -- widen the C float into the PHP double --
            ctx.emitter.instruction(&format!("ldr s0, {}", operand));           // load the C float field as an IEEE 754 single
            abi::emit_float_result_single_to_double(ctx.emitter);
        }
        (Arch::AArch64, ExternAbiType::CStr) => {
            let scan = ctx.next_label("extern_cstr_scan");
            let done = ctx.next_label("extern_cstr_done");
            // -- borrow the C string and measure it up to its terminator --
            ctx.emitter.instruction(&format!("ldr x1, {}", operand));           // load the C char* field as the borrowed string pointer
            ctx.emitter.instruction("mov x2, #0");                              // start the borrowed view at length zero
            ctx.emitter.instruction(&format!("cbz x1, {}", done));              // read a NULL char* as the empty string
            ctx.emitter.label(&scan);
            ctx.emitter.instruction("ldrb w9, [x1, x2]");                       // load the next C string byte
            ctx.emitter.instruction(&format!("cbz w9, {}", done));              // stop at the C string terminator
            ctx.emitter.instruction("add x2, x2, #1");                          // count one more byte of the borrowed view
            ctx.emitter.instruction(&format!("b {}", scan));                    // keep scanning for the terminator
            ctx.emitter.label(&done);
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 64, .. } | ExternAbiType::Ptr) => {
            ctx.emitter.instruction(&format!(
                "mov rax, QWORD PTR {}",
                operand
            ));                                                                 // load the 64-bit C field as the PHP value
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 32, signed: true }) => {
            ctx.emitter.instruction(&format!(
                "movsxd rax, DWORD PTR {}",
                operand
            ));                                                                 // load the C int32 field and sign-extend it into the PHP int
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 32, signed: false }) => {
            ctx.emitter.instruction(&format!(
                "mov eax, DWORD PTR {}",
                operand
            ));                                                                 // load the C uint32 field and zero-extend it into the PHP int
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 16, signed: true }) => {
            ctx.emitter.instruction(&format!(
                "movsx rax, WORD PTR {}",
                operand
            ));                                                                 // load the C int16 field and sign-extend it into the PHP int
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 16, signed: false }) => {
            ctx.emitter.instruction(&format!(
                "movzx eax, WORD PTR {}",
                operand
            ));                                                                 // load the C uint16 field and zero-extend it into the PHP int
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 8, signed: true }) => {
            ctx.emitter.instruction(&format!(
                "movsx rax, BYTE PTR {}",
                operand
            ));                                                                 // load the C int8 field and sign-extend it into the PHP int
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 8, signed: false }) => {
            ctx.emitter.instruction(&format!(
                "movzx eax, BYTE PTR {}",
                operand
            ));                                                                 // load the C uint8 field and zero-extend it into the PHP int
        }
        (Arch::X86_64, ExternAbiType::Bool) => {
            // -- read the C bool byte as a PHP bool --
            ctx.emitter.instruction(&format!(
                "movzx eax, BYTE PTR {}",
                operand
            ));                                                                 // load the one-byte C bool field
            ctx.emitter.instruction("test eax, eax");                           // test whether the C bool byte is set
            ctx.emitter.instruction("setne al");                                // read any nonzero C bool byte as PHP true
            ctx.emitter.instruction("movzx eax, al");                           // widen the 0/1 flag into the PHP bool payload
        }
        (Arch::X86_64, ExternAbiType::Float64) => {
            ctx.emitter.instruction(&format!(
                "movsd xmm0, QWORD PTR {}",
                operand
            ));                                                                 // load the C double field as the PHP float
        }
        (Arch::X86_64, ExternAbiType::Float32) => {
            // -- widen the C float into the PHP double --
            ctx.emitter.instruction(&format!(
                "movss xmm0, DWORD PTR {}",
                operand
            ));                                                                 // load the C float field as an IEEE 754 single
            abi::emit_float_result_single_to_double(ctx.emitter);
        }
        (Arch::X86_64, ExternAbiType::CStr) => {
            let scan = ctx.next_label("extern_cstr_scan");
            let done = ctx.next_label("extern_cstr_done");
            // -- borrow the C string and measure it up to its terminator --
            ctx.emitter.instruction(&format!(
                "mov rax, QWORD PTR {}",
                operand
            ));                                                                 // load the C char* field as the borrowed string pointer
            ctx.emitter.instruction("xor edx, edx");                            // start the borrowed view at length zero
            ctx.emitter.instruction("test rax, rax");                           // check for a NULL char*
            ctx.emitter.instruction(&format!("jz {}", done));                   // read a NULL char* as the empty string
            ctx.emitter.label(&scan);
            ctx.emitter.instruction("cmp BYTE PTR [rax + rdx], 0");             // test the next C string byte for the terminator
            ctx.emitter.instruction(&format!("je {}", done));                   // stop at the C string terminator
            ctx.emitter.instruction("add rdx, 1");                              // count one more byte of the borrowed view
            ctx.emitter.instruction(&format!("jmp {}", scan));                  // keep scanning for the terminator
            ctx.emitter.label(&done);
        }
        (_, other) => {
            return Err(CodegenIrError::unsupported(format!(
                "extern field {} load of C shape {:?}",
                field.name, other
            )))
        }
    }
    Ok(())
}

/// Emits the width-aware store of one field; the struct pointer is back in the result register
/// and the payload is in the store scratch register or the float result register.
fn emit_extern_field_store(
    ctx: &mut FunctionContext<'_>,
    field: &ExternFieldInfo,
    operand: &str,
) -> Result<()> {
    let arch = ctx.emitter.target.arch;
    // -- store the payload into the C field --
    match (arch, field.abi) {
        (Arch::AArch64, ExternAbiType::Int { bits: 64, .. } | ExternAbiType::Ptr | ExternAbiType::CStr) => {
            ctx.emitter.instruction(&format!("str x10, {}", operand));          // store the 64-bit payload into the C field
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 32, .. }) => {
            ctx.emitter.instruction(&format!("str w10, {}", operand));          // store the low 32 bits into the C 32-bit field
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 16, .. }) => {
            ctx.emitter.instruction(&format!("strh w10, {}", operand));         // store the low 16 bits into the C 16-bit field
        }
        (Arch::AArch64, ExternAbiType::Int { bits: 8, .. } | ExternAbiType::Bool) => {
            ctx.emitter.instruction(&format!("strb w10, {}", operand));         // store the low byte into the C 8-bit or bool field
        }
        (Arch::AArch64, ExternAbiType::Float64) => {
            ctx.emitter.instruction(&format!("str d0, {}", operand));           // store the PHP float into the C double field
        }
        (Arch::AArch64, ExternAbiType::Float32) => {
            // -- round the PHP double into the C float --
            abi::emit_float_result_double_to_single(ctx.emitter);
            ctx.emitter.instruction(&format!("str s0, {}", operand));           // store the rounded IEEE 754 single into the C float field
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 64, .. } | ExternAbiType::Ptr | ExternAbiType::CStr) => {
            ctx.emitter.instruction(&format!(
                "mov QWORD PTR {}, rcx",
                operand
            ));                                                                 // store the 64-bit payload into the C field
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 32, .. }) => {
            ctx.emitter.instruction(&format!(
                "mov DWORD PTR {}, ecx",
                operand
            ));                                                                 // store the low 32 bits into the C 32-bit field
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 16, .. }) => {
            ctx.emitter.instruction(&format!("mov WORD PTR {}, cx", operand));  // store the low 16 bits into the C 16-bit field
        }
        (Arch::X86_64, ExternAbiType::Int { bits: 8, .. } | ExternAbiType::Bool) => {
            ctx.emitter.instruction(&format!("mov BYTE PTR {}, cl", operand));  // store the low byte into the C 8-bit or bool field
        }
        (Arch::X86_64, ExternAbiType::Float64) => {
            ctx.emitter.instruction(&format!(
                "movsd QWORD PTR {}, xmm0",
                operand
            ));                                                                 // store the PHP float into the C double field
        }
        (Arch::X86_64, ExternAbiType::Float32) => {
            // -- round the PHP double into the C float --
            abi::emit_float_result_double_to_single(ctx.emitter);
            ctx.emitter.instruction(&format!(
                "movss DWORD PTR {}, xmm0",
                operand
            ));                                                                 // store the rounded IEEE 754 single into the C float field
        }
        (_, other) => {
            return Err(CodegenIrError::unsupported(format!(
                "extern field {} store of C shape {:?}",
                field.name, other
            )))
        }
    }
    Ok(())
}
