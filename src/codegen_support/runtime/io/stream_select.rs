//! Purpose:
//! Emits the `__rt_stream_select` runtime helper, which waits for readability,
//! writability, or exceptional conditions across descriptor sets.
//!
//! Called from:
//! - `crate::codegen_support::runtime::emitters::emit_runtime()` via `crate::codegen_support::runtime::io`.
//!
//! Key details:
//! - macOS uses `select` (5 args, `timeval`); Linux uses `pselect6` (6 args,
//!   `timespec` + signal mask). The helper sets up six argument registers so a
//!   single `svc` works after `linux_transform` remaps the syscall number.
//! - Descriptor sets are `fd_set` bitmaps of `FD_SETSIZE` (1024) bits, PHP's own limit: the bit
//!   for descriptor `fd` is bit `fd & 63` of word `fd >> 6`, and `nfds` is the highest set
//!   descriptor plus one. A descriptor of 1024 or more is skipped; php-src warns and returns
//!   `false` there instead, which the `Int`-typed builtin cannot express yet.
//! - A set passed as `null` arrives as a zero array pointer and is skipped on both passes.
//!   The three resource arrays are compacted in place to the ready subset.

use crate::codegen_support::{emit::Emitter, platform::{Arch, Platform}};

/// `FD_SETSIZE` on Linux and macOS: descriptors at or above it are never set.
const FD_SETSIZE: i64 = 1024;
/// Bytes in one `fd_set` bitmap of `FD_SETSIZE` bits.
const FD_SET_BYTES: i64 = FD_SETSIZE / 8;
/// AArch64 frame offsets of the read, write and except bitmaps (above the 160-byte state).
const AARCH64_SETS: [i64; 3] = [160, 160 + FD_SET_BYTES, 160 + 2 * FD_SET_BYTES];
/// AArch64 frame size: the state, the three bitmaps.
const AARCH64_FRAME: i64 = 160 + 3 * FD_SET_BYTES;
/// x86_64 `rbp`-relative depths of the read, write and except bitmaps (below the 160-byte state).
const X86_SETS: [i64; 3] = [X86_FRAME, X86_FRAME - FD_SET_BYTES, X86_FRAME - 2 * FD_SET_BYTES];
/// x86_64 frame size below `rbp`.
const X86_FRAME: i64 = 160 + 3 * FD_SET_BYTES;

/// stream_select: select across three resource arrays, compacting each to the
/// ready subset and returning the number of ready descriptors.
/// Input:  AArch64 x0/x1/x2 = read/write/except arrays, x3 = seconds, x4 = microseconds
///         x86_64  rdi/rsi/rdx = read/write/except arrays, rcx = seconds, r8 = microseconds
/// Output: the count of ready descriptors, or -1 on failure
pub fn emit_stream_select(emitter: &mut Emitter) {
    if emitter.target.arch == Arch::X86_64 {
        emit_stream_select_linux_x86_64(emitter);
        return;
    }

    let linux = emitter.target.platform == Platform::Linux;

    emitter.blank();
    emitter.comment("--- runtime: stream_select ---");
    emitter.label_global("__rt_stream_select");

    // Frame (544 bytes): [0]=read_arr [8]=write_arr [16]=except_arr
    //   [48]=timeout.sec [56]=timeout.frac [64]=ready count [72]=nfds [80]=x29 [88]=x30
    //   [96..152]=loop-register spill slots saved across __rt_user_wrapper_stream_cast
    //   [160]=read fd_set [288]=write fd_set [416]=except fd_set (128 bytes each)
    emitter.instruction(&format!("sub sp, sp, #{}", AARCH64_FRAME));            // allocate the select state frame
    emitter.instruction("stp x29, x30, [sp, #80]");                             // save frame pointer and return address
    emitter.instruction("add x29, sp, #80");                                    // establish the helper frame pointer
    emitter.instruction("str x0, [sp, #0]");                                    // save the read resource array
    emitter.instruction("str x1, [sp, #8]");                                    // save the write resource array
    emitter.instruction("str x2, [sp, #16]");                                   // save the except resource array
    emitter.instruction("str x3, [sp, #48]");                                   // timeout seconds field

    // -- timeout fractional field: timeval microseconds vs timespec nanoseconds --
    if linux {
        emitter.instruction("mov x9, #1000");                                   // a microsecond holds 1000 nanoseconds
        emitter.instruction("mul x4, x4, x9");                                  // convert microseconds to timespec nanoseconds
    }
    emitter.instruction("str x4, [sp, #56]");                                   // timeout fractional field

    // -- clear the three fd_set bitmaps and the highest-descriptor bound --
    emitter.instruction(&format!("add x9, sp, #{}", AARCH64_SETS[0]));          // base of the three contiguous fd_set bitmaps
    for offset in (0..3 * FD_SET_BYTES).step_by(16) {
        emitter.instruction(&format!("stp xzr, xzr, [x9, #{}]", offset));       // clear sixteen bytes of descriptor bits
    }
    emitter.instruction("str xzr, [sp, #72]");                                  // nfds starts at zero: no descriptor set yet

    for (arr_off, fds_off, suffix) in [(0, AARCH64_SETS[0], "r"), (8, AARCH64_SETS[1], "w"), (16, AARCH64_SETS[2], "e")] {
        emit_build_fdset_aarch64(emitter, arr_off, fds_off, suffix);
    }

    // -- select(nfds, read, write, except, timeout, sigmask) --
    emitter.instruction("ldr x0, [sp, #72]");                                   // nfds: the highest set descriptor plus one
    emitter.instruction(&format!("add x1, sp, #{}", AARCH64_SETS[0]));          // read descriptor bitmap pointer
    emitter.instruction(&format!("add x2, sp, #{}", AARCH64_SETS[1]));          // write descriptor bitmap pointer
    emitter.instruction(&format!("add x3, sp, #{}", AARCH64_SETS[2]));          // except descriptor bitmap pointer
    emitter.instruction("add x4, sp, #48");                                     // timeout struct pointer
    emitter.instruction("mov x5, #0");                                          // signal mask pointer (NULL); ignored by macOS select
    emitter.syscall(93);
    emitter.instruction("str x0, [sp, #64]");                                   // save the ready descriptor count

    for (arr_off, fds_off, suffix) in [(0, AARCH64_SETS[0], "r"), (8, AARCH64_SETS[1], "w"), (16, AARCH64_SETS[2], "e")] {
        emit_compact_aarch64(emitter, arr_off, fds_off, suffix);
    }

    emitter.instruction("ldr x0, [sp, #64]");                                   // return the ready descriptor count
    emitter.instruction("ldp x29, x30, [sp, #80]");                             // restore frame pointer and return address
    emitter.instruction(&format!("add sp, sp, #{}", AARCH64_FRAME));            // release the select state frame
    emitter.instruction("ret");                                                 // return to the caller
}

/// Set one bit per in-range descriptor of `arr` into the fd_set bitmap at `fds_off`, and raise
/// the `nfds` bound at `[sp, #72]` past each descriptor set.
/// Handles both raw-int resource arrays (`value_type != 7`) and Mixed-boxed
/// arrays (`value_type == 7`, slots hold Mixed* cells whose payload_lo is the fd).
fn emit_build_fdset_aarch64(emitter: &mut Emitter, arr_off: i64, fds_off: i64, suffix: &str) {
    let loop_l = format!("__rt_stream_select_build_{}_loop", suffix);
    let next_l = format!("__rt_stream_select_build_{}_next", suffix);
    let unbox_l = format!("__rt_stream_select_build_{}_unbox", suffix);
    let after_unbox_l = format!("__rt_stream_select_build_{}_after_unbox", suffix);
    let done_l = format!("__rt_stream_select_build_{}_done", suffix);

    emitter.instruction(&format!("ldr x9, [sp, #{}]", arr_off));                // load the resource array pointer
    emitter.instruction(&format!("cbz x9, {}", done_l));                        // a null set (no array) adds no descriptors
    emitter.instruction("ldr x10, [x9]");                                       // load the array length
    emitter.instruction("ldr x4, [x9, #-8]");                                   // load the packed indexed-array kind word
    emitter.instruction("lsr x4, x4, #8");                                      // shift the value_type tag into the low byte
    emitter.instruction("and x4, x4, #0x7f");                                   // isolate the value_type tag
    emitter.instruction("add x12, x9, #24");                                    // skip the array header to the data region
    emitter.instruction("mov x11, #0");                                         // descriptor index
    emitter.label(&loop_l);
    emitter.instruction("cmp x11, x10");                                        // have all descriptors been scanned?
    emitter.instruction(&format!("b.ge {}", done_l));                           // bitmap is fully built
    emitter.instruction("ldr x13, [x12, x11, lsl #3]");                         // load the slot value (raw fd or Mixed* cell)
    emitter.instruction("cmp x4, #7");                                          // is this a Mixed-boxed indexed array?
    emitter.instruction(&format!("b.eq {}", unbox_l));                          // unbox the Mixed cell to get the underlying fd
    emitter.instruction(&format!("b {}", after_unbox_l));                       // raw-int array: x13 already holds the fd
    emitter.label(&unbox_l);
    emitter.instruction(&format!("cbz x13, {}", next_l));                       // null Mixed cell → skip the descriptor
    emitter.instruction("ldr x13, [x13, #8]");                                  // payload_lo of the Mixed cell is the fd
    emitter.label(&after_unbox_l);
    // -- resolve synthetic user-wrapper fds to a real selectable fd via stream_cast --
    let cast_skip_l = format!("__rt_stream_select_build_{}_castskip", suffix);
    emitter.instruction("tst x13, #0x40000000");                                // is this a synthetic user-wrapper descriptor?
    emitter.instruction(&format!("b.eq {}", cast_skip_l));                      // ordinary OS fd → use it directly
    emitter.instruction("str x9, [sp, #96]");                                   // spill the array pointer across the cast call
    emitter.instruction("str x10, [sp, #104]");                                 // spill the array length
    emitter.instruction("str x11, [sp, #112]");                                 // spill the descriptor index
    emitter.instruction("str x4, [sp, #120]");                                  // spill the value_type tag
    emitter.instruction("str x12, [sp, #128]");                                 // spill the data-region pointer
    emitter.instruction("mov x0, x13");                                         // synthetic fd → stream_cast argument
    emitter.instruction("mov x1, #3");                                          // STREAM_CAST_FOR_SELECT
    emitter.instruction("bl __rt_user_wrapper_stream_cast");                    // resolve to the wrapper's underlying fd (or -1)
    emitter.instruction("mov x13, x0");                                         // adopt the resolved descriptor
    emitter.instruction("ldr x9, [sp, #96]");                                   // reload the array pointer
    emitter.instruction("ldr x10, [sp, #104]");                                 // reload the array length
    emitter.instruction("ldr x11, [sp, #112]");                                 // reload the descriptor index
    emitter.instruction("ldr x4, [sp, #120]");                                  // reload the value_type tag
    emitter.instruction("ldr x12, [sp, #128]");                                 // reload the data-region pointer
    emitter.label(&cast_skip_l);
    emitter.instruction("cmp x13, #0");                                         // is the descriptor negative?
    emitter.instruction(&format!("b.lt {}", next_l));                           // skip descriptors that cannot be set
    emitter.instruction(&format!("cmp x13, #{}", FD_SETSIZE));                  // is the descriptor at or above FD_SETSIZE?
    emitter.instruction(&format!("b.ge {}", next_l));                           // skip descriptors no fd_set can hold
    emitter.instruction("lsr x15, x13, #6");                                    // word index of the descriptor's bit
    emitter.instruction("mov x14, #1");                                         // bit seed for the descriptor
    emitter.instruction("lsl x14, x14, x13");                                   // shift by fd & 63 into the bit position
    emitter.instruction(&format!("add x16, sp, #{}", fds_off));                 // base of this set's bitmap
    emitter.instruction("ldr x17, [x16, x15, lsl #3]");                         // load the descriptor's bitmap word
    emitter.instruction("orr x17, x17, x14");                                   // set the descriptor bit
    emitter.instruction("str x17, [x16, x15, lsl #3]");                         // store the updated bitmap word
    emitter.instruction("add x14, x13, #1");                                    // the bound this descriptor needs
    emitter.instruction("ldr x15, [sp, #72]");                                  // load the current nfds bound
    emitter.instruction("cmp x14, x15");                                        // does this descriptor raise the bound?
    emitter.instruction("csel x15, x14, x15, gt");                              // keep the larger bound
    emitter.instruction("str x15, [sp, #72]");                                  // store the nfds bound
    emitter.label(&next_l);
    emitter.instruction("add x11, x11, #1");                                    // advance to the next descriptor
    emitter.instruction(&format!("b {}", loop_l));                              // continue scanning the array
    emitter.label(&done_l);
}

/// Compact `arr` in place to the descriptors whose bit survives in the bitmap.
/// Handles both raw-int arrays and Mixed-boxed arrays: for Mixed slots, the
/// underlying fd is read from `[slot, #8]` for the bitmap check, but the
/// slot itself (the Mixed* cell) is what gets retained in the compacted
/// prefix so the caller's array keeps its boxed contents.
fn emit_compact_aarch64(emitter: &mut Emitter, arr_off: i64, fds_off: i64, suffix: &str) {
    let loop_l = format!("__rt_stream_select_keep_{}_loop", suffix);
    let next_l = format!("__rt_stream_select_keep_{}_next", suffix);
    let unbox_l = format!("__rt_stream_select_keep_{}_unbox", suffix);
    let after_unbox_l = format!("__rt_stream_select_keep_{}_after_unbox", suffix);
    let done_l = format!("__rt_stream_select_keep_{}_done", suffix);
    let absent_l = format!("__rt_stream_select_keep_{}_absent", suffix);

    emitter.instruction(&format!("ldr x9, [sp, #{}]", arr_off));                // load the resource array pointer
    emitter.instruction(&format!("cbz x9, {}", absent_l));                      // a null set has no array to compact
    emitter.instruction("ldr x10, [x9]");                                       // load the array length
    emitter.instruction(&format!("add x14, sp, #{}", fds_off));                 // base of the post-select bitmap
    emitter.instruction("ldr x4, [x9, #-8]");                                   // load the packed indexed-array kind word
    emitter.instruction("lsr x4, x4, #8");                                      // shift the value_type tag into the low byte
    emitter.instruction("and x4, x4, #0x7f");                                   // isolate the value_type tag
    emitter.instruction("add x12, x9, #24");                                    // skip the array header to the data region
    emitter.instruction("mov x11, #0");                                         // source descriptor index
    emitter.instruction("mov x13, #0");                                         // destination (kept) descriptor index
    emitter.label(&loop_l);
    emitter.instruction("cmp x11, x10");                                        // have all descriptors been scanned?
    emitter.instruction(&format!("b.ge {}", done_l));                           // compaction is complete
    emitter.instruction("ldr x15, [x12, x11, lsl #3]");                         // load the raw slot value (preserved for the kept-array store)
    emitter.instruction("mov x16, x15");                                        // copy for fd extraction; x15 stays the slot's stored value
    emitter.instruction("cmp x4, #7");                                          // is this a Mixed-boxed indexed array?
    emitter.instruction(&format!("b.eq {}", unbox_l));                          // unbox the Mixed cell to get the underlying fd
    emitter.instruction(&format!("b {}", after_unbox_l));                       // raw-int array: x16 already holds the fd
    emitter.label(&unbox_l);
    emitter.instruction(&format!("cbz x16, {}", next_l));                       // null Mixed cell → drop the slot
    emitter.instruction("ldr x16, [x16, #8]");                                  // payload_lo of the Mixed cell is the fd
    emitter.label(&after_unbox_l);
    // -- resolve synthetic user-wrapper fds to the same real fd used at build --
    let cast_skip_l = format!("__rt_stream_select_keep_{}_castskip", suffix);
    emitter.instruction("tst x16, #0x40000000");                                // is this a synthetic user-wrapper descriptor?
    emitter.instruction(&format!("b.eq {}", cast_skip_l));                      // ordinary OS fd → use it directly
    emitter.instruction("str x9, [sp, #96]");                                   // spill the array pointer across the cast call
    emitter.instruction("str x10, [sp, #104]");                                 // spill the array length
    emitter.instruction("str x11, [sp, #112]");                                 // spill the source index
    emitter.instruction("str x13, [sp, #120]");                                 // spill the destination (kept) index
    emitter.instruction("str x14, [sp, #136]");                                 // spill the post-select bitmap base
    emitter.instruction("str x4, [sp, #144]");                                  // spill the value_type tag
    emitter.instruction("str x12, [sp, #152]");                                 // spill the data-region pointer
    emitter.instruction("str x15, [sp, #128]");                                 // spill the original slot value (kept on a match)
    emitter.instruction("mov x0, x16");                                         // synthetic fd → stream_cast argument
    emitter.instruction("mov x1, #3");                                          // STREAM_CAST_FOR_SELECT
    emitter.instruction("bl __rt_user_wrapper_stream_cast");                    // resolve to the wrapper's underlying fd (idempotent with build)
    emitter.instruction("mov x16, x0");                                         // adopt the resolved descriptor
    emitter.instruction("ldr x9, [sp, #96]");                                   // reload the array pointer
    emitter.instruction("ldr x10, [sp, #104]");                                 // reload the array length
    emitter.instruction("ldr x11, [sp, #112]");                                 // reload the source index
    emitter.instruction("ldr x13, [sp, #120]");                                 // reload the destination index
    emitter.instruction("ldr x14, [sp, #136]");                                 // reload the post-select bitmap base
    emitter.instruction("ldr x4, [sp, #144]");                                  // reload the value_type tag
    emitter.instruction("ldr x12, [sp, #152]");                                 // reload the data-region pointer
    emitter.instruction("ldr x15, [sp, #128]");                                 // reload the original slot value
    emitter.label(&cast_skip_l);
    emitter.instruction("cmp x16, #0");                                         // is the descriptor negative?
    emitter.instruction(&format!("b.lt {}", next_l));                           // drop descriptors that were never set
    emitter.instruction(&format!("cmp x16, #{}", FD_SETSIZE));                  // is the descriptor at or above FD_SETSIZE?
    emitter.instruction(&format!("b.ge {}", next_l));                           // drop descriptors that were never set
    emitter.instruction("lsr x6, x16, #6");                                     // word index of the descriptor's bit
    emitter.instruction("ldr x6, [x14, x6, lsl #3]");                           // load the descriptor's post-select bitmap word
    emitter.instruction("lsr x6, x6, x16");                                     // shift by fd & 63 to bring the bit to position 0
    emitter.instruction("and x6, x6, #1");                                      // isolate the descriptor's ready bit
    emitter.instruction(&format!("cbz x6, {}", next_l));                        // drop descriptors that are not ready
    emitter.instruction("str x15, [x12, x13, lsl #3]");                         // keep the original slot value (raw fd or Mixed*) at the front
    emitter.instruction("add x13, x13, #1");                                    // advance the kept-descriptor index
    emitter.label(&next_l);
    emitter.instruction("add x11, x11, #1");                                    // advance to the next source descriptor
    emitter.instruction(&format!("b {}", loop_l));                              // continue compacting the array
    emitter.label(&done_l);
    emitter.instruction("str x13, [x9]");                                       // store the compacted array length
    emitter.label(&absent_l);
}

/// Emits the Linux x86_64 stream runtime helper for stream select.
fn emit_stream_select_linux_x86_64(emitter: &mut Emitter) {
    emitter.blank();
    emitter.comment("--- runtime: stream_select ---");
    emitter.label_global("__rt_stream_select");

    // Frame (rbp-relative): [-8]=read_arr [-16]=write_arr [-24]=except_arr
    //   [-56]=timeout.sec [-64]=timeout.nsec [-72]=ready count [-80]=nfds
    //   [-88..-120]=caller-saved loop registers spilled across
    //   __rt_user_wrapper_stream_cast (r12/r13 are callee-saved and survive it)
    //   [-544]=read fd_set [-416]=write fd_set [-288]=except fd_set (128 bytes each)
    emitter.instruction("push rbp");                                            // preserve the caller frame pointer
    emitter.instruction("mov rbp, rsp");                                        // establish the helper frame pointer
    emitter.instruction(&format!("sub rsp, {}", X86_FRAME));                    // allocate the select state frame (incl. cast-call spill slots)
    emitter.instruction("mov QWORD PTR [rbp - 8], rdi");                        // save the read resource array
    emitter.instruction("mov QWORD PTR [rbp - 16], rsi");                       // save the write resource array
    emitter.instruction("mov QWORD PTR [rbp - 24], rdx");                       // save the except resource array
    emitter.instruction("mov QWORD PTR [rbp - 56], rcx");                       // timeout seconds field
    emitter.instruction("mov rax, r8");                                         // microseconds into a scratch register
    emitter.instruction("imul rax, rax, 1000");                                 // convert microseconds to timespec nanoseconds
    emitter.instruction("mov QWORD PTR [rbp - 64], rax");                       // timeout nanoseconds field

    // -- clear the three fd_set bitmaps and the highest-descriptor bound --
    emitter.instruction(&format!("lea rdi, [rbp - {}]", X86_SETS[0]));          // base of the three contiguous fd_set bitmaps
    emitter.instruction(&format!("mov ecx, {}", 3 * FD_SET_BYTES / 8));         // quadwords across the three bitmaps
    emitter.instruction("xor eax, eax");                                        // the fill value: no descriptor bits
    emitter.instruction("rep stosq");                                           // clear every descriptor bit
    emitter.instruction("mov QWORD PTR [rbp - 80], 0");                         // nfds starts at zero: no descriptor set yet

    for (arr_off, fds_off, suffix) in [(8, X86_SETS[0], "r"), (16, X86_SETS[1], "w"), (24, X86_SETS[2], "e")] {
        emit_build_fdset_x86(emitter, arr_off, fds_off, suffix);
    }

    // -- pselect6(nfds, read, write, except, timeout, sigmask) --
    emitter.instruction("mov rdi, QWORD PTR [rbp - 80]");                       // nfds: the highest set descriptor plus one
    emitter.instruction(&format!("lea rsi, [rbp - {}]", X86_SETS[0]));          // read descriptor bitmap pointer
    emitter.instruction(&format!("lea rdx, [rbp - {}]", X86_SETS[1]));          // write descriptor bitmap pointer
    emitter.instruction(&format!("lea r10, [rbp - {}]", X86_SETS[2]));          // except descriptor bitmap pointer
    emitter.instruction("lea r8, [rbp - 56]");                                  // timeout struct pointer
    emitter.instruction("xor r9d, r9d");                                        // signal mask pointer (NULL)
    emitter.instruction("mov eax, 270");                                        // Linux x86_64 syscall 270 = pselect6
    emitter.instruction("syscall");                                             // wait for descriptor readiness
    emitter.instruction("mov QWORD PTR [rbp - 72], rax");                       // save the ready descriptor count

    for (arr_off, fds_off, suffix) in [(8, X86_SETS[0], "r"), (16, X86_SETS[1], "w"), (24, X86_SETS[2], "e")] {
        emit_compact_x86(emitter, arr_off, fds_off, suffix);
    }

    emitter.instruction("mov rax, QWORD PTR [rbp - 72]");                       // return the ready descriptor count
    emitter.instruction(&format!("add rsp, {}", X86_FRAME));                    // release the select state frame
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer
    emitter.instruction("ret");                                                 // return to the caller
}

/// Sets one bit per in-range descriptor of `arr` into the fd_set bitmap at `rbp - fds_off`, and
/// raises the `nfds` bound at `[rbp - 80]` past each descriptor set.
fn emit_build_fdset_x86(emitter: &mut Emitter, arr_off: i64, fds_off: i64, suffix: &str) {
    let loop_l = format!("__rt_stream_select_build_{}_loop_x86", suffix);
    let next_l = format!("__rt_stream_select_build_{}_next_x86", suffix);
    let unbox_l = format!("__rt_stream_select_build_{}_unbox_x86", suffix);
    let after_unbox_l = format!("__rt_stream_select_build_{}_after_unbox_x86", suffix);
    let done_l = format!("__rt_stream_select_build_{}_done_x86", suffix);

    emitter.instruction(&format!("mov r11, QWORD PTR [rbp - {}]", arr_off));    // load the resource array pointer
    emitter.instruction("test r11, r11");                                       // is this set absent (a null argument)?
    emitter.instruction(&format!("jz {}", done_l));                             // a null set adds no descriptors
    emitter.instruction("mov rdi, QWORD PTR [r11]");                            // load the array length
    emitter.instruction("mov r12, QWORD PTR [r11 - 8]");                        // load the packed indexed-array kind word
    emitter.instruction("shr r12, 8");                                          // shift the value_type tag into the low byte
    emitter.instruction("and r12, 0x7f");                                       // isolate the value_type tag
    emitter.instruction("xor rsi, rsi");                                        // descriptor index
    emitter.label(&loop_l);
    emitter.instruction("cmp rsi, rdi");                                        // have all descriptors been scanned?
    emitter.instruction(&format!("jae {}", done_l));                            // bitmap is fully built
    emitter.instruction("mov rdx, QWORD PTR [r11 + 24 + rsi * 8]");             // load the slot value (raw fd or Mixed* cell)
    emitter.instruction("cmp r12, 7");                                          // is this a Mixed-boxed indexed array?
    emitter.instruction(&format!("je {}", unbox_l));                            // unbox the Mixed cell to get the underlying fd
    emitter.instruction(&format!("jmp {}", after_unbox_l));                     // raw-int array: rdx already holds the fd
    emitter.label(&unbox_l);
    emitter.instruction("test rdx, rdx");                                       // null Mixed cell?
    emitter.instruction(&format!("jz {}", next_l));                             // null Mixed cell → skip the descriptor
    emitter.instruction("mov rdx, QWORD PTR [rdx + 8]");                        // payload_lo of the Mixed cell is the fd
    emitter.label(&after_unbox_l);
    // -- resolve synthetic user-wrapper fds to a real selectable fd via stream_cast --
    let cast_skip_l = format!("__rt_stream_select_build_{}_castskip_x86", suffix);
    emitter.instruction("test rdx, 0x40000000");                                // is this a synthetic user-wrapper descriptor?
    emitter.instruction(&format!("jz {}", cast_skip_l));                        // ordinary OS fd → use it directly
    emitter.instruction("mov QWORD PTR [rbp - 88], r11");                       // spill the array pointer across the cast call
    emitter.instruction("mov QWORD PTR [rbp - 96], rdi");                       // spill the array length
    emitter.instruction("mov QWORD PTR [rbp - 104], rsi");                      // spill the descriptor index
    emitter.instruction("mov rdi, rdx");                                        // synthetic fd → stream_cast argument
    emitter.instruction("mov esi, 3");                                          // STREAM_CAST_FOR_SELECT
    emitter.instruction("call __rt_user_wrapper_stream_cast");                  // resolve to the wrapper's underlying fd (or -1)
    emitter.instruction("mov rdx, rax");                                        // adopt the resolved descriptor
    emitter.instruction("mov r11, QWORD PTR [rbp - 88]");                       // reload the array pointer
    emitter.instruction("mov rdi, QWORD PTR [rbp - 96]");                       // reload the array length
    emitter.instruction("mov rsi, QWORD PTR [rbp - 104]");                      // reload the descriptor index
    emitter.label(&cast_skip_l);
    emitter.instruction("cmp rdx, 0");                                          // is the descriptor negative?
    emitter.instruction(&format!("jl {}", next_l));                             // skip descriptors that cannot be set
    emitter.instruction(&format!("cmp rdx, {}", FD_SETSIZE));                   // is the descriptor at or above FD_SETSIZE?
    emitter.instruction(&format!("jae {}", next_l));                            // skip descriptors no fd_set can hold
    emitter.instruction("mov rcx, rdx");                                        // descriptor for the word index
    emitter.instruction("shr rcx, 6");                                          // word index of the descriptor's bit
    emitter.instruction(&format!("lea r10, [rbp + rcx * 8 - {}]", fds_off));    // address of the descriptor's bitmap word
    emitter.instruction("mov rcx, rdx");                                        // shift count: cl & 63 is fd & 63
    emitter.instruction("mov rax, 1");                                          // bit seed for the descriptor
    emitter.instruction("shl rax, cl");                                         // shift the bit into the descriptor position
    emitter.instruction("or QWORD PTR [r10], rax");                             // set the descriptor bit
    emitter.instruction("lea rax, [rdx + 1]");                                  // the bound this descriptor needs
    emitter.instruction("mov r10, QWORD PTR [rbp - 80]");                       // load the current nfds bound
    emitter.instruction("cmp rax, r10");                                        // does this descriptor raise the bound?
    emitter.instruction("cmovg r10, rax");                                      // keep the larger bound
    emitter.instruction("mov QWORD PTR [rbp - 80], r10");                       // store the nfds bound
    emitter.label(&next_l);
    emitter.instruction("add rsi, 1");                                          // advance to the next descriptor
    emitter.instruction(&format!("jmp {}", loop_l));                            // continue scanning the array
    emitter.label(&done_l);
}

/// Compacts `arr` in place to the descriptors whose bit survives in the bitmap at `rbp - fds_off`.
fn emit_compact_x86(emitter: &mut Emitter, arr_off: i64, fds_off: i64, suffix: &str) {
    let loop_l = format!("__rt_stream_select_keep_{}_loop_x86", suffix);
    let next_l = format!("__rt_stream_select_keep_{}_next_x86", suffix);
    let unbox_l = format!("__rt_stream_select_keep_{}_unbox_x86", suffix);
    let after_unbox_l = format!("__rt_stream_select_keep_{}_after_unbox_x86", suffix);
    let done_l = format!("__rt_stream_select_keep_{}_done_x86", suffix);
    let absent_l = format!("__rt_stream_select_keep_{}_absent_x86", suffix);

    emitter.instruction(&format!("mov r11, QWORD PTR [rbp - {}]", arr_off));    // load the resource array pointer
    emitter.instruction("test r11, r11");                                       // is this set absent (a null argument)?
    emitter.instruction(&format!("jz {}", absent_l));                           // a null set has no array to compact
    emitter.instruction("mov rdi, QWORD PTR [r11]");                            // load the array length
    emitter.instruction(&format!("lea r8, [rbp - {}]", fds_off));               // base of the post-select bitmap
    emitter.instruction("mov r12, QWORD PTR [r11 - 8]");                        // load the packed indexed-array kind word
    emitter.instruction("shr r12, 8");                                          // shift the value_type tag into the low byte
    emitter.instruction("and r12, 0x7f");                                       // isolate the value_type tag
    emitter.instruction("xor rsi, rsi");                                        // source descriptor index
    emitter.instruction("xor r9, r9");                                          // destination (kept) descriptor index
    emitter.label(&loop_l);
    emitter.instruction("cmp rsi, rdi");                                        // have all descriptors been scanned?
    emitter.instruction(&format!("jae {}", done_l));                            // compaction is complete
    emitter.instruction("mov r13, QWORD PTR [r11 + 24 + rsi * 8]");             // load the raw slot value (preserved for the kept-array store)
    emitter.instruction("mov rdx, r13");                                        // copy for fd extraction; r13 stays the slot's stored value
    emitter.instruction("cmp r12, 7");                                          // is this a Mixed-boxed indexed array?
    emitter.instruction(&format!("je {}", unbox_l));                            // unbox the Mixed cell to get the underlying fd
    emitter.instruction(&format!("jmp {}", after_unbox_l));                     // raw-int array: rdx already holds the fd
    emitter.label(&unbox_l);
    emitter.instruction("test rdx, rdx");                                       // null Mixed cell?
    emitter.instruction(&format!("jz {}", next_l));                             // null Mixed cell → drop the slot
    emitter.instruction("mov rdx, QWORD PTR [rdx + 8]");                        // payload_lo of the Mixed cell is the fd
    emitter.label(&after_unbox_l);
    // -- resolve synthetic user-wrapper fds to the same real fd used at build --
    let cast_skip_l = format!("__rt_stream_select_keep_{}_castskip_x86", suffix);
    emitter.instruction("test rdx, 0x40000000");                                // is this a synthetic user-wrapper descriptor?
    emitter.instruction(&format!("jz {}", cast_skip_l));                        // ordinary OS fd → use it directly
    emitter.instruction("mov QWORD PTR [rbp - 88], r11");                       // spill the array pointer across the cast call
    emitter.instruction("mov QWORD PTR [rbp - 96], rdi");                       // spill the array length
    emitter.instruction("mov QWORD PTR [rbp - 104], rsi");                      // spill the source index
    emitter.instruction("mov QWORD PTR [rbp - 112], r8");                       // spill the post-select bitmap base
    emitter.instruction("mov QWORD PTR [rbp - 120], r9");                       // spill the destination (kept) index
    emitter.instruction("mov rdi, rdx");                                        // synthetic fd → stream_cast argument
    emitter.instruction("mov esi, 3");                                          // STREAM_CAST_FOR_SELECT
    emitter.instruction("call __rt_user_wrapper_stream_cast");                  // resolve to the wrapper's underlying fd (idempotent with build)
    emitter.instruction("mov rdx, rax");                                        // adopt the resolved descriptor
    emitter.instruction("mov r11, QWORD PTR [rbp - 88]");                       // reload the array pointer
    emitter.instruction("mov rdi, QWORD PTR [rbp - 96]");                       // reload the array length
    emitter.instruction("mov rsi, QWORD PTR [rbp - 104]");                      // reload the source index
    emitter.instruction("mov r8, QWORD PTR [rbp - 112]");                       // reload the post-select bitmap base
    emitter.instruction("mov r9, QWORD PTR [rbp - 120]");                       // reload the destination index
    emitter.label(&cast_skip_l);
    emitter.instruction("cmp rdx, 0");                                          // is the descriptor negative?
    emitter.instruction(&format!("jl {}", next_l));                             // drop descriptors that were never set
    emitter.instruction(&format!("cmp rdx, {}", FD_SETSIZE));                   // is the descriptor at or above FD_SETSIZE?
    emitter.instruction(&format!("jae {}", next_l));                            // drop descriptors that were never set
    emitter.instruction("mov rcx, rdx");                                        // descriptor for the word index
    emitter.instruction("shr rcx, 6");                                          // word index of the descriptor's bit
    emitter.instruction("mov rax, QWORD PTR [r8 + rcx * 8]");                   // load the descriptor's post-select bitmap word
    emitter.instruction("mov rcx, rdx");                                        // shift count: cl & 63 is fd & 63
    emitter.instruction("shr rax, cl");                                         // shift the descriptor bit to position 0
    emitter.instruction("and rax, 1");                                          // isolate the descriptor's ready bit
    emitter.instruction(&format!("jz {}", next_l));                             // drop descriptors that are not ready
    emitter.instruction("mov QWORD PTR [r11 + 24 + r9 * 8], r13");              // keep the original slot value (raw fd or Mixed*) at the front
    emitter.instruction("add r9, 1");                                           // advance the kept-descriptor index
    emitter.label(&next_l);
    emitter.instruction("add rsi, 1");                                          // advance to the next source descriptor
    emitter.instruction(&format!("jmp {}", loop_l));                            // continue compacting the array
    emitter.label(&done_l);
    emitter.instruction("mov QWORD PTR [r11], r9");                             // store the compacted array length
    emitter.label(&absent_l);
}
