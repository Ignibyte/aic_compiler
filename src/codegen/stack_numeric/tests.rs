//! Purpose:
//! Verifies the complete nonescaping numeric-cell lifetime proof with explicit EIR graphs.
//!
//! Called from:
//! - The library test harness through `codegen::stack_numeric`.
//!
//! Key details:
//! - Negative cases cover owners escaping, aliases, delayed reads and loop-carried pointers.

use super::*;
use crate::ir::{Builder, Ownership, Terminator};

/// Extracts selected cells for concise gate assertions.
fn candidates(function: &Function) -> Vec<ValueId> {
    plan(function).cells
}

/// Builds a local numeric update with a scalar read and an optional escaping observation.
fn fixture(escape: bool, delayed: bool) -> (Function, ValueId) {
    let mut function = Function::new("numeric".into(), IrType::I64, PhpType::Int);
    let slot = function.add_local(
        Some("sum".into()),
        IrType::Heap(IrHeapKind::Mixed),
        PhpType::Mixed,
        LocalKind::PhpLocal,
    );
    let mut builder = Builder::new(&mut function);
    let entry = builder.create_named_block("entry", vec![]);
    builder.set_entry(entry);
    builder.position_at_end(entry);
    let zero = builder.emit_const_i64(0);
    builder.emit_store_local(slot, zero);
    let old = builder.emit_load_local(slot, IrType::Heap(IrHeapKind::Mixed), PhpType::Mixed);
    let one = builder.emit_const_i64(1);
    let result = builder
        .emit(
            Op::ICheckedAdd,
            vec![one, one],
            None,
            IrType::Heap(IrHeapKind::Mixed),
            PhpType::Mixed,
            Ownership::Owned,
        )
        .unwrap();
    let acquired = builder
        .emit(
            Op::Acquire,
            vec![result],
            None,
            IrType::Heap(IrHeapKind::Mixed),
            PhpType::Mixed,
            Ownership::Owned,
        )
        .unwrap();
    builder.emit_store_local(slot, acquired);
    builder.emit(
        Op::Release,
        vec![old],
        None,
        IrType::Void,
        PhpType::Void,
        Ownership::NonHeap,
    );
    builder.emit(
        Op::Release,
        vec![result],
        None,
        IrType::Void,
        PhpType::Void,
        Ownership::NonHeap,
    );
    if delayed {
        builder.emit(
            Op::Cast,
            vec![old],
            Some(Immediate::CastTarget(IrType::I64)),
            IrType::I64,
            PhpType::Int,
            Ownership::NonHeap,
        );
    }
    let loaded = builder.emit_load_local(slot, IrType::I64, PhpType::Int);
    builder.terminate(Terminator::Return {
        value: Some(if escape { acquired } else { loaded }),
    });
    if escape {
        function.return_type = IrType::Heap(IrHeapKind::Mixed);
        function.return_php_type = PhpType::Mixed;
    }
    (function, result)
}

/// The ordinary local update stays in its frame, but returning its pointer is an escape.
#[test]
fn stack_numeric_local_and_return_gates() {
    let (function, value) = fixture(false, false);
    assert_eq!(candidates(&function), vec![value]);
    let (function, _) = fixture(true, false);
    assert!(candidates(&function).is_empty());
}

/// Converting an old pointer after the cell has been reused would read the new payload.
#[test]
fn stack_numeric_rejects_delayed_borrowed_read() {
    let (function, _) = fixture(false, true);
    assert!(candidates(&function).is_empty());
}

/// A reference or an eval context can expose a local through a side channel.
#[test]
fn stack_numeric_rejects_reference_eval_and_suspension() {
    for kind in [
        LocalKind::RefCell,
        LocalKind::EvalContext,
        LocalKind::EvalScope,
        LocalKind::EvalGlobalScope,
        LocalKind::GeneratorState,
    ] {
        let (mut function, _) = fixture(false, false);
        function.add_local(None, IrType::I64, PhpType::Int, kind);
        assert!(candidates(&function).is_empty(), "{kind:?}");
    }
    let (mut function, _) = fixture(false, false);
    function.flags.is_generator = true;
    assert!(candidates(&function).is_empty());
}

/// A second destination would alias mutable stack storage across later updates.
#[test]
fn stack_numeric_rejects_shared_destination() {
    let (mut function, result) = fixture(false, false);
    let second = function.add_local(
        Some("other".into()),
        IrType::Heap(IrHeapKind::Mixed),
        PhpType::Mixed,
        LocalKind::PhpLocal,
    );
    let term = function.blocks[0].terminator.take().unwrap();
    let mut builder = Builder::new(&mut function);
    builder.position_at_end(crate::ir::BlockId::from_raw(0));
    builder.emit_store_local(second, result);
    builder.terminate(term);
    assert!(candidates(&function).is_empty());
}

/// Unknown consumers, cloning, child-bearing assignments and conditional local identities fail closed.
#[test]
fn stack_numeric_rejects_unknown_consumers_and_writers() {
    for op in [Op::MixedClone, Op::MixedBox, Op::EchoValue, Op::VarDump] {
        let (mut function, result) = fixture(false, false);
        let term = function.blocks[0].terminator.take().unwrap();
        let mut builder = Builder::new(&mut function);
        builder.position_at_end(crate::ir::BlockId::from_raw(0));
        let (ir, php, owner) = if matches!(op, Op::MixedClone | Op::MixedBox) {
            (
                IrType::Heap(IrHeapKind::Mixed),
                PhpType::Mixed,
                Ownership::Owned,
            )
        } else {
            (IrType::Void, PhpType::Void, Ownership::NonHeap)
        };
        builder.emit(op, vec![result], None, ir, php, owner);
        builder.terminate(term);
        assert!(candidates(&function).is_empty(), "{op:?}");
    }
    let (mut function, _) = fixture(false, false);
    function.locals[0].kind = LocalKind::GlobalAlias;
    assert!(candidates(&function).is_empty());
}

/// Block parameters can carry a previous iteration's pointer, so they keep real heap cells.
#[test]
fn stack_numeric_rejects_pointer_block_arguments() {
    let (mut function, result) = fixture(false, false);
    let mut builder = Builder::new(&mut function);
    let exit = builder.create_named_block(
        "exit",
        vec![(IrType::Heap(IrHeapKind::Mixed), PhpType::Mixed)],
    );
    let pointer = builder.block_param(exit, 0);
    builder.position_at_end(exit);
    let scalar = builder
        .emit(
            Op::Cast,
            vec![pointer],
            Some(Immediate::CastTarget(IrType::I64)),
            IrType::I64,
            PhpType::Int,
            Ownership::NonHeap,
        )
        .unwrap();
    builder.terminate(Terminator::Return {
        value: Some(scalar),
    });
    function.blocks[0].terminator = Some(Terminator::Br {
        target: exit,
        args: vec![result],
    });
    assert!(candidates(&function).is_empty());
}

/// Inlined borrowed owners and function parameters do not acquire a new local lifetime.
#[test]
fn stack_numeric_rejects_cleanup_excluded_local() {
    let (mut function, _) = fixture(false, false);
    function
        .no_epilogue_cleanup_slots
        .insert(LocalSlotId::from_raw(0));
    assert!(candidates(&function).is_empty());
}
