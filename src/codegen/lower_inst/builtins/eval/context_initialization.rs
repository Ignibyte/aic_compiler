//! Purpose:
//! Creates activation-local eval handles and initializes native metadata when it is needed.
//!
//! Called from:
//! - Dynamic eval lowering and native symbol/reflection bridge calls.
//!
//! Key details:
//! - Every context keeps its private scope, handlers and declarations.
//! - Scalar eval may postpone catalog construction; every other bridge requests it eagerly.
//! - A later fragment upgrades the same context before executing any PHP that needs metadata.
//! - Pre-flush and post-flush guards share one registration block and a frame-local phase word.

use super::*;

/// Ensures a context and the complete native catalog for a general bridge operation.
pub(super) fn ensure_eval_context(ctx: &mut FunctionContext<'_>) -> Result<()> {
    ensure_eval_context_handle(ctx)?;
    ensure_eval_native_catalog(ctx)?;
    let offset = ctx.local_offset(eval_context_slot(ctx)?)?;
    abi::load_at_offset(ctx.emitter, abi::int_result_reg(ctx.emitter), offset);
    Ok(())
}

/// Ensures an activation handle without yet demanding its module's native metadata.
pub(super) fn ensure_eval_context_handle(ctx: &mut FunctionContext<'_>) -> Result<()> {
    let offset = ctx.local_offset(eval_context_slot(ctx)?)?;
    let ready = ctx.next_label("eval_context_ready");
    let result = abi::int_result_reg(ctx.emitter);
    abi::load_at_offset(ctx.emitter, result, offset);
    abi::emit_branch_if_int_result_nonzero(ctx.emitter, &ready);
    register_eval_regex_provider(ctx);
    // -- attach any existing catalog without constructing a missing one --
    let catalog = ctx.data.add_private_identity("L__elephc_eval_native_catalog");
    abi::emit_symbol_address(ctx.emitter, abi::int_arg_reg_name(ctx.emitter.target, 0), &catalog);
    let symbol = ctx.emitter.target.extern_symbol("__elephc_eval_context_new_with_native_catalog");
    abi::emit_call_label(ctx.emitter, &symbol);
    abi::store_at_offset(ctx.emitter, result, offset);
    // The shared runtime wrapper must follow the currently entered loaded module.
    let scratch = abi::symbol_scratch_reg(ctx.emitter);
    abi::emit_symbol_address(ctx.emitter, scratch,
        crate::codegen::eval_callable_helpers::EVAL_CALLBACK_WRAPPER_LABEL);
    abi::emit_store_reg_to_symbol(ctx.emitter, scratch, "_elephc_eval_wrap_callback_fn", 0);
    ctx.emitter.label(&ready);
    abi::load_at_offset(ctx.emitter, result, offset);
    abi::emit_store_to_sp(ctx.emitter, result, EVAL_CONTEXT_HANDLE_OFFSET);
    Ok(())
}

/// Registers a missing catalog even when a prior scalar fragment already created this context.
fn ensure_eval_native_catalog(ctx: &mut FunctionContext<'_>) -> Result<()> {
    let offset = ctx.local_offset(eval_context_slot(ctx)?)?;
    let registered = ctx.next_label("eval_native_catalog_registered");
    load_eval_context_local_to_arg(ctx, offset, 0);
    let symbol = ctx.emitter.target.extern_symbol("__elephc_eval_context_native_catalog_ready");
    abi::emit_call_label(ctx.emitter, &symbol);
    abi::emit_branch_if_int_result_nonzero(ctx.emitter, &registered);
    // -- publish only after the complete generated registration transaction --
    register_eval_declared_symbols(ctx, offset);
    register_eval_native_global_constants(ctx, offset)?;
    register_eval_native_user_constants(ctx, offset);
    register_eval_native_functions(ctx, offset)?;
    register_eval_native_method_signatures(ctx, offset);
    load_eval_context_local_to_arg(ctx, offset, 0);
    let symbol = ctx.emitter.target.extern_symbol("__elephc_eval_context_publish_native_catalog");
    abi::emit_call_label(ctx.emitter, &symbol);
    ctx.emitter.label(&registered);
    Ok(())
}

/// Local branch targets sharing one catalog registration sequence across both guards.
pub(super) struct EvalCatalogInitialization {
    flush: String,
    register: String,
    execute: String,
}

/// Requests metadata before scope replacement can invoke a destructor, reference hook or warning.
pub(super) fn prepare_eval_scope_flush(ctx: &mut FunctionContext<'_>) -> Result<EvalCatalogInitialization> {
    let labels = EvalCatalogInitialization {
        flush: ctx.next_label("eval_catalog_scope_flush"),
        register: ctx.next_label("eval_catalog_register"),
        execute: ctx.next_label("eval_catalog_execute"),
    };
    load_eval_context_to_arg(ctx, 0);
    load_eval_scope_to_arg(ctx, 1);
    let global = abi::int_arg_reg_name(ctx.emitter.target, 2);
    abi::emit_load_temporary_stack_slot(ctx.emitter, global, EVAL_GLOBAL_SCOPE_HANDLE_OFFSET);
    let symbol = ctx.emitter.target.extern_symbol("__elephc_eval_context_can_flush_without_native_catalog");
    abi::emit_call_label(ctx.emitter, &symbol);
    abi::emit_branch_if_int_result_nonzero(ctx.emitter, &labels.flush);
    let result = abi::int_result_reg(ctx.emitter);
    abi::emit_load_int_immediate(ctx.emitter, result, 0);
    abi::emit_store_to_sp(ctx.emitter, result, EVAL_CATALOG_PHASE_OFFSET);
    abi::emit_jump(ctx.emitter, &labels.register);
    ctx.emitter.label(&labels.flush);
    Ok(labels)
}

/// Proves the new fragment after flush, or upgrades before resuming the appropriate phase.
pub(super) fn prepare_eval_native_catalog(
    ctx: &mut FunctionContext<'_>, labels: EvalCatalogInitialization,
) -> Result<()> {
    load_eval_context_to_arg(ctx, 0);
    load_eval_scope_to_arg(ctx, 1);
    move_saved_eval_code_to_eval_args(ctx);
    let symbol = ctx.emitter.target.extern_symbol("__elephc_eval_context_can_defer_native_catalog");
    abi::emit_call_label(ctx.emitter, &symbol);
    abi::emit_branch_if_int_result_nonzero(ctx.emitter, &labels.execute);
    let result = abi::int_result_reg(ctx.emitter);
    abi::emit_load_int_immediate(ctx.emitter, result, 1);
    abi::emit_store_to_sp(ctx.emitter, result, EVAL_CATALOG_PHASE_OFFSET);
    // -- one registration transaction resumes either before flush or before execution --
    ctx.emitter.label(&labels.register);
    ensure_eval_native_catalog(ctx)?;
    abi::emit_load_temporary_stack_slot(ctx.emitter, result, EVAL_CATALOG_PHASE_OFFSET);
    abi::emit_branch_if_int_result_zero(ctx.emitter, &labels.flush);
    ctx.emitter.label(&labels.execute);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::{data_section::DataSection, emit::Emitter, frame,
        platform::Target, shared_helper::helper_function, shared_state::SharedCodegenState};
    use crate::ir::IrType;

    /// Emits only a deferred handle followed by the guarded upgrade on all target ABIs.
    #[test]
    fn deferred_native_catalog_initialization_preserves_the_upgrade_boundary() {
        for name in ["macos-aarch64", "ios-arm64", "ios-sim-arm64", "linux-aarch64", "linux-x86_64"] {
            for pic in [false, true] {
                let target = Target::parse(name).unwrap();
                let module = Module::new(target);
                let mut function = helper_function("deferred_probe", PhpType::Void);
                function.add_local(Some("context".into()), IrType::I64, PhpType::Int, LocalKind::EvalContext);
                let layout = frame::layout_for_function(&function, target, false, false, false);
                let mut emitter = if pic { Emitter::new_pic(target) } else { Emitter::new(target) };
                let mut data = DataSection::new();
                let mut shared = SharedCodegenState::default();
                let mut context = FunctionContext::new(&module, &function, &mut emitter, &mut data,
                    &mut shared, layout, false, false, false, None);
                ensure_eval_context_handle(&mut context).unwrap();
                context.emitter.comment("deferred_handle_boundary");
                let labels = prepare_eval_scope_flush(&mut context).unwrap();
                prepare_eval_native_catalog(&mut context, labels).unwrap();
                let asm = emitter.output();
                let handle = asm.split_once("deferred_handle_boundary").unwrap().0;
                assert!(handle.contains("__elephc_eval_context_new_with_native_catalog"), "{name}, {pic}");
                assert!(!handle.contains("__elephc_eval_context_publish_native_catalog"), "{name}, {pic}");
                assert!(!handle.contains("__elephc_eval_context_native_catalog_ready"), "{name}, {pic}");
                let proof = asm.find("__elephc_eval_context_can_defer_native_catalog").unwrap();
                let publish = asm.find("__elephc_eval_context_publish_native_catalog").unwrap();
                assert_eq!(asm.matches("__elephc_eval_context_publish_native_catalog").count(), 1);
                assert_eq!(EVAL_SCOPE_ACCESS_MASK_OFFSET + 8, EVAL_CATALOG_PHASE_OFFSET);
                assert_eq!(EVAL_CATALOG_PHASE_OFFSET + 8, EVAL_BRIDGE_STACK_BYTES);
                assert!(proof < publish, "{name}, {pic}");
                assert!(asm[proof..publish].contains("eval_catalog_execute"), "{name}, {pic}");
            }
        }
    }
}
