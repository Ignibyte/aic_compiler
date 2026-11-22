//! Purpose:
//! Integration coverage for the C ABI boundary of the `--web` request handler.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The Rust server calls `_elephc_web_handler` once per request and keeps live values in
//!   callee-saved registers across that call, so the handler must hand every one of them back.

use crate::support::*;

/// Returns the host target's C-ABI callee-saved registers that the handler's own frame must
/// preserve (the frame pointer and the link register are saved with the frame itself).
fn callee_saved_registers() -> &'static [&'static str] {
    match target().arch {
        Arch::X86_64 => &["rbx", "r12", "r13", "r14", "r15"],
        Arch::AArch64 => &[
            "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27", "x28", "d8", "d9",
            "d10", "d11", "d12", "d13", "d14", "d15",
        ],
    }
}

/// Returns the `_elephc_web_handler` body, from its label to its return.
fn web_handler_body(asm: &str) -> &str {
    let start = asm
        .find("_elephc_web_handler:")
        .expect("web asm has no _elephc_web_handler");
    let body = &asm[start..];
    let end = body.find("\n    ret\n").expect("web handler has no ret");
    &body[..end]
}

/// Returns the frame slot a register is saved to in `body`, if it is saved there.
fn saved_slot(body: &str, register: &str) -> Option<String> {
    let store = match target().arch {
        Arch::X86_64 => format!(", {register}\n"),
        Arch::AArch64 => format!("stur {register}, "),
    };
    body.lines().find_map(|line| {
        let line = format!("{line}\n");
        let is_store = match target().arch {
            Arch::X86_64 => line.trim_start().starts_with("mov QWORD PTR [rbp - ")
                || line.trim_start().starts_with("movsd QWORD PTR [rbp - "),
            Arch::AArch64 => line.trim_start().starts_with("stur "),
        };
        (is_store && line.contains(&store)).then(|| {
            let open = line.find('[').unwrap_or(0);
            let close = line.find(']').unwrap_or(line.len());
            line[open..=close].to_string()
        })
    })
}

/// Verifies the `--web` handler saves every C-ABI callee-saved register in its prologue and
/// reloads each one from the same slot before it returns. Saving only the registers its own
/// allocator used let runtime routines clobber the rest, and the server crashed after correct
/// answers (`free(): invalid pointer`) when it next used them.
#[test]
fn test_web_handler_preserves_every_callee_saved_register() {
    let asm = compile_source_to_web_user_asm("<?php\necho \"ok\";\n");
    let body = web_handler_body(&asm);
    for register in callee_saved_registers() {
        let slot = saved_slot(body, register)
            .unwrap_or_else(|| panic!("the web handler does not save {register}"));
        let reload = match target().arch {
            Arch::X86_64 if register.starts_with('d') => unreachable!(),
            Arch::X86_64 => format!("mov {register}, QWORD PTR {slot}"),
            Arch::AArch64 => format!("ldur {register}, {slot}"),
        };
        assert!(
            body.contains(&reload),
            "the web handler saves {register} to {slot} but never reloads it before returning"
        );
    }
}
