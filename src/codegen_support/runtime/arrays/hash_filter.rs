//! Purpose:
//! Emits the `__rt_hash_filter` runtime helper: filters an associative-array (hash) source with a
//! predicate callback, producing a new hash that preserves the surviving entries' keys.
//!
//! Called from:
//! - `crate::codegen_support::runtime::emitters::emit_runtime()` via `crate::codegen_support::runtime::arrays`.
//!
//! Key details:
//! - Complements `__rt_array_filter_refcounted` (indexed arrays) with the hash path. It walks the
//!   source via `__rt_hash_iter_next`, boxes each entry's value (and, for the key modes, its key)
//!   into a reused Mixed cell to invoke the first-class-callable predicate, and re-inserts the kept
//!   entry into a fresh hash via `__rt_hash_set`.
//! - `__rt_hash_set` does NOT retain the value it stores (it only releases an overwritten value on
//!   key update), so a kept refcounted value is `__rt_incref`-ed before insertion — both the source
//!   and result hashes then own it. String keys are persisted by `__rt_hash_set` itself.
//! - The per-callback Mixed cell is a raw box of the BORROWED (lo,hi,tag) triple: no payload is
//!   retained, and the cell is freed with `__rt_heap_free` (a raw block free) after the walk, so the
//!   source's payloads are never touched. FCC type predicates borrow (never retain) their argument.

use crate::codegen_support::emit::Emitter;
use crate::codegen_support::platform::Arch;

/// x86_64 boxed-Mixed heap kind word: the ELEPHC signature in the high 32 bits combined with the
/// low-byte kind `5`. Matches `__rt_array_to_mixed`'s Mixed-cell stamp so a reader cannot tell the
/// reused predicate cell from a genuine boxed-Mixed cell.
const X86_64_MIXED_CELL_KIND: u64 = (0x454C_5048u64 << 32) | 5;

/// Emits `__rt_hash_filter` for the host target.
///
/// ABI (ARM64): x0 = callback wrapper address, x1 = source hash pointer, x2 = optional callback
/// environment pointer (0 = none), x3 = mode (0 = USE_VALUE, 1 = USE_BOTH, 2 = USE_KEY). Returns the
/// filtered hash pointer in x0.
///
/// ABI (x86_64 SysV): rdi = callback, rsi = source hash, rdx = env, rcx = mode. Returns rax.
pub fn emit_hash_filter(emitter: &mut Emitter) {
    if emitter.target.arch == Arch::X86_64 {
        emit_hash_filter_linux_x86_64(emitter);
        return;
    }

    emitter.blank();
    emitter.comment("--- runtime: hash_filter ---");
    emitter.label_global("__rt_hash_filter");

    // -- frame + callee-saved: x19=callback, x20=source, x21=env, x22=mode, x23=cursor, x24=result --
    // slots: [sp+0]=valcell [sp+8]=keycell [sp+16]=key_ptr [sp+24]=key_len [sp+32]=value_lo
    //        [sp+40]=value_hi [sp+48]=value_tag
    emitter.instruction("sub sp, sp, #128");
    emitter.instruction("stp x29, x30, [sp, #112]");
    emitter.instruction("add x29, sp, #112");
    emitter.instruction("stp x19, x20, [sp, #96]");
    emitter.instruction("stp x21, x22, [sp, #80]");
    emitter.instruction("stp x23, x24, [sp, #64]");
    emitter.instruction("mov x19, x0"); // callback wrapper address
    emitter.instruction("mov x20, x1"); // source hash pointer
    emitter.instruction("mov x21, x2"); // optional callback environment pointer
    emitter.instruction("mov x22, x3"); // mode

    // -- result = __rt_hash_new(source_count, value_type 7 = mixed) --
    emitter.instruction("ldr x0, [x20]"); // source entry count at hash header offset 0
    emitter.instruction("mov x1, #7");
    emitter.instruction("bl __rt_hash_new");
    emitter.instruction("mov x24, x0"); // result hash pointer

    // -- allocate two reusable Mixed cells (value + key) for predicate invocation --
    emitter.instruction("mov x0, #24");
    emitter.instruction("bl __rt_heap_alloc");
    emitter.instruction("mov x9, #5"); // low-byte kind 5 = boxed Mixed cell
    emitter.instruction("str x9, [x0, #-8]");
    emitter.instruction("str x0, [sp, #0]"); // valcell
    emitter.instruction("mov x0, #24");
    emitter.instruction("bl __rt_heap_alloc");
    emitter.instruction("mov x9, #5");
    emitter.instruction("str x9, [x0, #-8]");
    emitter.instruction("str x0, [sp, #8]"); // keycell
    emitter.instruction("mov x23, #0"); // cursor

    emitter.label("__rt_hash_filter_loop");
    emitter.instruction("mov x0, x20");
    emitter.instruction("mov x1, x23");
    emitter.instruction("bl __rt_hash_iter_next"); // x0=cursor x1=key_ptr x2=key_len x3=lo x4=hi x5=tag
    emitter.instruction("cmn x0, #1"); // cursor == -1 -> iteration done
    emitter.instruction("b.eq __rt_hash_filter_done");
    emitter.instruction("mov x23, x0"); // save next cursor before any clobbering call
    emitter.instruction("str x1, [sp, #16]"); // key_ptr
    emitter.instruction("str x2, [sp, #24]"); // key_len (-1 = integer key)
    emitter.instruction("str x3, [sp, #32]"); // value_lo
    emitter.instruction("str x4, [sp, #40]"); // value_hi
    emitter.instruction("str x5, [sp, #48]"); // value_tag

    // -- build the value cell (raw box of the borrowed value triple) --
    emitter.instruction("ldr x10, [sp, #0]");
    emitter.instruction("str x5, [x10]");
    emitter.instruction("str x3, [x10, #8]");
    emitter.instruction("str x4, [x10, #16]");

    // -- build the key cell: string key (tag 1, ptr+len) or integer key (tag 0, value) --
    emitter.instruction("ldr x10, [sp, #8]");
    emitter.instruction("ldr x11, [sp, #24]");
    emitter.instruction("cmn x11, #1"); // key_len == -1 -> integer key
    emitter.instruction("b.eq __rt_hash_filter_key_int");
    emitter.instruction("mov x12, #1");
    emitter.instruction("str x12, [x10]");
    emitter.instruction("ldr x12, [sp, #16]");
    emitter.instruction("str x12, [x10, #8]");
    emitter.instruction("ldr x12, [sp, #24]");
    emitter.instruction("str x12, [x10, #16]");
    emitter.instruction("b __rt_hash_filter_key_done");
    emitter.label("__rt_hash_filter_key_int");
    emitter.instruction("str xzr, [x10]"); // tag 0 = integer
    emitter.instruction("ldr x12, [sp, #16]");
    emitter.instruction("str x12, [x10, #8]");
    emitter.instruction("str xzr, [x10, #16]");
    emitter.label("__rt_hash_filter_key_done");

    // -- invoke the predicate with mode-specific arguments --
    emitter.instruction("cmp x22, #2");
    emitter.instruction("b.eq __rt_hash_filter_arg_key");
    emitter.instruction("cmp x22, #1");
    emitter.instruction("b.eq __rt_hash_filter_arg_both");
    emitter.instruction("ldr x0, [sp, #0]"); // USE_VALUE: arg0 = value cell
    emitter.instruction("cbz x21, __rt_hash_filter_call");
    emitter.instruction("mov x1, x21");
    emitter.instruction("b __rt_hash_filter_call");
    emitter.label("__rt_hash_filter_arg_key");
    emitter.instruction("ldr x0, [sp, #8]"); // USE_KEY: arg0 = key cell
    emitter.instruction("cbz x21, __rt_hash_filter_call");
    emitter.instruction("mov x1, x21");
    emitter.instruction("b __rt_hash_filter_call");
    emitter.label("__rt_hash_filter_arg_both");
    emitter.instruction("ldr x0, [sp, #0]"); // USE_BOTH: arg0 = value cell, arg1 = key cell
    emitter.instruction("ldr x1, [sp, #8]");
    emitter.instruction("cbz x21, __rt_hash_filter_call");
    emitter.instruction("mov x2, x21");
    emitter.label("__rt_hash_filter_call");
    emitter.instruction("blr x19");
    emitter.instruction("cbz x0, __rt_hash_filter_skip"); // falsy predicate -> drop entry

    // -- retain the kept value (refcounted tags 1,4,5,6,7) before the result takes ownership --
    emitter.instruction("ldr x5, [sp, #48]");
    emitter.instruction("cmp x5, #1");
    emitter.instruction("b.eq __rt_hash_filter_incref");
    emitter.instruction("cmp x5, #4");
    emitter.instruction("b.lo __rt_hash_filter_no_incref"); // 0,2,3 = scalar
    emitter.instruction("cmp x5, #8");
    emitter.instruction("b.hs __rt_hash_filter_no_incref"); // 8=null, 9+, 10=callable
    emitter.label("__rt_hash_filter_incref");
    emitter.instruction("ldr x0, [sp, #32]"); // value_lo
    emitter.instruction("bl __rt_incref");
    emitter.label("__rt_hash_filter_no_incref");

    // -- __rt_hash_set(result, key_lo, key_hi, value_lo, value_hi, value_tag) (persists string keys) --
    emitter.instruction("mov x0, x24");
    emitter.instruction("ldr x1, [sp, #16]");
    emitter.instruction("ldr x2, [sp, #24]");
    emitter.instruction("ldr x3, [sp, #32]");
    emitter.instruction("ldr x4, [sp, #40]");
    emitter.instruction("ldr x5, [sp, #48]");
    emitter.instruction("bl __rt_hash_set");
    emitter.instruction("mov x24, x0"); // result may have grown

    emitter.label("__rt_hash_filter_skip");
    emitter.instruction("b __rt_hash_filter_loop");

    emitter.label("__rt_hash_filter_done");
    emitter.instruction("ldr x0, [sp, #0]"); // free the reusable value cell (raw block free)
    emitter.instruction("bl __rt_heap_free");
    emitter.instruction("ldr x0, [sp, #8]"); // free the reusable key cell
    emitter.instruction("bl __rt_heap_free");
    emitter.instruction("mov x0, x24"); // return the filtered hash
    emitter.instruction("ldp x23, x24, [sp, #64]");
    emitter.instruction("ldp x21, x22, [sp, #80]");
    emitter.instruction("ldp x19, x20, [sp, #96]");
    emitter.instruction("ldp x29, x30, [sp, #112]");
    emitter.instruction("add sp, sp, #128");
    emitter.instruction("ret");
}

/// Emits `__rt_hash_filter` for x86_64 Linux (SysV ABI). Behaviourally identical to the ARM64
/// variant. Note the input-register quirks: `__rt_heap_alloc`, `__rt_heap_free`, and `__rt_incref`
/// all take their argument in RAX, not RDI.
fn emit_hash_filter_linux_x86_64(emitter: &mut Emitter) {
    emitter.blank();
    emitter.comment("--- runtime: hash_filter ---");
    emitter.label_global("__rt_hash_filter");

    // callee-saved: rbx=callback, r12=source, r13=env, r14=mode, r15=cursor (all survive the nested
    // calls). rsp-relative slots: [rsp+0]=result [rsp+8]=valcell [rsp+16]=keycell [rsp+24]=key_ptr
    // [rsp+32]=key_len [rsp+40]=value_lo [rsp+48]=value_hi [rsp+56]=value_tag.
    emitter.instruction("push rbp");
    emitter.instruction("mov rbp, rsp");
    emitter.instruction("push rbx");
    emitter.instruction("push r12");
    emitter.instruction("push r13");
    emitter.instruction("push r14");
    emitter.instruction("push r15");
    emitter.instruction("sub rsp, 72"); // 8 slots + 16-byte-alignment padding for nested calls
    emitter.instruction("mov rbx, rdi"); // callback wrapper address
    emitter.instruction("mov r12, rsi"); // source hash pointer
    emitter.instruction("mov r13, rdx"); // optional callback environment pointer
    emitter.instruction("mov r14, rcx"); // mode

    emitter.instruction("mov rdi, QWORD PTR [r12]"); // source entry count
    emitter.instruction("mov rsi, 7"); // value_type 7 = mixed
    emitter.instruction("call __rt_hash_new");
    emitter.instruction("mov QWORD PTR [rsp + 0], rax"); // result hash pointer

    emitter.instruction("mov rax, 24"); // heap_alloc takes the size in RAX
    emitter.instruction("call __rt_heap_alloc");
    emitter.instruction(&format!("mov r10, 0x{:x}", X86_64_MIXED_CELL_KIND));
    emitter.instruction("mov QWORD PTR [rax - 8], r10"); // stamp value cell as boxed Mixed
    emitter.instruction("mov QWORD PTR [rsp + 8], rax"); // valcell
    emitter.instruction("mov rax, 24");
    emitter.instruction("call __rt_heap_alloc");
    emitter.instruction(&format!("mov r10, 0x{:x}", X86_64_MIXED_CELL_KIND));
    emitter.instruction("mov QWORD PTR [rax - 8], r10");
    emitter.instruction("mov QWORD PTR [rsp + 16], rax"); // keycell
    emitter.instruction("xor r15d, r15d"); // cursor = 0

    emitter.label("__rt_hash_filter_loop");
    emitter.instruction("mov rdi, r12");
    emitter.instruction("mov rsi, r15");
    emitter.instruction("call __rt_hash_iter_next"); // rax=cursor rdi=key_ptr rdx=key_len rcx=lo r8=hi r9=tag
    emitter.instruction("cmp rax, -1");
    emitter.instruction("je __rt_hash_filter_done");
    emitter.instruction("mov r15, rax"); // save next cursor
    emitter.instruction("mov QWORD PTR [rsp + 24], rdi"); // key_ptr
    emitter.instruction("mov QWORD PTR [rsp + 32], rdx"); // key_len
    emitter.instruction("mov QWORD PTR [rsp + 40], rcx"); // value_lo
    emitter.instruction("mov QWORD PTR [rsp + 48], r8"); // value_hi
    emitter.instruction("mov QWORD PTR [rsp + 56], r9"); // value_tag

    // -- build the value cell --
    emitter.instruction("mov r10, QWORD PTR [rsp + 8]");
    emitter.instruction("mov QWORD PTR [r10], r9");
    emitter.instruction("mov QWORD PTR [r10 + 8], rcx");
    emitter.instruction("mov QWORD PTR [r10 + 16], r8");

    // -- build the key cell --
    emitter.instruction("mov r10, QWORD PTR [rsp + 16]");
    emitter.instruction("mov r11, QWORD PTR [rsp + 32]"); // key_len
    emitter.instruction("cmp r11, -1");
    emitter.instruction("je __rt_hash_filter_key_int");
    emitter.instruction("mov QWORD PTR [r10], 1"); // tag 1 = string
    emitter.instruction("mov r11, QWORD PTR [rsp + 24]");
    emitter.instruction("mov QWORD PTR [r10 + 8], r11"); // lo = key_ptr
    emitter.instruction("mov r11, QWORD PTR [rsp + 32]");
    emitter.instruction("mov QWORD PTR [r10 + 16], r11"); // hi = key_len
    emitter.instruction("jmp __rt_hash_filter_key_done");
    emitter.label("__rt_hash_filter_key_int");
    emitter.instruction("mov QWORD PTR [r10], 0"); // tag 0 = integer
    emitter.instruction("mov r11, QWORD PTR [rsp + 24]");
    emitter.instruction("mov QWORD PTR [r10 + 8], r11");
    emitter.instruction("mov QWORD PTR [r10 + 16], 0");
    emitter.label("__rt_hash_filter_key_done");

    // -- invoke the predicate with mode-specific arguments --
    emitter.instruction("cmp r14, 2");
    emitter.instruction("je __rt_hash_filter_arg_key");
    emitter.instruction("cmp r14, 1");
    emitter.instruction("je __rt_hash_filter_arg_both");
    emitter.instruction("mov rdi, QWORD PTR [rsp + 8]"); // USE_VALUE
    emitter.instruction("test r13, r13");
    emitter.instruction("jz __rt_hash_filter_call");
    emitter.instruction("mov rsi, r13");
    emitter.instruction("jmp __rt_hash_filter_call");
    emitter.label("__rt_hash_filter_arg_key");
    emitter.instruction("mov rdi, QWORD PTR [rsp + 16]"); // USE_KEY
    emitter.instruction("test r13, r13");
    emitter.instruction("jz __rt_hash_filter_call");
    emitter.instruction("mov rsi, r13");
    emitter.instruction("jmp __rt_hash_filter_call");
    emitter.label("__rt_hash_filter_arg_both");
    emitter.instruction("mov rdi, QWORD PTR [rsp + 8]"); // USE_BOTH: value then key
    emitter.instruction("mov rsi, QWORD PTR [rsp + 16]");
    emitter.instruction("test r13, r13");
    emitter.instruction("jz __rt_hash_filter_call");
    emitter.instruction("mov rdx, r13");
    emitter.label("__rt_hash_filter_call");
    emitter.instruction("call rbx");
    emitter.instruction("test rax, rax");
    emitter.instruction("jz __rt_hash_filter_skip");

    // -- retain the kept value (refcounted tags 1,4,5,6,7) --
    emitter.instruction("mov r9, QWORD PTR [rsp + 56]");
    emitter.instruction("cmp r9, 1");
    emitter.instruction("je __rt_hash_filter_incref");
    emitter.instruction("cmp r9, 4");
    emitter.instruction("jb __rt_hash_filter_no_incref");
    emitter.instruction("cmp r9, 8");
    emitter.instruction("jae __rt_hash_filter_no_incref");
    emitter.label("__rt_hash_filter_incref");
    emitter.instruction("mov rax, QWORD PTR [rsp + 40]"); // incref takes its pointer in RAX
    emitter.instruction("call __rt_incref");
    emitter.label("__rt_hash_filter_no_incref");

    emitter.instruction("mov rdi, QWORD PTR [rsp + 0]"); // result
    emitter.instruction("mov rsi, QWORD PTR [rsp + 24]"); // key_lo
    emitter.instruction("mov rdx, QWORD PTR [rsp + 32]"); // key_hi
    emitter.instruction("mov rcx, QWORD PTR [rsp + 40]"); // value_lo
    emitter.instruction("mov r8, QWORD PTR [rsp + 48]"); // value_hi
    emitter.instruction("mov r9, QWORD PTR [rsp + 56]"); // value_tag
    emitter.instruction("call __rt_hash_set");
    emitter.instruction("mov QWORD PTR [rsp + 0], rax"); // result may have grown

    emitter.label("__rt_hash_filter_skip");
    emitter.instruction("jmp __rt_hash_filter_loop");

    emitter.label("__rt_hash_filter_done");
    emitter.instruction("mov rax, QWORD PTR [rsp + 8]"); // free value cell (heap_free takes RAX)
    emitter.instruction("call __rt_heap_free");
    emitter.instruction("mov rax, QWORD PTR [rsp + 16]"); // free key cell
    emitter.instruction("call __rt_heap_free");
    emitter.instruction("mov rax, QWORD PTR [rsp + 0]"); // return filtered hash
    emitter.instruction("add rsp, 72");
    emitter.instruction("pop r15");
    emitter.instruction("pop r14");
    emitter.instruction("pop r13");
    emitter.instruction("pop r12");
    emitter.instruction("pop rbx");
    emitter.instruction("pop rbp");
    emitter.instruction("ret");
}
