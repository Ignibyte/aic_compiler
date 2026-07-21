//! Purpose:
//! Emits the `__rt_array_slice_str` runtime helper assembly for slicing a `string`-element indexed
//! array. Each sliced element is a 16-byte string descriptor (pointer + length) whose heap buffer is
//! OWNED by its array (`__rt_array_free_deep` frees string payloads for string arrays), so the slice
//! must `__rt_str_persist` every element into its own owned copy — a raw descriptor copy would share
//! the source's buffers and double-free them when the slice is released.
//!
//! Called from:
//! - `crate::codegen_support::runtime::emitters::emit_runtime()` via `crate::codegen_support::runtime::arrays`.
//!
//! Key details:
//! - Same offset/length normalization as `__rt_array_slice`. All loop state lives on the stack so the
//!   per-element `__rt_str_persist` call (which clobbers caller-saved registers) is safe.

use crate::codegen_support::emit::Emitter;
use crate::codegen_support::platform::Arch;

/// Emits the `__rt_array_slice_str` runtime helper.
///
/// # ABI
/// - Input: (x0/rdi) = source array pointer, (x1/rsi) = element offset, (x2/rdx) = slice length
///   - length == -1 indicates "read to end of array"
/// - Output: (x0/rax) = pointer to a newly allocated `string`-element array with owned copies
pub fn emit_array_slice_str(emitter: &mut Emitter) {
    if emitter.target.arch == Arch::X86_64 {
        emit_array_slice_str_linux_x86_64(emitter);
        return;
    }

    emitter.blank();
    emitter.comment("--- runtime: array_slice_str (16-byte string descriptors, per-element persist) ---");
    emitter.label_global("__rt_array_slice_str");

    // stack: [sp,0]=src ptr, [sp,8]=offset, [sp,16]=length, [sp,24]=dst ptr, [sp,32]=i
    emitter.instruction("sub sp, sp, #80");
    emitter.instruction("stp x29, x30, [sp, #64]");
    emitter.instruction("add x29, sp, #64");
    emitter.instruction("str x0, [sp, #0]");                                    // save source array pointer
    emitter.instruction("ldr x9, [x0]");                                        // x9 = source element count

    // -- normalize offset --
    emitter.instruction("cmp x1, #0");
    emitter.instruction("b.ge __rt_array_slice_str_pos_off");
    emitter.instruction("add x1, x9, x1");                                      // offset = len + offset
    emitter.instruction("cmp x1, #0");
    emitter.instruction("csel x1, xzr, x1, lt");                                // clamp to 0

    emitter.label("__rt_array_slice_str_pos_off");
    emitter.instruction("cmp x1, x9");
    emitter.instruction("b.ge __rt_array_slice_str_empty");                     // offset >= len -> empty
    emitter.instruction("sub x3, x9, x1");                                      // x3 = remaining
    super::slice_length::emit_slice_length_normalization(
        emitter,
        "x2",
        "x3",
        "x4",
        "x5",
        "__rt_array_slice_str_len",
    );
    emitter.instruction("str x1, [sp, #8]");                                    // save offset
    emitter.instruction("str x2, [sp, #16]");                                   // save length

    // -- allocate result (16-byte string slots) --
    emitter.instruction("mov x0, x2");                                          // capacity = slice length
    emitter.instruction("mov x1, #16");                                         // elem_size = 16
    emitter.instruction("bl __rt_array_new");
    emitter.instruction("str x0, [sp, #24]");                                   // save dest pointer
    emitter.instruction("str xzr, [sp, #32]");                                  // i = 0

    // -- copy loop: persist each source string into the destination --
    emitter.label("__rt_array_slice_str_copy");
    emitter.instruction("ldr x6, [sp, #32]");                                   // i
    emitter.instruction("ldr x4, [sp, #16]");                                   // length
    emitter.instruction("cmp x6, x4");
    emitter.instruction("b.ge __rt_array_slice_str_done");
    emitter.instruction("ldr x3, [sp, #8]");                                    // offset
    emitter.instruction("add x7, x3, x6");                                      // source index = offset + i
    emitter.instruction("ldr x5, [sp, #0]");                                    // source ptr
    emitter.instruction("add x10, x5, x7, lsl #4");                             // + (offset+i)*16
    emitter.instruction("ldp x1, x2, [x10, #24]");                              // load (ptr,len) past the 24-byte header
    emitter.instruction("bl __rt_str_persist");                                 // x1 = owned ptr, x2 = owned len
    emitter.instruction("ldr x6, [sp, #32]");                                   // reload i (persist clobbered regs)
    emitter.instruction("ldr x0, [sp, #24]");                                   // dest ptr
    emitter.instruction("add x12, x0, x6, lsl #4");                             // dest + i*16
    emitter.instruction("stp x1, x2, [x12, #24]");                              // store owned (ptr,len)
    emitter.instruction("add x6, x6, #1");                                      // i += 1
    emitter.instruction("str x6, [sp, #32]");
    emitter.instruction("b __rt_array_slice_str_copy");

    emitter.label("__rt_array_slice_str_done");
    emitter.instruction("ldr x0, [sp, #24]");                                   // dest ptr
    emitter.instruction("ldr x9, [sp, #16]");                                   // length
    emitter.instruction("str x9, [x0]");                                        // publish length
    emitter.instruction("ldp x29, x30, [sp, #64]");
    emitter.instruction("add sp, sp, #80");
    emitter.instruction("ret");

    emitter.label("__rt_array_slice_str_empty");
    emitter.instruction("mov x0, #0");
    emitter.instruction("mov x1, #16");
    emitter.instruction("bl __rt_array_new");
    emitter.instruction("ldp x29, x30, [sp, #64]");
    emitter.instruction("add sp, sp, #80");
    emitter.instruction("ret");
}

/// Emits the `__rt_array_slice_str` runtime helper for x86_64 Linux.
fn emit_array_slice_str_linux_x86_64(emitter: &mut Emitter) {
    emitter.blank();
    emitter.comment("--- runtime: array_slice_str (16-byte string descriptors, per-element persist) ---");
    emitter.label_global("__rt_array_slice_str");

    // stack: [rbp-8]=src ptr, [rbp-16]=offset, [rbp-24]=length, [rbp-32]=dst ptr, [rbp-40]=i
    emitter.instruction("push rbp");
    emitter.instruction("mov rbp, rsp");
    emitter.instruction("sub rsp, 48");
    emitter.instruction("mov QWORD PTR [rbp - 8], rdi");                        // source array pointer
    emitter.instruction("mov r10, QWORD PTR [rdi]");                            // source element count

    // -- normalize offset --
    emitter.instruction("cmp rsi, 0");
    emitter.instruction("jge __rt_array_slice_str_pos_off_x86");
    emitter.instruction("add rsi, r10");                                        // offset = len + offset
    emitter.instruction("cmp rsi, 0");
    emitter.instruction("jge __rt_array_slice_str_pos_off_x86");
    emitter.instruction("xor esi, esi");                                        // clamp to 0

    emitter.label("__rt_array_slice_str_pos_off_x86");
    emitter.instruction("cmp rsi, r10");
    emitter.instruction("jge __rt_array_slice_str_empty_x86");                  // offset >= len -> empty
    emitter.instruction("mov rcx, r10");
    emitter.instruction("sub rcx, rsi");                                        // remaining
    super::slice_length::emit_slice_length_normalization(
        emitter,
        "rdx",
        "rcx",
        "r11",
        "r11",
        "__rt_array_slice_str_len_x86",
    );
    emitter.instruction("mov QWORD PTR [rbp - 16], rsi");                       // offset
    emitter.instruction("mov QWORD PTR [rbp - 24], rdx");                       // length
    emitter.instruction("mov rdi, rdx");                                        // capacity
    emitter.instruction("mov rsi, 16");                                         // elem_size = 16
    emitter.instruction("call __rt_array_new");
    emitter.instruction("mov QWORD PTR [rbp - 32], rax");                       // dest pointer
    emitter.instruction("mov QWORD PTR [rbp - 40], 0");                         // i = 0

    // -- copy loop: persist each source string into the destination --
    emitter.label("__rt_array_slice_str_copy_x86");
    emitter.instruction("mov rax, QWORD PTR [rbp - 40]");                       // i
    emitter.instruction("cmp rax, QWORD PTR [rbp - 24]");                       // i < length?
    emitter.instruction("jge __rt_array_slice_str_done_x86");
    emitter.instruction("mov rcx, QWORD PTR [rbp - 16]");                       // offset
    emitter.instruction("add rcx, rax");                                        // offset + i
    emitter.instruction("shl rcx, 4");                                          // * 16
    emitter.instruction("mov r8, QWORD PTR [rbp - 8]");                         // source ptr
    emitter.instruction("add r8, rcx");                                         // + (offset+i)*16
    emitter.instruction("mov rax, QWORD PTR [r8 + 24]");                        // ptr (past 24-byte header)
    emitter.instruction("mov rdx, QWORD PTR [r8 + 32]");                        // len
    emitter.instruction("call __rt_str_persist");                              // rax = owned ptr, rdx = owned len
    emitter.instruction("mov rcx, QWORD PTR [rbp - 40]");                       // reload i (persist clobbered regs)
    emitter.instruction("shl rcx, 4");                                          // i * 16
    emitter.instruction("mov r8, QWORD PTR [rbp - 32]");                        // dest ptr
    emitter.instruction("add r8, rcx");                                         // + i*16
    emitter.instruction("mov QWORD PTR [r8 + 24], rax");                        // store owned ptr
    emitter.instruction("mov QWORD PTR [r8 + 32], rdx");                        // store owned len
    emitter.instruction("inc QWORD PTR [rbp - 40]");                            // i += 1
    emitter.instruction("jmp __rt_array_slice_str_copy_x86");

    emitter.label("__rt_array_slice_str_done_x86");
    emitter.instruction("mov rax, QWORD PTR [rbp - 32]");                       // dest ptr
    emitter.instruction("mov rcx, QWORD PTR [rbp - 24]");                       // length
    emitter.instruction("mov QWORD PTR [rax], rcx");                            // publish length
    emitter.instruction("add rsp, 48");
    emitter.instruction("pop rbp");
    emitter.instruction("ret");

    emitter.label("__rt_array_slice_str_empty_x86");
    emitter.instruction("mov rdi, 0");
    emitter.instruction("mov rsi, 16");
    emitter.instruction("call __rt_array_new");
    emitter.instruction("add rsp, 48");
    emitter.instruction("pop rbp");
    emitter.instruction("ret");
}
