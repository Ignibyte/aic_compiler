//! Purpose:
//! Emits the shared slice-length normalization used by every `array_slice`/`array_splice` runtime
//! helper: resolve the "to end" sentinel, fold a negative length into a count back from the end,
//! and clamp the result to the elements remaining after the normalized offset.
//!
//! Called from:
//! - `crate::codegen_support::runtime::arrays::array_slice{,_str,_refcounted}` and
//!   `array_splice{,_refcounted}`, each after computing its remaining-element count.
//!
//! Key details:
//! - PHP gives a negative `$length` a real meaning — it stops that many elements short of the end,
//!   so `array_slice([1,2,3,4,5], 0, -1)` is `[1,2,3,4]`. The helpers therefore cannot use `-1` as
//!   their "run to the end" marker; they take `ARRAY_SLICE_LENGTH_TO_END` (`i64::MIN`), the one
//!   value no legal PHP length can collide with.
//! - Emitted branchlessly on ARM64 (conditional selects) and with local labels on x86_64. Callers
//!   supply their own register assignment and a unique label prefix.

use crate::codegen_support::abi;
use crate::codegen_support::emit::Emitter;
use crate::codegen_support::platform::Arch;
use crate::codegen_support::sentinels::ARRAY_SLICE_LENGTH_TO_END;

/// Normalizes a requested slice length in `length_reg` against the remaining-element count in
/// `remaining_reg`, leaving the clamped length in `length_reg`.
///
/// Handles, in order: the `ARRAY_SLICE_LENGTH_TO_END` sentinel (becomes the full remainder), a
/// negative length (becomes `remaining + length`, floored at zero), and an over-long length
/// (clamped down to `remaining`).
///
/// `scratch_a`/`scratch_b` are clobbered. On x86_64 only `scratch_a` is used; `label_prefix` must
/// be unique per emitted helper because the generated labels are file-global.
pub(super) fn emit_slice_length_normalization(
    emitter: &mut Emitter,
    length_reg: &str,
    remaining_reg: &str,
    scratch_a: &str,
    scratch_b: &str,
    label_prefix: &str,
) {
    match emitter.target.arch {
        Arch::AArch64 => {
            abi::emit_load_int_immediate(emitter, scratch_a, ARRAY_SLICE_LENGTH_TO_END);
            emitter.instruction(&format!("cmp {}, {}", length_reg, scratch_a)); // is the requested length the run-to-the-end sentinel?
            emitter.instruction(&format!(
                // replace the sentinel with every element remaining after the normalized offset
                "csel {}, {}, {}, eq",
                length_reg, remaining_reg, length_reg
            ));
            emitter.instruction(&format!(
                // a negative length counts back from the end: remaining + length
                "add {}, {}, {}",
                scratch_b, length_reg, remaining_reg
            ));
            emitter.instruction(&format!("cmp {}, #0", scratch_b)); // did counting back from the end run past the start of the slice?
            emitter.instruction(&format!(
                // floor a back-counted length at zero so the slice cannot be negative
                "csel {}, xzr, {}, lt",
                scratch_b, scratch_b
            ));
            emitter.instruction(&format!("cmp {}, #0", length_reg)); // was the requested length actually negative?
            emitter.instruction(&format!(
                // adopt the back-counted length only for a negative request
                "csel {}, {}, {}, lt",
                length_reg, scratch_b, length_reg
            ));
            emitter.instruction(&format!("cmp {}, {}", length_reg, remaining_reg)); // does the length reach past the end of the source array?
            emitter.instruction(&format!(
                // clamp the length down to the remaining element count
                "csel {}, {}, {}, gt",
                length_reg, remaining_reg, length_reg
            ));
        }
        Arch::X86_64 => {
            let clamp_label = format!("{}_clamp", label_prefix);
            let ready_label = format!("{}_ready", label_prefix);
            abi::emit_load_int_immediate(emitter, scratch_a, ARRAY_SLICE_LENGTH_TO_END);
            emitter.instruction(&format!("cmp {}, {}", length_reg, scratch_a)); // is the requested length the run-to-the-end sentinel?
            emitter.instruction(&format!("jne {}_signed", label_prefix)); // keep an explicitly requested length for signed normalization
            emitter.instruction(&format!("mov {}, {}", length_reg, remaining_reg)); // replace the sentinel with every element remaining after the normalized offset
            emitter.instruction(&format!("jmp {}", clamp_label)); // a full remainder is already within bounds

            emitter.label(&format!("{}_signed", label_prefix));
            emitter.instruction(&format!("cmp {}, 0", length_reg)); // was the requested length negative?
            emitter.instruction(&format!("jge {}", clamp_label)); // a non-negative length only needs the upper clamp
            emitter.instruction(&format!("add {}, {}", length_reg, remaining_reg)); // a negative length counts back from the end: remaining + length
            emitter.instruction(&format!("cmp {}, 0", length_reg)); // did counting back from the end run past the start of the slice?
            emitter.instruction(&format!("jge {}", clamp_label)); // keep the back-counted length when it is still positive
            emitter.instruction(&format!("xor {}, {}", length_reg, length_reg)); // floor a back-counted length at zero so the slice cannot be negative

            emitter.label(&clamp_label);
            emitter.instruction(&format!("cmp {}, {}", length_reg, remaining_reg)); // does the length reach past the end of the source array?
            emitter.instruction(&format!("jle {}", ready_label)); // keep a length that already fits inside the remaining window
            emitter.instruction(&format!("mov {}, {}", length_reg, remaining_reg)); // clamp the length down to the remaining element count

            emitter.label(&ready_label);
        }
    }
}
