//! Purpose:
//! Owns the slot count of the runtime's per-descriptor tables (`_eof_flags`,
//! `_zstream_handles`, `_tls_sessions`, ...) and the bounds check emitted before
//! every lookup into them.
//!
//! Called from:
//! - `crate::codegen_support::runtime::data::fixed` to size each table.
//! - Runtime I/O helpers, stream-filter emitters, and the I/O builtin emitters
//!   that index those tables by raw descriptor.
//!
//! Key details:
//! - A descriptor at or past `FD_TABLE_SLOTS`, or a negative one, has no slot:
//!   the check is an unsigned compare, so negative values fall out of range too.
//! - A read past the bound means "nothing attached"; a write past it must not
//!   happen, and the caller reports failure the way its builtin already does.
//! - `_eof_flags` alone has `EOF_FLAG_SLOTS` (1 << 20) slots, Linux's default
//!   `fs.nr_open`, so every descriptor a default system can open records EOF and
//!   `feof()` turns true on it. Only a system with a raised `fs.nr_open` can open
//!   a descriptor past that bound, and there `feof()` still reads false.
//! - Long-lived processes (the `--web` pool handlers) reach descriptors past the
//!   old 256-slot tables, where an unchecked index read the next BSS symbol.

use crate::codegen_support::{emit::Emitter, platform::Arch};

/// Number of descriptor slots in every per-fd runtime table except `_eof_flags`.
pub(crate) const FD_TABLE_SLOTS: usize = 65536;

/// Number of one-byte slots in `_eof_flags`: Linux's default `fs.nr_open`, the
/// highest descriptor count a process can raise `RLIMIT_NOFILE` to by default.
pub(crate) const EOF_FLAG_SLOTS: usize = 1 << 20;

const _: () = assert!(aarch64_cmp_encodable(FD_TABLE_SLOTS) && aarch64_cmp_encodable(EOF_FLAG_SLOTS));

/// Returns whether `slots` fits the AArch64 `cmp` immediate form `#imm12, lsl #12`.
const fn aarch64_cmp_encodable(slots: usize) -> bool {
    slots % 4096 == 0 && (slots >> 12) < 4096
}

/// Returns the byte size of a per-fd table whose slots are `slot_width` bytes wide.
pub(crate) const fn fd_table_bytes(slot_width: usize) -> usize {
    FD_TABLE_SLOTS * slot_width
}

/// Emits a branch to `label` when the descriptor in `fd_reg` has no table slot.
///
/// `fd_reg` is a general register holding the raw descriptor (64- or 32-bit name).
/// The compare is unsigned, so a negative descriptor also branches. Clobbers only
/// the condition flags.
pub(crate) fn emit_branch_if_fd_out_of_table(emitter: &mut Emitter, fd_reg: &str, label: &str) {
    emit_branch_if_fd_at_or_past(emitter, fd_reg, FD_TABLE_SLOTS, label);
}

/// Emits a branch to `label` when the descriptor in `fd_reg` has no `_eof_flags` slot.
///
/// Same contract as `emit_branch_if_fd_out_of_table`, against `EOF_FLAG_SLOTS`.
pub(crate) fn emit_branch_if_fd_out_of_eof_flags(emitter: &mut Emitter, fd_reg: &str, label: &str) {
    emit_branch_if_fd_at_or_past(emitter, fd_reg, EOF_FLAG_SLOTS, label);
}

/// Emits the unsigned `fd >= slots` compare and branch shared by both table bounds.
///
/// `slots` must satisfy `aarch64_cmp_encodable`, which the module's const assert
/// checks for every bound passed here.
fn emit_branch_if_fd_at_or_past(emitter: &mut Emitter, fd_reg: &str, slots: usize, label: &str) {
    match emitter.target.arch {
        Arch::AArch64 => {
            emitter.instruction(&format!("cmp {}, #{}, lsl #12", fd_reg, slots >> 12)); // compare the descriptor with the table's slot count
            emitter.instruction(&format!("b.hs {}", label));                    // no table slot for this descriptor (unsigned: negatives too)
        }
        Arch::X86_64 => {
            emitter.instruction(&format!("cmp {}, {}", fd_reg, slots));         // compare the descriptor with the table's slot count
            emitter.instruction(&format!("jae {}", label));                     // no table slot for this descriptor (unsigned: negatives too)
        }
    }
}
