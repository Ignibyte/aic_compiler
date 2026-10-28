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
//! - Long-lived processes (the `--web` pool handlers) reach descriptors past the
//!   old 256-slot tables, where an unchecked index read the next BSS symbol.

use crate::codegen_support::{emit::Emitter, platform::Arch};

/// Number of descriptor slots in every per-fd runtime table.
pub(crate) const FD_TABLE_SLOTS: usize = 65536;

/// `FD_TABLE_SLOTS` as the AArch64 `cmp` immediate, which is 12 bits shifted by 12.
const FD_TABLE_SLOTS_SHIFTED: usize = FD_TABLE_SLOTS >> 12;

const _: () = assert!(FD_TABLE_SLOTS % 4096 == 0 && FD_TABLE_SLOTS_SHIFTED < 4096);

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
    match emitter.target.arch {
        Arch::AArch64 => {
            emitter.instruction(&format!("cmp {}, #{}, lsl #12", fd_reg, FD_TABLE_SLOTS_SHIFTED)); // compare the descriptor with the per-fd table slot count
            emitter.instruction(&format!("b.hs {}", label));                    // no table slot for this descriptor (unsigned: negatives too)
        }
        Arch::X86_64 => {
            emitter.instruction(&format!("cmp {}, {}", fd_reg, FD_TABLE_SLOTS)); // compare the descriptor with the per-fd table slot count
            emitter.instruction(&format!("jae {}", label));                     // no table slot for this descriptor (unsigned: negatives too)
        }
    }
}
