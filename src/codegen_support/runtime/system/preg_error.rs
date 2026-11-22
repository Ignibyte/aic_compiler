//! Purpose:
//! Owns PHP's PCRE error state: the `_preg_last_error` code the regex helpers store,
//! the `preg_last_error_msg()` message table, and its `__rt_preg_last_error_msg` helper.
//!
//! Called from:
//! - `crate::codegen_support::runtime::system` regex helpers (`preg_match` and friends).
//! - `crate::codegen_support::runtime::emitters::emit_runtime()` for the message helper.
//! - `crate::codegen_support::runtime::data::fixed` for the message table.
//!
//! Key details:
//! - Every regex helper stores `PREG_NO_ERROR` on entry, `PREG_INTERNAL_ERROR` when the
//!   pattern is rejected, and `PREG_BAD_UTF8_ERROR` when a `/u` subject is not valid UTF-8.
//! - PCRE2 checks UTF-8 before it matches. `pcre2_regexec()` folds every UTF-8 error code
//!   into `REG_INVARG`, and so does the shim's `REG_STARTEND` path, so the helpers read
//!   that status on their first probe of a `/u` pattern as the bad-UTF-8 case.
//! - Only the first probe of a call reports malformed UTF-8; PCRE2 checks the whole
//!   subject there, so a later probe keeps its "no further match" reading.

use crate::codegen_support::{abi, emit::Emitter, platform::Arch};

/// `PREG_NO_ERROR`: the last regex call succeeded.
pub(super) const PREG_NO_ERROR: i64 = 0;
/// `PREG_INTERNAL_ERROR`: the pattern failed to compile or used a rejected modifier.
pub(super) const PREG_INTERNAL_ERROR: i64 = 1;
/// `PREG_BAD_UTF8_ERROR`: a `/u` subject is not valid UTF-8.
pub(super) const PREG_BAD_UTF8_ERROR: i64 = 4;
/// pcre2posix `REG_INVARG`, the exec status for a UTF-8 check failure.
const PCRE2_REG_INVARG: i64 = 16;
/// Bit of pcre2posix `REG_UTF` in the compile flags `__rt_preg_strip` returns for `/u`.
const PCRE2_REG_UTF_BIT: u32 = 6;

/// PHP's `preg_last_error_msg()` text, indexed by `PREG_*_ERROR` code.
const PREG_ERROR_MESSAGES: &[&str] = &[
    "No error",
    "Internal error",
    "Backtrack limit exhausted",
    "Recursion limit exhausted",
    "Malformed UTF-8 characters, possibly incorrectly encoded",
    "The offset did not correspond to the beginning of a valid UTF-8 code point",
    "JIT stack limit exhausted",
];

/// Stores one `PREG_*_ERROR` code into `_preg_last_error`.
///
/// AArch64 clobbers `x9` and `x10`; x86_64 clobbers no general register.
pub(super) fn emit_set_preg_last_error(emitter: &mut Emitter, code: i64) {
    abi::emit_store_imm_to_symbol(emitter, "_preg_last_error", 0, code);
}

/// Branches to `otherwise` unless the exec status names a UTF-8 rejection of a `/u` subject.
///
/// The 32-bit exec status is in `w0`/`eax`; `flags_off` is the stack slot that holds the
/// compile flags from `__rt_preg_strip`. Falls through on the bad-UTF-8 case. Leaves the
/// status register intact; AArch64 clobbers `x9`.
pub(super) fn emit_branch_unless_bad_utf8(emitter: &mut Emitter, flags_off: usize, otherwise: &str) {
    match emitter.target.arch {
        Arch::AArch64 => {
            emitter.instruction(&format!("cmp w0, #{}", PCRE2_REG_INVARG));     // did PCRE2 reject the subject before matching?
            emitter.instruction(&format!("b.ne {}", otherwise));                // any other status keeps its existing meaning
            emitter.instruction(&format!("ldr x9, [sp, #{}]", flags_off));      // reload the compile flags from delimiter parsing
            emitter.instruction(&format!("tbz x9, #{}, {}", PCRE2_REG_UTF_BIT, otherwise)); // only a /u pattern reports malformed UTF-8
        }
        Arch::X86_64 => {
            emitter.instruction(&format!("cmp eax, {}", PCRE2_REG_INVARG));     // did PCRE2 reject the subject before matching?
            emitter.instruction(&format!("jne {}", otherwise));                 // any other status keeps its existing meaning
            emitter.instruction(&format!("test BYTE PTR [rsp + {}], {}", flags_off, 1 << PCRE2_REG_UTF_BIT)); // was the pattern compiled for /u?
            emitter.instruction(&format!("jz {}", otherwise));                  // only a /u pattern reports malformed UTF-8
        }
    }
}

/// Emits the `preg_last_error_msg()` message strings and their (ptr, len) table.
pub(crate) fn emit_preg_error_data() -> String {
    let mut out = String::new();
    for (code, message) in PREG_ERROR_MESSAGES.iter().enumerate() {
        out.push_str(&format!(
            ".globl _preg_err_msg_{code}\n_preg_err_msg_{code}:\n    .ascii \"{message}\"\n"
        ));
    }
    out.push_str("    .balign 8\n");
    out.push_str(".globl _preg_err_msg_table\n_preg_err_msg_table:\n");
    for (code, message) in PREG_ERROR_MESSAGES.iter().enumerate() {
        out.push_str(&format!("    .quad _preg_err_msg_{code}\n    .quad {}\n", message.len()));
    }
    out
}

/// __rt_preg_last_error_msg: returns PHP's message for the `_preg_last_error` code.
///
/// Output ABI: AArch64 x1 = string ptr, x2 = string len; x86_64 rax = ptr, rdx = len.
/// An out-of-range code returns the `PREG_NO_ERROR` message.
pub(crate) fn emit_preg_last_error_msg(emitter: &mut Emitter) {
    let count = PREG_ERROR_MESSAGES.len();
    emitter.blank();
    emitter.comment("--- runtime: preg_last_error_msg ---");
    emitter.label_global("__rt_preg_last_error_msg");
    match emitter.target.arch {
        Arch::AArch64 => {
            abi::emit_load_symbol_to_reg(emitter, "x10", "_preg_last_error", 0);
            emitter.instruction(&format!("cmp x10, #{}", count));               // is the stored code inside the message table?
            emitter.instruction("b.lo __rt_preg_last_error_msg_index");         // index the table for a known code
            emitter.instruction(&format!("mov x10, #{}", PREG_NO_ERROR));       // fall back to the PREG_NO_ERROR message
            emitter.label("__rt_preg_last_error_msg_index");
            abi::emit_symbol_address(emitter, "x9", "_preg_err_msg_table");
            emitter.instruction("lsl x10, x10, #4");                            // step over one 16-byte (ptr, len) pair per code
            emitter.instruction("add x9, x9, x10");                             // address the entry for the stored code
            emitter.instruction("ldr x1, [x9]");                                // return the message pointer
            emitter.instruction("ldr x2, [x9, #8]");                            // return the message length
            emitter.instruction("ret");                                         // return the borrowed message slice
        }
        Arch::X86_64 => {
            abi::emit_load_symbol_to_reg(emitter, "rcx", "_preg_last_error", 0);
            emitter.instruction(&format!("cmp rcx, {}", count));                // is the stored code inside the message table?
            emitter.instruction("jb __rt_preg_last_error_msg_index_x86_64");    // index the table for a known code
            emitter.instruction(&format!("mov ecx, {}", PREG_NO_ERROR));        // fall back to the PREG_NO_ERROR message
            emitter.label("__rt_preg_last_error_msg_index_x86_64");
            abi::emit_symbol_address(emitter, "r9", "_preg_err_msg_table");
            emitter.instruction("shl rcx, 4");                                  // step over one 16-byte (ptr, len) pair per code
            emitter.instruction("add r9, rcx");                                 // address the entry for the stored code
            emitter.instruction("mov rax, QWORD PTR [r9]");                     // return the message pointer
            emitter.instruction("mov rdx, QWORD PTR [r9 + 8]");                 // return the message length
            emitter.instruction("ret");                                         // return the borrowed message slice
        }
    }
}
