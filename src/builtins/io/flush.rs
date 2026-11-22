//! Purpose:
//! Home of the PHP `flush` builtin: its declaration and semantic metadata.
//!
//! Called from:
//! - Checker, EIR, optimizer, ownership, and callable consumers through `crate::builtins::registry`.
//!
//! Key details:
//! - elephc's terminal writes are unbuffered syscalls, and `--web`'s `pool` and `request`
//!   models stream each write as it is made, so no body bytes wait for a flush. What `flush()`
//!   still does is freeze the response head: `__rt_flush` commits the header metadata as the
//!   first output would, and under `--web` sends the status and headers before any body, as
//!   PHP's built-in server does. PHP's `flush()` never touches the `ob_*` buffers.
//! - A runtime call with I/O effects, so the optimizer keeps it in place.

builtin! {
    contract: "flush",
    semantics: crate::builtins::semantics::runtime_fn_semantics(crate::ir::RuntimeFnId::Flush),
}
