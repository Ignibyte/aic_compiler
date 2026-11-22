//! Purpose:
//! Home of the PHP `preg_last_error` builtin: its declaration and semantic metadata.
//!
//! Called from:
//! - Checker, EIR, optimizer, ownership, and callable consumers through
//!   `crate::builtins::registry`.
//!
//! Key details:
//! - No `check` hook is needed: `preg_last_error` takes no arguments and always
//!   returns `Int`. The registry common path enforces arity.
//! - It reads `_preg_last_error`, which every `preg_*` runtime helper resets on entry.


builtin! {
    contract: "preg_last_error",
    semantics: crate::builtins::semantics::runtime_fn_semantics(
        crate::ir::RuntimeFnId::PregLastError,
    ),
}
