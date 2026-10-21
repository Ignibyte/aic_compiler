//! Purpose:
//! Hoists generation-safe `buffer<T>` handle resolution out of loops that cannot invalidate
//! a buffer, and shares one bounds-checked record address between the repeated `$b[$i]`
//! packed accesses of an iteration.
//!
//! Called from:
//! - `crate::ir_passes::driver::optimize_module`, once after the ordinary fixed point, so
//!   the follow-up function passes clean up the loads the rewrite leaves unused.
//!
//! Key details:
//! - Every `buffer_get`/`buffer_set` resolves its handle through `__rt_buffer_resolve`
//!   (registry lookup, active marker, generation) before its bounds check. Inside a loop that
//!   runs no instruction able to free, allocate or rebind a buffer, that answer cannot change
//!   between iterations, so one `buffer_resolve` in the preheader serves every access, which
//!   becomes `buffer_get_resolved`/`buffer_set_resolved`.
//! - The hoisted resolution is NON-FAULTING: it answers 0 for a stale handle and the access
//!   tests the descriptor, so the use-after-free fatal still fires at the access itself and a
//!   loop that never runs its body never reports anything, exactly as before.
//! - A loop qualifies only when every instruction in its natural body is on a default-deny
//!   allowlist: buffer element and packed field accesses, local loads, stores of scalar
//!   locals, concat resets, and instructions whose effects only read state or fatal. That
//!   includes a user function or method call whose refined effect summary is read-only. Any
//!   other call, and releases, allocations, warnings (user error handlers), output
//!   (output-buffer callbacks) and throws disqualify it, because each can run user code that
//!   frees a buffer through an alias. Blocks that leave the loop for good are outside the natural body, and coming
//!   back always passes the preheader, which resolves again.
//! - The buffer operand must be loop-invariant: an SSA value defined outside the loop, or a
//!   `load_local` of a slot nothing in the loop writes and no by-reference alias can reach.
//! - Inside a rewritten loop, packed `buffer_get_resolved` instructions with the same
//!   descriptor and index compute the same record address, so a dominated one is replaced by
//!   its dominator. Scalar element reads are left alone: a `buffer_set_resolved` between two of
//!   them can change the loaded value.
//! - Skipped entirely for generators (a `yield` lets the caller run), functions with exception
//!   handlers or dynamic `eval`, and modules that enable `pcntl_async_signals()`, whose backend
//!   dispatches signal handlers after every instruction.

use std::collections::{HashMap, HashSet};

use crate::ir::{
    BlockId, Effects, Function, Immediate, InstId, Instruction, IrType, LocalSlotId, Module, Op,
    Ownership, PassOrigin, Value, ValueDef, ValueId,
};
use crate::types::PhpType;

use super::cfg::has_exception_handlers;
use super::dominance::{compute_dominance, DominanceInfo};
use super::loops::{compute_loops, NaturalLoop};
use super::rewrite::{neutralize_to_nop, replace_all_uses, resolve_chains};

/// Rewrites buffer accesses in every function-like body and reports whether anything changed.
pub(super) fn hoist_buffer_resolution(module: &mut Module) -> bool {
    if module.uses_pcntl_runtime(crate::ir::PcntlRuntime::AsyncSignals) {
        return false;
    }
    let Module {
        functions,
        class_methods,
        closures,
        fiber_wrappers,
        callback_wrappers,
        extern_callback_trampolines,
        runtime_callable_invokers,
        ..
    } = module;
    let mut changed = false;
    for function in functions
        .iter_mut()
        .chain(class_methods.iter_mut())
        .chain(closures.iter_mut())
        .chain(fiber_wrappers.iter_mut())
        .chain(callback_wrappers.iter_mut())
        .chain(extern_callback_trampolines.iter_mut())
        .chain(runtime_callable_invokers.iter_mut())
    {
        changed |= optimize_function(function);
    }
    changed
}

/// Hoists buffer resolution out of every qualifying loop of one function, outermost first.
fn optimize_function(function: &mut Function) -> bool {
    if !function
        .instructions
        .iter()
        .any(|inst| matches!(inst.op, Op::BufferGet | Op::BufferSet))
        || function.flags.is_generator
        || has_exception_handlers(function)
        || super::local_scope::has_dynamic_eval(function)
        || function.instructions.iter().any(|inst| inst.op.name().starts_with("eval_"))
    {
        return false;
    }
    let dominance = compute_dominance(function);
    let loops = compute_loops(function, &dominance);
    if loops.is_empty() {
        return false;
    }
    let escaping = super::by_ref_alias::address_escaping_slots(function);
    let mut order: Vec<usize> = (0..loops.loops().len()).collect();
    order.sort_by_key(|&index| loops.loops()[index].depth);
    let mut changed = false;
    for index in order {
        let loop_ref = &loops.loops()[index];
        let Some(preheader) = loop_ref.preheader else {
            continue;
        };
        if !loop_preserves_buffers(function, loop_ref) {
            continue;
        }
        changed |= hoist_loop(function, loop_ref, preheader, &dominance, &escaping);
    }
    changed
}

/// Where a loop-invariant buffer handle comes from.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum HandleSource {
    /// An SSA value defined outside the loop.
    Value(ValueId),
    /// A local slot that nothing in the loop writes.
    Slot(LocalSlotId),
}

/// Rewrites one loop's invariant buffer accesses through preheader descriptors.
fn hoist_loop(
    function: &mut Function,
    loop_ref: &NaturalLoop,
    preheader: BlockId,
    dominance: &DominanceInfo,
    escaping: &HashSet<LocalSlotId>,
) -> bool {
    let written = slots_named_in_loop(function, loop_ref);
    let mut groups: Vec<(HandleSource, Vec<InstId>)> = Vec::new();
    let mut replaced_loads: Vec<ValueId> = Vec::new();
    for &block in &loop_ref.blocks {
        for &inst_id in &function.blocks[block.as_raw() as usize].instructions {
            let inst = &function.instructions[inst_id.as_raw() as usize];
            if !matches!(inst.op, Op::BufferGet | Op::BufferSet) {
                continue;
            }
            let Some(&handle) = inst.operands.first() else {
                continue;
            };
            let Some(source) = handle_source(function, loop_ref, handle, &written, escaping) else {
                continue;
            };
            if matches!(source, HandleSource::Slot(_)) {
                replaced_loads.push(handle);
            }
            match groups.iter_mut().find(|(existing, _)| *existing == source) {
                Some((_, accesses)) => accesses.push(inst_id),
                None => groups.push((source, vec![inst_id])),
            }
        }
    }
    if groups.is_empty() {
        return false;
    }

    let mut rewritten = Vec::new();
    for (source, accesses) in groups {
        let handle = match source {
            HandleSource::Value(value) => value,
            HandleSource::Slot(slot) => {
                let template = function.instructions[accesses[0].as_raw() as usize].operands[0];
                append_slot_load(function, preheader, slot, template)
            }
        };
        let descriptor = append_instruction(
            function,
            preheader,
            Op::BufferResolve,
            vec![handle],
            IrType::I64,
            PhpType::Int,
        );
        for inst_id in accesses {
            let inst = &mut function.instructions[inst_id.as_raw() as usize];
            inst.op = if inst.op == Op::BufferGet {
                Op::BufferGetResolved
            } else {
                Op::BufferSetResolved
            };
            inst.operands[0] = descriptor;
            inst.effects = inst.op.default_effects();
            inst.origin = Some(PassOrigin::BufferAccess);
            rewritten.push(inst_id);
        }
    }
    neutralize_unused_loads(function, &replaced_loads);
    share_record_addresses(function, &rewritten, dominance);
    true
}

/// Returns whether no instruction in the loop body can free, allocate, or rebind a buffer.
fn loop_preserves_buffers(function: &Function, loop_ref: &NaturalLoop) -> bool {
    loop_ref.blocks.iter().all(|&block| {
        function.blocks[block.as_raw() as usize]
            .instructions
            .iter()
            .all(|&inst_id| instruction_preserves_buffers(function, &function.instructions[inst_id.as_raw() as usize]))
    })
}

/// Default-deny classification of one loop instruction.
///
/// Anything able to run user code (a destructor, an error handler, an output callback, an
/// unwinding throw), allocate, or write local or heap state other than a scalar local, a
/// buffer element or a packed field disqualifies the loop. A call passes only through the
/// effects check: effect refinement gives a direct user call its callee's summary, so a callee
/// proven to only read state or fatal cannot free a buffer either.
fn instruction_preserves_buffers(function: &Function, inst: &Instruction) -> bool {
    match inst.op {
        Op::Nop
        | Op::ConcatReset
        | Op::BufferGet
        | Op::BufferSet
        | Op::BufferResolve
        | Op::BufferGetResolved
        | Op::BufferSetResolved => true,
        Op::StoreLocal => store_targets_scalar_local(function, inst),
        Op::PropGet | Op::PropSet if function.is_packed_field_access(inst) => true,
        _ => inst
            .effects
            .difference(Effects::READS_LOCAL | Effects::READS_HEAP | Effects::READS_GLOBAL | Effects::MAY_FATAL)
            .is_empty(),
    }
}

/// Returns whether a `store_local` writes a scalar local, which cannot release anything.
fn store_targets_scalar_local(function: &Function, inst: &Instruction) -> bool {
    let Some(Immediate::LocalSlot(slot)) = inst.immediate else {
        return false;
    };
    function.locals.get(slot.as_raw() as usize).is_some_and(|local| {
        matches!(
            local.php_type.codegen_repr(),
            PhpType::Int | PhpType::Float | PhpType::Bool | PhpType::Pointer(_)
        )
    })
}

/// Collects every local slot an instruction in the loop names other than through a load.
fn slots_named_in_loop(function: &Function, loop_ref: &NaturalLoop) -> HashSet<LocalSlotId> {
    let mut written = HashSet::new();
    for &block in &loop_ref.blocks {
        for &inst_id in &function.blocks[block.as_raw() as usize].instructions {
            let inst = &function.instructions[inst_id.as_raw() as usize];
            match &inst.immediate {
                Some(Immediate::LocalSlot(slot)) if inst.op != Op::LoadLocal => {
                    written.insert(*slot);
                }
                Some(Immediate::LocalSlotPair { first, second }) => {
                    written.insert(*first);
                    written.insert(*second);
                }
                Some(Immediate::IterStart(metadata)) => written.extend(metadata.local_slots()),
                _ => {}
            }
        }
    }
    written
}

/// Classifies a buffer access's handle operand as loop-invariant, or `None` when it may vary.
fn handle_source(
    function: &Function,
    loop_ref: &NaturalLoop,
    handle: ValueId,
    written: &HashSet<LocalSlotId>,
    escaping: &HashSet<LocalSlotId>,
) -> Option<HandleSource> {
    match function.value(handle)?.def {
        ValueDef::BlockParam { block, .. } => {
            (!loop_ref.contains(block)).then_some(HandleSource::Value(handle))
        }
        ValueDef::Instruction { block, inst, .. } => {
            if !loop_ref.contains(block) {
                return Some(HandleSource::Value(handle));
            }
            let load = function.instruction(inst)?;
            let Some(Immediate::LocalSlot(slot)) = load.immediate else {
                return None;
            };
            let read_only = load.effects.difference(Effects::READS_LOCAL).is_empty();
            (load.op == Op::LoadLocal
                && read_only
                && !written.contains(&slot)
                && !escaping.contains(&slot))
            .then_some(HandleSource::Slot(slot))
        }
    }
}

/// Appends a copy of an in-loop handle load to the preheader and returns its value.
fn append_slot_load(
    function: &mut Function,
    preheader: BlockId,
    slot: LocalSlotId,
    template: ValueId,
) -> ValueId {
    let (ir_type, php_type, ownership, effects) = {
        let value = &function.values[template.as_raw() as usize];
        let ValueDef::Instruction { inst, .. } = value.def else {
            unreachable!("slot handles are defined by load_local");
        };
        let load = &function.instructions[inst.as_raw() as usize];
        (value.ir_type, value.php_type.clone(), value.ownership, load.effects)
    };
    let result = append_instruction(function, preheader, Op::LoadLocal, Vec::new(), ir_type, php_type);
    function.values[result.as_raw() as usize].ownership = ownership;
    let ValueDef::Instruction { inst, .. } = function.values[result.as_raw() as usize].def else {
        unreachable!("appended instructions define their result");
    };
    let load = &mut function.instructions[inst.as_raw() as usize];
    load.immediate = Some(Immediate::LocalSlot(slot));
    load.result_ownership = ownership;
    load.effects = effects;
    result
}

/// Appends a result-producing instruction to a block before its terminator.
fn append_instruction(
    function: &mut Function,
    block: BlockId,
    op: Op,
    operands: Vec<ValueId>,
    ir_type: IrType,
    php_type: PhpType,
) -> ValueId {
    let inst_id = InstId::from_raw(function.instructions.len() as u32);
    let result = ValueId::from_raw(function.values.len() as u32);
    let position = function.blocks[block.as_raw() as usize].instructions.len();
    function.values.push(Value {
        ir_type,
        php_type: php_type.clone(),
        def: ValueDef::Instruction { block, index: position as u32, inst: inst_id },
        ownership: Ownership::NonHeap,
    });
    let mut inst = Instruction::new(
        op,
        operands,
        None,
        Some(result),
        ir_type,
        php_type,
        Ownership::NonHeap,
        op.default_effects(),
        None,
    );
    inst.origin = Some(PassOrigin::BufferAccess);
    function.instructions.push(inst);
    function.blocks[block.as_raw() as usize].instructions.push(inst_id);
    result
}

/// Neutralizes in-loop handle loads that no instruction or terminator reads any more.
fn neutralize_unused_loads(function: &mut Function, loads: &[ValueId]) {
    let uses = function.value_use_counts();
    for &value in loads {
        if uses.get(&value).copied().unwrap_or(0) > 0 {
            continue;
        }
        if let ValueDef::Instruction { inst, .. } = function.values[value.as_raw() as usize].def {
            neutralize_to_nop(&mut function.instructions[inst.as_raw() as usize]);
        }
    }
}

/// Replaces each packed record address that a dominating access with the same descriptor and
/// index already computed.
fn share_record_addresses(function: &mut Function, rewritten: &[InstId], dominance: &DominanceInfo) {
    let mut positions: HashMap<InstId, (BlockId, usize)> = HashMap::new();
    for block in &function.blocks {
        for (index, &inst_id) in block.instructions.iter().enumerate() {
            positions.insert(inst_id, (block.id, index));
        }
    }
    let incoming = incoming_arguments(function);
    let mut groups: HashMap<(ValueId, ValueId), Vec<InstId>> = HashMap::new();
    for &inst_id in rewritten {
        let inst = &function.instructions[inst_id.as_raw() as usize];
        if inst.op != Op::BufferGetResolved
            || !matches!(inst.result_php_type.codegen_repr(), PhpType::Packed(_))
        {
            continue;
        }
        let index = canonical_value(function, &incoming, inst.operands[1]);
        groups.entry((inst.operands[0], index)).or_default().push(inst_id);
    }
    let dominates = |earlier: InstId, later: InstId| {
        let (earlier_block, earlier_index) = positions[&earlier];
        let (later_block, later_index) = positions[&later];
        if earlier_block == later_block {
            earlier_index < later_index
        } else {
            dominance.strictly_dominates(earlier_block, later_block)
        }
    };
    let mut replacements = HashMap::new();
    let mut redundant = Vec::new();
    for accesses in groups.values() {
        for &later in accesses {
            let Some(&earlier) = accesses.iter().find(|&&earlier| earlier != later && dominates(earlier, later)) else {
                continue;
            };
            let (Some(old), Some(new)) = (
                function.instructions[later.as_raw() as usize].result,
                function.instructions[earlier.as_raw() as usize].result,
            ) else {
                continue;
            };
            replacements.insert(old, new);
            redundant.push(later);
        }
    }
    if replacements.is_empty() {
        return;
    }
    replace_all_uses(function, &resolve_chains(&replacements));
    for inst_id in redundant {
        neutralize_to_nop(&mut function.instructions[inst_id.as_raw() as usize]);
    }
}

/// Collects, for every block parameter, the arguments each incoming edge passes to it.
fn incoming_arguments(function: &Function) -> HashMap<(BlockId, usize), Vec<ValueId>> {
    let mut incoming: HashMap<(BlockId, usize), Vec<ValueId>> = HashMap::new();
    for block in &function.blocks {
        let Some(terminator) = block.terminator.as_ref() else {
            continue;
        };
        for (target, args) in edge_arguments(terminator) {
            for (index, &arg) in args.iter().enumerate() {
                incoming.entry((target, index)).or_default().push(arg);
            }
        }
    }
    incoming
}

/// Returns each explicit successor of a terminator with the arguments it passes.
fn edge_arguments(terminator: &crate::ir::Terminator) -> Vec<(BlockId, &[ValueId])> {
    use crate::ir::Terminator;
    match terminator {
        Terminator::Br { target, args } => vec![(*target, args.as_slice())],
        Terminator::CondBr { then_target, then_args, else_target, else_args, .. } => {
            vec![(*then_target, then_args.as_slice()), (*else_target, else_args.as_slice())]
        }
        Terminator::Switch { cases, default, default_args, .. } => {
            let mut edges: Vec<_> = cases
                .iter()
                .map(|case| (case.target, case.args.as_slice()))
                .collect();
            edges.push((*default, default_args.as_slice()));
            edges
        }
        Terminator::GeneratorSuspend { resume, resume_args, .. } => {
            vec![(*resume, resume_args.as_slice())]
        }
        Terminator::Return { .. }
        | Terminator::Throw { .. }
        | Terminator::Fatal { .. }
        | Terminator::Unreachable => Vec::new(),
    }
}

/// Follows block parameters whose every incoming edge passes one and the same value.
///
/// Scalar promotion places a parameter at each join a promoted local is live into, even when
/// no path changes it, so the loop index reaches the second half of an iteration as a fresh
/// parameter equal to the header's. Seeing through such trivial parameters lets two accesses
/// of `$b[$i]` on either side of an `if` share one record address.
fn canonical_value(
    function: &Function,
    incoming: &HashMap<(BlockId, usize), Vec<ValueId>>,
    value: ValueId,
) -> ValueId {
    let mut current = value;
    for _ in 0..function.values.len() {
        let Some(ValueDef::BlockParam { block, index }) = function.value(current).map(|value| value.def) else {
            break;
        };
        let Some(args) = incoming.get(&(block, index as usize)) else {
            break;
        };
        let mut distinct = args.iter().copied().filter(|&arg| arg != current);
        let Some(first) = distinct.next() else {
            break;
        };
        if distinct.any(|arg| arg != first) {
            break;
        }
        current = first;
    }
    current
}
