//! Purpose:
//! Proves which checked numeric results may use a frame-local Mixed cell.
//!
//! Called from:
//! - Frame layout before any generated instruction is emitted.
//!
//! Key details:
//! - Only numeric casts and one ordinary local may observe the cell.
//! - Pointer reads cannot survive another execution of the producing instruction.
//! - References, eval, suspension, unknown consumers and escaping values fail closed.

use crate::ir::{
    Function, Immediate, InstId, IrHeapKind, IrType, LocalKind, LocalSlotId, Op, Ownership,
    ValueDef, ValueId,
};
use crate::types::PhpType;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;

/// Stack cells and the exclusively numeric locals proven while selecting them.
#[derive(Default)]
pub(super) struct NumericFrame {
    pub(super) cells: Vec<ValueId>,
    pub(super) locals: HashSet<LocalSlotId>,
}

/// Returns checked producers whose complete pointer lifetime stays inside their frame.
pub(super) fn plan(function: &Function) -> NumericFrame {
    if !function.instructions.iter().any(|inst| checked(inst.op)) {
        return NumericFrame::default();
    }
    if function.flags.is_main
        || function.flags.is_generator
        || function.flags.is_fiber_wrapper
        || function.locals.iter().any(|local| {
            matches!(
                local.kind,
                LocalKind::RefCell
                    | LocalKind::ReturnRefCell
                    | LocalKind::EvalContext
                    | LocalKind::EvalScope
                    | LocalKind::EvalGlobalScope
                    | LocalKind::GeneratorState
            )
        })
    {
        return NumericFrame::default();
    }
    let mut uses: HashMap<ValueId, Vec<InstId>> = HashMap::new();
    let mut terminator_uses = HashSet::new();
    let mut locations = HashMap::new();
    let mut accesses: HashMap<LocalSlotId, Vec<InstId>> = HashMap::new();
    for block in &function.blocks {
        let mut epoch = 0;
        for (index, &id) in block.instructions.iter().enumerate() {
            let inst = &function.instructions[id.as_raw() as usize];
            if checked(inst.op) {
                epoch += 1;
            }
            locations.insert(id, (block.id, index, epoch));
            match &inst.immediate {
                Some(Immediate::LocalSlot(slot)) => accesses.entry(*slot).or_default().push(id),
                Some(Immediate::LocalSlotPair { first, second }) => {
                    accesses.entry(*first).or_default().push(id);
                    accesses.entry(*second).or_default().push(id);
                }
                Some(Immediate::IterStart(iter)) => {
                    for slot in iter.local_slots() {
                        accesses.entry(slot).or_default().push(id);
                    }
                }
                _ => {}
            }
            for &value in &function.instructions[id.as_raw() as usize].operands {
                uses.entry(value).or_default().push(id);
            }
        }
        if let Some(term) = &block.terminator {
            terminator_uses.extend(term.used_values());
        }
    }
    let mut proof = Proof {
        function,
        uses,
        terminator_uses,
        locations,
        accesses,
        slots: HashMap::new(),
    };
    let cells = function
        .instructions
        .iter()
        .filter(|inst| checked(inst.op))
        .filter_map(|inst| inst.result)
        .filter(|&value| proof.producer(value))
        .collect();
    NumericFrame {
        cells,
        locals: proof
            .slots
            .into_iter()
            .filter_map(|(slot, safe)| safe.then_some(slot))
            .collect(),
    }
}

/// Shared use graph and cached whole-slot proofs for one function.
struct Proof<'a> {
    function: &'a Function,
    uses: HashMap<ValueId, Vec<InstId>>,
    terminator_uses: HashSet<ValueId>,
    slots: HashMap<LocalSlotId, bool>,
    locations: HashMap<InstId, (crate::ir::BlockId, usize, usize)>,
    accesses: HashMap<LocalSlotId, Vec<InstId>>,
}

impl Proof<'_> {
    /// Requires an owned numeric producer with an independently proven scalar-input ABI.
    fn producer(&mut self, value: ValueId) -> bool {
        let Some((id, definition)) = definition(self.function, value) else {
            return false;
        };
        if !checked(definition.op)
            || definition.result_type != IrType::Heap(IrHeapKind::Mixed)
            || definition.result_php_type != PhpType::Mixed
            || definition.result_ownership != Ownership::Owned
            || definition.operands.len() != 2
            || definition.operands.iter().any(|&input| {
                self.function.value(input).is_none_or(|input| {
                    input.ir_type != IrType::I64 || input.php_type != PhpType::Int
                })
            })
        {
            return false;
        }
        let mut stores = HashSet::new();
        if !self.value_uses(value, id, &mut HashSet::new(), &mut stores) || stores.len() > 1 {
            return false;
        }
        stores.into_iter().all(|slot| self.slot(slot))
    }

    /// Allows only same-block numeric observations, transparent retains and one destination.
    fn value_uses(
        &self,
        value: ValueId,
        producer: InstId,
        seen: &mut HashSet<ValueId>,
        stores: &mut HashSet<LocalSlotId>,
    ) -> bool {
        if !seen.insert(value) {
            return true;
        }
        if self.terminator_uses.contains(&value) {
            return false;
        }
        self.uses.get(&value).is_some_and(|uses| {
            uses.iter().all(|&id| {
                if self
                    .locations
                    .get(&producer)
                    .zip(self.locations.get(&id))
                    .is_none_or(|(producer, user)| producer.0 != user.0)
                {
                    return false;
                }
                let user = &self.function.instructions[id.as_raw() as usize];
                match user.op {
                    Op::Release => true,
                    Op::Acquire if user.immediate.is_none() => user
                        .result
                        .is_some_and(|result| self.value_uses(result, producer, seen, stores)),
                    Op::Cast => numeric_cast(user),
                    Op::StoreLocal => match user.immediate {
                        Some(Immediate::LocalSlot(slot)) => {
                            stores.insert(slot);
                            true
                        }
                        _ => false,
                    },
                    _ => false,
                }
            })
        })
    }

    /// Checks every access before admitting a pointer into a PHP local across a backedge.
    fn slot(&mut self, slot: LocalSlotId) -> bool {
        if let Some(&safe) = self.slots.get(&slot) {
            return safe;
        }
        let Some(local) = self.function.locals.get(slot.as_raw() as usize) else {
            return false;
        };
        if (slot.as_raw() as usize) < self.function.params.len()
            || local.kind != LocalKind::PhpLocal
            || self.function.no_epilogue_cleanup_slots.contains(&slot)
            || local.php_type.codegen_repr() != PhpType::Mixed
            || local.ir_type != IrType::Heap(IrHeapKind::Mixed)
        {
            return false;
        }
        let safe = self.accesses.get(&slot).is_some_and(|accesses| {
            accesses.iter().all(|&id| {
                let inst = &self.function.instructions[id.as_raw() as usize];
                match inst.op {
                    Op::StoreLocal => inst
                        .operands
                        .first()
                        .is_some_and(|&value| self.slot_writer(value, slot)),
                    Op::LoadLocal => inst.result.is_some_and(|value| {
                        let Some(metadata) = self.function.value(value) else {
                            return false;
                        };
                        if matches!(
                            (&metadata.ir_type, &metadata.php_type),
                            (IrType::I64, PhpType::Int) | (IrType::F64, PhpType::Float)
                        ) {
                            return true;
                        }
                        self.load_uses(id, value)
                    }),
                    Op::ReleaseLocalSlot => true,
                    _ => false,
                }
            })
        });
        self.slots.insert(slot, safe);
        safe
    }

    /// Allows fresh numeric seed cells and producers that cannot feed any other local.
    fn slot_writer(&self, mut value: ValueId, slot: LocalSlotId) -> bool {
        for _ in 0..self.function.values.len() {
            let Some(metadata) = self.function.value(value) else {
                return false;
            };
            if matches!(
                (&metadata.ir_type, &metadata.php_type),
                (IrType::I64, PhpType::Int) | (IrType::F64, PhpType::Float)
            ) {
                return true;
            }
            let Some((id, inst)) = definition(self.function, value) else {
                return false;
            };
            match inst.op {
                Op::Acquire if inst.immediate.is_none() => {
                    let Some(&source) = inst.operands.first() else {
                        return false;
                    };
                    value = source;
                }
                Op::MixedBox => {
                    return inst.operands.first().is_some_and(|&source| {
                        self.function.value(source).is_some_and(|source| {
                            matches!(
                                (&source.ir_type, &source.php_type),
                                (IrType::I64, PhpType::Int) | (IrType::F64, PhpType::Float)
                            )
                        })
                    })
                }
                op if checked(op) => {
                    let mut stores = HashSet::new();
                    return self.value_uses(value, id, &mut HashSet::new(), &mut stores)
                        && stores == HashSet::from([slot]);
                }
                _ => return false,
            }
        }
        false
    }

    /// A borrowed pointer must be converted before its frame cell can be written again.
    fn load_uses(&self, load: InstId, value: ValueId) -> bool {
        if self.terminator_uses.contains(&value) {
            return false;
        }
        self.uses.get(&value).is_none_or(|uses| {
            uses.iter().all(|&user| {
                let inst = &self.function.instructions[user.as_raw() as usize];
                if inst.op == Op::Release {
                    return true;
                }
                if !numeric_cast(inst) {
                    return false;
                }
                self.locations
                    .get(&load)
                    .zip(self.locations.get(&user))
                    .is_some_and(|(load, user)| {
                        load.0 == user.0 && load.1 < user.1 && load.2 == user.2
                    })
            })
        })
    }
}

/// Identifies operations whose result owns no heap children, even after overflow.
fn checked(op: Op) -> bool {
    matches!(op, Op::ICheckedAdd | Op::ICheckedSub | Op::ICheckedMul)
}

/// Accepts only scalar casts that copy the value rather than expose its address.
fn numeric_cast(inst: &crate::ir::Instruction) -> bool {
    inst.op == Op::Cast
        && matches!(
            (&inst.immediate, &inst.result_php_type),
            (Some(Immediate::CastTarget(IrType::I64)), PhpType::Int)
                | (Some(Immediate::CastTarget(IrType::F64)), PhpType::Float)
        )
}

/// Finds the defining instruction for a non-parameter SSA value.
fn definition(function: &Function, value: ValueId) -> Option<(InstId, &crate::ir::Instruction)> {
    let ValueDef::Instruction { inst, .. } = function.value(value)?.def else {
        return None;
    };
    Some((inst, function.instruction(inst)?))
}

/// Recognizes only transparent retains of a selected stack cell.
pub(super) fn is_stack_value(
    function: &Function,
    cells: &HashMap<ValueId, usize>,
    mut value: ValueId,
) -> bool {
    for _ in 0..function.values.len() {
        if cells.contains_key(&value) {
            return true;
        }
        let Some((_, inst)) = definition(function, value) else {
            return false;
        };
        if inst.op != Op::Acquire || inst.immediate.is_some() {
            return false;
        }
        let Some(&source) = inst.operands.first() else {
            return false;
        };
        value = source;
    }
    false
}

/// Finds a borrowed read of an exclusively numeric local for direct scalar cleanup.
pub(super) fn numeric_local_read(
    function: &Function,
    locals: &HashSet<LocalSlotId>,
    value: ValueId,
) -> bool {
    let Some((_, inst)) = definition(function, value) else {
        return false;
    };
    inst.op == Op::LoadLocal
        && matches!(inst.immediate, Some(Immediate::LocalSlot(slot)) if locals.contains(&slot))
}
