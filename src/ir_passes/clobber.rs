//! Purpose:
//! Classifies which EIR opcodes and terminators are "volatile-safe": their
//! `codegen` lowering emits no call and touches only the fixed result and
//! scratch registers, never the caller-saved registers the allocator may hand
//! out to non-call-crossing intervals.
//!
//! Called from:
//! - `crate::ir_passes::intervals` to mark each live interval as call-free.
//!
//! Key details:
//! - Safe-by-default in the correctness direction: the allowlist returns `true`
//!   only for opcodes whose lowering was audited to (a) emit no `bl`/`call`,
//!   (b) clobber a register set contained in {result, x9/x10/x11 and d0/d1 on
//!   AArch64; rax/rdx/rcx/r10/r11 and xmm0/xmm1 on x86_64}, and (c) load their
//!   SSA operands before using any scratch. Every other opcode falls through to
//!   `false`, so a forgotten opcode only loses an optimization opportunity — it
//!   can never place a live value in a register the lowering then clobbers.
//! - Because the caller-saved pools (`x12`–`x15`, `d16`–`d23`, `rsi`/`rdi`/`r8`/
//!   `r9`, `xmm2`–`xmm7`) are disjoint from every register the allowlisted
//!   lowerings touch, a value that lives only across allowlisted ops keeps its
//!   caller-saved register intact with no prologue save/restore.
//! - Branch edges with lifetime-tracked parameters are excluded because retaining
//!   a borrowed argument can call the runtime after other parameters are stored.

use crate::ir::{BlockId, Function, Op, Ownership, Terminator};

/// Returns true when `op`'s lowering neither emits a call nor touches a
/// caller-saved register outside the fixed result/scratch set.
///
/// Used to decide whether a live interval spanning this instruction can safely
/// keep a value in a caller-saved register. The default is `false`: only the
/// audited pure-compute opcodes below are treated as volatile-safe.
pub(super) fn op_is_volatile_safe(op: Op) -> bool {
    use Op::*;
    matches!(
        op,
        // Constants materialized directly into the result register.
        ConstI64 | ConstBool | ConstNull | ConstF64
        // Integer arithmetic and bitwise: result + secondary/tertiary scratch only
        // (see `lower_inst::arithmetic`). `IShl`, `IShrA`, `ISMod`, and `IDiv` are
        // deliberately absent: their PHP guards (negative shift count, zero divisor,
        // `PHP_INT_MIN % -1`) branch into `lower_inst::exceptions`, which allocates a
        // throwable and calls into the runtime unwinder. `FToI`, `IChecked*ToInt`, and
        // `ICheckedNumericChainToInt` are absent because their overflow/conversion paths
        // call `__rt_php_float_to_int` even though their EIR effects are semantically pure.
        | IAdd | ISub | IMul | INeg
        | IBitAnd | IBitOr | IBitXor | IBitNot
        // Floating-point arithmetic: d0/d1 or xmm0/xmm1 only (`lower_inst::floats`).
        // `FDiv` is excluded: its zero-divisor guard branches into an exception throw.
        | FAdd | FSub | FMul | FNeg
        // Integer/float comparisons: result + secondary scratch only.
        | ICmp | FCmp
        // Int-to-float promotion is still a single inline scvtf / cvtsi2sd.
        | IToF
        // A statement-boundary concat reset uses reserved x9/x10 or r10 scratch
        // registers and stores its value to `_concat_off`. PIC x86_64 preserves
        // its additional r11 symbol-address scratch around the store.
        | ConcatReset | Nop
        // Buffer element access through a hoisted descriptor: x9-x11 or r10/r11 plus the
        // result register (`lower_inst::buffers`). Its stale-handle and bounds faults jump to
        // runtime fatals that never return, so no call can observe a caller-saved home.
        | BufferGetResolved | BufferSetResolved
    )
}

/// Returns true when `term`'s lowering neither emits a call nor clobbers a
/// caller-saved register holding another live value.
///
/// An incoming lifetime-tracked parameter may need a runtime retain during
/// parallel edge-copy materialization, even when its source is only borrowed.
pub(super) fn edge_materialization_may_call(function: &Function, target: BlockId) -> bool {
    function.block(target).is_some_and(|block| {
        block.params.iter().any(|param| {
            function.value(*param).is_some_and(|value| {
                value.ownership == Ownership::MaybeOwned
                    && Ownership::php_type_needs_lifetime_tracking(&value.php_type)
            })
        })
    })
}

/// Returns true when lowering this terminator cannot clobber caller-saved homes.
///
/// Edge copies may retain a heap value after writing an earlier scalar parameter.
/// Throws, generator suspends, fatals, and unreachable are conservatively unsafe.
pub(super) fn terminator_is_volatile_safe(function: &Function, term: &Terminator) -> bool {
    match term {
        Terminator::Br { target, .. } => !edge_materialization_may_call(function, *target),
        Terminator::CondBr { then_target, else_target, .. } => {
            !edge_materialization_may_call(function, *then_target)
                && !edge_materialization_may_call(function, *else_target)
        }
        Terminator::Switch { cases, default, .. } => {
            !edge_materialization_may_call(function, *default)
                && cases.iter().all(|case| !edge_materialization_may_call(function, case.target))
        }
        Terminator::Return { .. } => true,
        _ => false,
    }
}
