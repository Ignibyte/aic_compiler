//! Purpose:
//! Emits the UTF-8 branch of `__rt_htmlspecialchars`: the path a byte >= 0x80 takes inside the escaping loop.
//! Ports php-src's `get_next_char()` for UTF-8 so invalid sequences match PHP byte for byte.
//!
//! Called from:
//! - `crate::codegen_support::runtime::strings::htmlspecialchars` (both architecture emitters).
//!
//! Key details:
//! - A lead byte 0xC2-0xDF, 0xE0-0xEF or 0xF0-0xF4 starts a 2-, 3- or 4-byte sequence of trail bytes 0x80-0xBF;
//!   overlong 3- and 4-byte forms, surrogates and code points above U+10FFFF are invalid.
//! - An invalid sequence is one, two, three or four bytes long, exactly as PHP counts it: the bytes up to the
//!   first one that is missing or is itself a lead byte.
//! - `ENT_IGNORE` (4) drops it, `ENT_SUBSTITUTE` (8) writes U+FFFD (EF BF BD), and with neither the whole result
//!   is the empty string. Internal bit 65536 (`HTML_ESCAPE_RAW_BYTES`, set by the lowering for an `$encoding` other
//!   than UTF-8) copies high bytes as they are.
//! - U+FFFD is 3 bytes per invalid byte at most, inside the 6x reservation the caller already made.

use crate::codegen_support::emit::Emitter;

/// Emits the x86_64 UTF-8 branch at `__rt_htmlsc_high_linux_x86_64`.
///
/// Entered with `dl` the lead byte (already consumed), `rsi` the cursor after it, `rcx` the bytes left after it,
/// `r11` the destination cursor, `r8` the result start and `r9` the flags. Returns to the loop, or to the done
/// label with an empty result when an invalid sequence is neither ignored nor substituted. Clobbers `rax`, `rdi`,
/// `rdx` and `r10`.
pub(super) fn emit_utf8_branch_linux_x86_64(emitter: &mut Emitter) {
    let lead = |emitter: &mut Emitter, reg: &str, target: &str| {
        // A lead byte is ASCII or 0xC2-0xF4.
        emitter.instruction(&format!("cmp {reg}, 128"));
        emitter.instruction(&format!("jb {target}"));
        emitter.instruction(&format!("lea r10d, [{} - 194]", wide(reg)));
        emitter.instruction("cmp r10d, 50");
        emitter.instruction(&format!("jbe {target}"));
    };
    let not_trail = |emitter: &mut Emitter, reg: &str, scratch: &str, target: &str| {
        // A trail byte is 0x80-0xBF.
        emitter.instruction(&format!("lea {scratch}, [{} - 128]", wide(reg)));
        emitter.instruction(&format!("cmp {scratch}, 63"));
        emitter.instruction(&format!("ja {target}"));
    };

    emitter.blank();
    emitter.comment("--- htmlspecialchars: a byte >= 0x80 (UTF-8, as php-src get_next_char) ---");
    emitter.label("__rt_htmlsc_high_linux_x86_64");
    emitter.instruction("test r9, 65536");                                      // an $encoding other than UTF-8?
    emitter.instruction("jnz __rt_htmlsc_plain_linux_x86_64");                  // yes: copy the byte as it is
    emitter.instruction("movzx eax, dl");                                       // the lead byte
    emitter.instruction("cmp eax, 194");                                        // 0x80-0xC1 never lead a sequence
    emitter.instruction("jb __rt_htmlsc_bad1_linux_x86_64");                    // invalid, one byte
    emitter.instruction("cmp eax, 224");                                        // 0xC2-0xDF lead two bytes
    emitter.instruction("jb __rt_htmlsc_two_linux_x86_64");
    emitter.instruction("cmp eax, 240");                                        // 0xE0-0xEF lead three bytes
    emitter.instruction("jb __rt_htmlsc_three_linux_x86_64");
    emitter.instruction("cmp eax, 245");                                        // 0xF0-0xF4 lead four bytes
    emitter.instruction("jb __rt_htmlsc_four_linux_x86_64");
    emitter.instruction("jmp __rt_htmlsc_bad1_linux_x86_64");                   // 0xF5-0xFF never lead a sequence

    // -- two bytes: lead, trail --
    emitter.label("__rt_htmlsc_two_linux_x86_64");
    emitter.instruction("test rcx, rcx");                                       // is a second byte there?
    emitter.instruction("jz __rt_htmlsc_bad1_linux_x86_64");                    // no: invalid, one byte
    emitter.instruction("movzx edi, BYTE PTR [rsi]");                           // the second byte
    emitter.instruction("lea r10d, [rdi - 128]");                               // is it a trail byte?
    emitter.instruction("cmp r10d, 63");
    emitter.instruction("jbe __rt_htmlsc_two_ok_linux_x86_64");                 // yes: a valid character
    lead(emitter, "edi", "__rt_htmlsc_bad1_linux_x86_64");                    // a lead byte stays for the next character
    emitter.instruction("jmp __rt_htmlsc_bad2_linux_x86_64");                   // any other byte is swallowed with the lead
    emitter.label("__rt_htmlsc_two_ok_linux_x86_64");
    emitter.instruction("mov BYTE PTR [r11], al");                              // copy the lead byte
    emitter.instruction("mov BYTE PTR [r11 + 1], dil");                         // copy the trail byte
    emitter.instruction("add r11, 2");                                          // advance the destination past both
    emitter.instruction("add rsi, 1");                                          // consume the trail byte
    emitter.instruction("sub rcx, 1");
    emitter.instruction("jmp __rt_htmlsc_loop_linux_x86_64");                   // next character

    // -- three bytes: lead, trail, trail; no overlong form, no surrogate --
    emitter.label("__rt_htmlsc_three_linux_x86_64");
    emitter.instruction("cmp rcx, 2");                                          // are two more bytes there?
    emitter.instruction("jb __rt_htmlsc_three_bad_linux_x86_64");
    emitter.instruction("movzx edi, BYTE PTR [rsi]");                           // the second byte
    not_trail(emitter, "edi", "r10d", "__rt_htmlsc_three_bad_linux_x86_64");
    emitter.instruction("movzx edx, BYTE PTR [rsi + 1]");                       // the third byte
    not_trail(emitter, "edx", "r10d", "__rt_htmlsc_three_bad_linux_x86_64");
    emitter.instruction("cmp eax, 224");                                        // 0xE0 with a second byte below 0xA0 is overlong
    emitter.instruction("jne __rt_htmlsc_three_e0_linux_x86_64");
    emitter.instruction("cmp edi, 160");
    emitter.instruction("jb __rt_htmlsc_bad3_linux_x86_64");
    emitter.label("__rt_htmlsc_three_e0_linux_x86_64");
    emitter.instruction("cmp eax, 237");                                        // 0xED with a second byte from 0xA0 is a surrogate
    emitter.instruction("jne __rt_htmlsc_three_ok_linux_x86_64");
    emitter.instruction("cmp edi, 160");
    emitter.instruction("jae __rt_htmlsc_bad3_linux_x86_64");
    emitter.label("__rt_htmlsc_three_ok_linux_x86_64");
    emitter.instruction("mov BYTE PTR [r11], al");                              // copy the three bytes
    emitter.instruction("mov BYTE PTR [r11 + 1], dil");
    emitter.instruction("mov BYTE PTR [r11 + 2], dl");
    emitter.instruction("add r11, 3");
    emitter.instruction("add rsi, 2");                                          // consume the two trail bytes
    emitter.instruction("sub rcx, 2");
    emitter.instruction("jmp __rt_htmlsc_loop_linux_x86_64");
    emitter.label("__rt_htmlsc_three_bad_linux_x86_64");
    emitter.instruction("test rcx, rcx");                                       // no second byte: one byte
    emitter.instruction("jz __rt_htmlsc_bad1_linux_x86_64");
    emitter.instruction("movzx edi, BYTE PTR [rsi]");
    lead(emitter, "edi", "__rt_htmlsc_bad1_linux_x86_64");                    // a lead second byte: one byte
    emitter.instruction("cmp rcx, 2");                                          // no third byte: two bytes
    emitter.instruction("jb __rt_htmlsc_bad2_linux_x86_64");
    emitter.instruction("movzx edx, BYTE PTR [rsi + 1]");
    lead(emitter, "edx", "__rt_htmlsc_bad2_linux_x86_64");                    // a lead third byte: two bytes
    emitter.instruction("jmp __rt_htmlsc_bad3_linux_x86_64");                   // otherwise all three

    // -- four bytes: lead, trail, trail, trail; U+10000-U+10FFFF only --
    emitter.label("__rt_htmlsc_four_linux_x86_64");
    emitter.instruction("cmp rcx, 3");                                          // are three more bytes there?
    emitter.instruction("jb __rt_htmlsc_four_bad_linux_x86_64");
    emitter.instruction("movzx edi, BYTE PTR [rsi]");                           // the second byte
    not_trail(emitter, "edi", "r10d", "__rt_htmlsc_four_bad_linux_x86_64");
    emitter.instruction("movzx edx, BYTE PTR [rsi + 1]");                       // the third byte
    not_trail(emitter, "edx", "r10d", "__rt_htmlsc_four_bad_linux_x86_64");
    emitter.instruction("movzx r10d, BYTE PTR [rsi + 2]");                      // the fourth byte
    not_trail(emitter, "r10d", "eax", "__rt_htmlsc_four_bad_linux_x86_64");
    emitter.instruction("movzx eax, BYTE PTR [rsi - 1]");                       // reload the lead byte the check reused
    emitter.instruction("cmp eax, 240");                                        // 0xF0 with a second byte below 0x90 is overlong
    emitter.instruction("jne __rt_htmlsc_four_f0_linux_x86_64");
    emitter.instruction("cmp edi, 144");
    emitter.instruction("jb __rt_htmlsc_bad4_linux_x86_64");
    emitter.label("__rt_htmlsc_four_f0_linux_x86_64");
    emitter.instruction("cmp eax, 244");                                        // 0xF4 with a second byte from 0x90 is above U+10FFFF
    emitter.instruction("jne __rt_htmlsc_four_ok_linux_x86_64");
    emitter.instruction("cmp edi, 144");
    emitter.instruction("jae __rt_htmlsc_bad4_linux_x86_64");
    emitter.label("__rt_htmlsc_four_ok_linux_x86_64");
    emitter.instruction("mov BYTE PTR [r11], al");                              // copy the four bytes
    emitter.instruction("mov BYTE PTR [r11 + 1], dil");
    emitter.instruction("mov BYTE PTR [r11 + 2], dl");
    emitter.instruction("mov BYTE PTR [r11 + 3], r10b");
    emitter.instruction("add r11, 4");
    emitter.instruction("add rsi, 3");                                          // consume the three trail bytes
    emitter.instruction("sub rcx, 3");
    emitter.instruction("jmp __rt_htmlsc_loop_linux_x86_64");
    emitter.label("__rt_htmlsc_four_bad_linux_x86_64");
    emitter.instruction("test rcx, rcx");                                       // no second byte: one byte
    emitter.instruction("jz __rt_htmlsc_bad1_linux_x86_64");
    emitter.instruction("movzx edi, BYTE PTR [rsi]");
    lead(emitter, "edi", "__rt_htmlsc_bad1_linux_x86_64");                    // a lead second byte: one byte
    emitter.instruction("cmp rcx, 2");                                          // no third byte: two bytes
    emitter.instruction("jb __rt_htmlsc_bad2_linux_x86_64");
    emitter.instruction("movzx edi, BYTE PTR [rsi + 1]");
    lead(emitter, "edi", "__rt_htmlsc_bad2_linux_x86_64");                    // a lead third byte: two bytes
    emitter.instruction("cmp rcx, 3");                                          // no fourth byte: three bytes
    emitter.instruction("jb __rt_htmlsc_bad3_linux_x86_64");
    emitter.instruction("movzx edi, BYTE PTR [rsi + 2]");
    lead(emitter, "edi", "__rt_htmlsc_bad3_linux_x86_64");                    // a lead fourth byte: three bytes
    emitter.instruction("jmp __rt_htmlsc_bad4_linux_x86_64");                   // otherwise all four

    // -- an invalid sequence of one to four bytes (the lead already consumed) --
    emitter.label("__rt_htmlsc_bad4_linux_x86_64");
    emitter.instruction("add rsi, 1");                                          // swallow one more byte
    emitter.instruction("sub rcx, 1");
    emitter.label("__rt_htmlsc_bad3_linux_x86_64");
    emitter.instruction("add rsi, 1");
    emitter.instruction("sub rcx, 1");
    emitter.label("__rt_htmlsc_bad2_linux_x86_64");
    emitter.instruction("add rsi, 1");
    emitter.instruction("sub rcx, 1");
    emitter.label("__rt_htmlsc_bad1_linux_x86_64");
    emitter.instruction("test r9, 4");                                          // ENT_IGNORE: drop it
    emitter.instruction("jnz __rt_htmlsc_loop_linux_x86_64");
    emitter.instruction("test r9, 8");                                          // ENT_SUBSTITUTE: write U+FFFD
    emitter.instruction("jz __rt_htmlsc_invalid_linux_x86_64");
    emitter.instruction("mov BYTE PTR [r11], 239");                             // EF
    emitter.instruction("mov BYTE PTR [r11 + 1], 191");                         // BF
    emitter.instruction("mov BYTE PTR [r11 + 2], 189");                         // BD
    emitter.instruction("add r11, 3");
    emitter.instruction("jmp __rt_htmlsc_loop_linux_x86_64");
    emitter.label("__rt_htmlsc_invalid_linux_x86_64");
    emitter.instruction("mov r11, r8");                                         // neither flag: PHP returns the empty string
    emitter.instruction("jmp __rt_htmlsc_done_linux_x86_64");
}

/// Emits the AArch64 UTF-8 branch at `__rt_htmlsc_high`.
///
/// Entered with `w12` the lead byte (already consumed), `x1` the cursor after it, `x11` the bytes left after it,
/// `x9` the destination cursor, `x10` the result start and `x14` the flags. Returns to the loop, or to the done
/// label with an empty result when an invalid sequence is neither ignored nor substituted. Clobbers `w13`, `w15`,
/// `w16` and `w17`. Written to mirror the x86_64 branch; not run on this architecture yet.
pub(super) fn emit_utf8_branch_aarch64(emitter: &mut Emitter) {
    let lead = |emitter: &mut Emitter, reg: &str, target: &str| {
        // A lead byte is ASCII or 0xC2-0xF4.
        emitter.instruction(&format!("cmp {reg}, #128"));
        emitter.instruction(&format!("b.lo {target}"));
        emitter.instruction(&format!("sub w13, {reg}, #194"));
        emitter.instruction("cmp w13, #50");
        emitter.instruction(&format!("b.ls {target}"));
    };
    let not_trail = |emitter: &mut Emitter, reg: &str, target: &str| {
        // A trail byte is 0x80-0xBF.
        emitter.instruction(&format!("sub w13, {reg}, #128"));
        emitter.instruction("cmp w13, #63");
        emitter.instruction(&format!("b.hi {target}"));
    };

    emitter.blank();
    emitter.comment("--- htmlspecialchars: a byte >= 0x80 (UTF-8, as php-src get_next_char) ---");
    emitter.label("__rt_htmlsc_high");
    emitter.instruction("tbnz x14, #16, __rt_htmlsc_plain");                    // an $encoding other than UTF-8: copy the byte
    emitter.instruction("cmp w12, #194");                                       // 0x80-0xC1 never lead a sequence
    emitter.instruction("b.lo __rt_htmlsc_bad1");
    emitter.instruction("cmp w12, #224");                                       // 0xC2-0xDF lead two bytes
    emitter.instruction("b.lo __rt_htmlsc_two");
    emitter.instruction("cmp w12, #240");                                       // 0xE0-0xEF lead three bytes
    emitter.instruction("b.lo __rt_htmlsc_three");
    emitter.instruction("cmp w12, #245");                                       // 0xF0-0xF4 lead four bytes
    emitter.instruction("b.lo __rt_htmlsc_four");
    emitter.instruction("b __rt_htmlsc_bad1");                                  // 0xF5-0xFF never lead a sequence

    emitter.label("__rt_htmlsc_two");
    emitter.instruction("cbz x11, __rt_htmlsc_bad1");                           // no second byte: one byte
    emitter.instruction("ldrb w15, [x1]");                                      // the second byte
    emitter.instruction("sub w13, w15, #128");                                  // is it a trail byte?
    emitter.instruction("cmp w13, #63");
    emitter.instruction("b.ls __rt_htmlsc_two_ok");
    lead(emitter, "w15", "__rt_htmlsc_bad1");                                 // a lead byte stays for the next character
    emitter.instruction("b __rt_htmlsc_bad2");
    emitter.label("__rt_htmlsc_two_ok");
    emitter.instruction("strb w12, [x9], #1");                                  // copy the lead and trail bytes
    emitter.instruction("strb w15, [x9], #1");
    emitter.instruction("add x1, x1, #1");                                      // consume the trail byte
    emitter.instruction("sub x11, x11, #1");
    emitter.instruction("b __rt_htmlsc_loop");

    emitter.label("__rt_htmlsc_three");
    emitter.instruction("cmp x11, #2");                                         // are two more bytes there?
    emitter.instruction("b.lo __rt_htmlsc_three_bad");
    emitter.instruction("ldrb w15, [x1]");
    not_trail(emitter, "w15", "__rt_htmlsc_three_bad");
    emitter.instruction("ldrb w16, [x1, #1]");
    not_trail(emitter, "w16", "__rt_htmlsc_three_bad");
    emitter.instruction("cmp w12, #224");                                       // 0xE0 with a second byte below 0xA0 is overlong
    emitter.instruction("b.ne __rt_htmlsc_three_e0");
    emitter.instruction("cmp w15, #160");
    emitter.instruction("b.lo __rt_htmlsc_bad3");
    emitter.label("__rt_htmlsc_three_e0");
    emitter.instruction("cmp w12, #237");                                       // 0xED with a second byte from 0xA0 is a surrogate
    emitter.instruction("b.ne __rt_htmlsc_three_ok");
    emitter.instruction("cmp w15, #160");
    emitter.instruction("b.hs __rt_htmlsc_bad3");
    emitter.label("__rt_htmlsc_three_ok");
    emitter.instruction("strb w12, [x9], #1");                                  // copy the three bytes
    emitter.instruction("strb w15, [x9], #1");
    emitter.instruction("strb w16, [x9], #1");
    emitter.instruction("add x1, x1, #2");
    emitter.instruction("sub x11, x11, #2");
    emitter.instruction("b __rt_htmlsc_loop");
    emitter.label("__rt_htmlsc_three_bad");
    emitter.instruction("cbz x11, __rt_htmlsc_bad1");
    emitter.instruction("ldrb w15, [x1]");
    lead(emitter, "w15", "__rt_htmlsc_bad1");
    emitter.instruction("cmp x11, #2");
    emitter.instruction("b.lo __rt_htmlsc_bad2");
    emitter.instruction("ldrb w16, [x1, #1]");
    lead(emitter, "w16", "__rt_htmlsc_bad2");
    emitter.instruction("b __rt_htmlsc_bad3");

    emitter.label("__rt_htmlsc_four");
    emitter.instruction("cmp x11, #3");                                         // are three more bytes there?
    emitter.instruction("b.lo __rt_htmlsc_four_bad");
    emitter.instruction("ldrb w15, [x1]");
    not_trail(emitter, "w15", "__rt_htmlsc_four_bad");
    emitter.instruction("ldrb w16, [x1, #1]");
    not_trail(emitter, "w16", "__rt_htmlsc_four_bad");
    emitter.instruction("ldrb w17, [x1, #2]");
    not_trail(emitter, "w17", "__rt_htmlsc_four_bad");
    emitter.instruction("cmp w12, #240");                                       // 0xF0 with a second byte below 0x90 is overlong
    emitter.instruction("b.ne __rt_htmlsc_four_f0");
    emitter.instruction("cmp w15, #144");
    emitter.instruction("b.lo __rt_htmlsc_bad4");
    emitter.label("__rt_htmlsc_four_f0");
    emitter.instruction("cmp w12, #244");                                       // 0xF4 with a second byte from 0x90 is above U+10FFFF
    emitter.instruction("b.ne __rt_htmlsc_four_ok");
    emitter.instruction("cmp w15, #144");
    emitter.instruction("b.hs __rt_htmlsc_bad4");
    emitter.label("__rt_htmlsc_four_ok");
    emitter.instruction("strb w12, [x9], #1");                                  // copy the four bytes
    emitter.instruction("strb w15, [x9], #1");
    emitter.instruction("strb w16, [x9], #1");
    emitter.instruction("strb w17, [x9], #1");
    emitter.instruction("add x1, x1, #3");
    emitter.instruction("sub x11, x11, #3");
    emitter.instruction("b __rt_htmlsc_loop");
    emitter.label("__rt_htmlsc_four_bad");
    emitter.instruction("cbz x11, __rt_htmlsc_bad1");
    emitter.instruction("ldrb w15, [x1]");
    lead(emitter, "w15", "__rt_htmlsc_bad1");
    emitter.instruction("cmp x11, #2");
    emitter.instruction("b.lo __rt_htmlsc_bad2");
    emitter.instruction("ldrb w15, [x1, #1]");
    lead(emitter, "w15", "__rt_htmlsc_bad2");
    emitter.instruction("cmp x11, #3");
    emitter.instruction("b.lo __rt_htmlsc_bad3");
    emitter.instruction("ldrb w15, [x1, #2]");
    lead(emitter, "w15", "__rt_htmlsc_bad3");
    emitter.instruction("b __rt_htmlsc_bad4");

    emitter.label("__rt_htmlsc_bad4");
    emitter.instruction("add x1, x1, #1");                                      // swallow one more byte
    emitter.instruction("sub x11, x11, #1");
    emitter.label("__rt_htmlsc_bad3");
    emitter.instruction("add x1, x1, #1");
    emitter.instruction("sub x11, x11, #1");
    emitter.label("__rt_htmlsc_bad2");
    emitter.instruction("add x1, x1, #1");
    emitter.instruction("sub x11, x11, #1");
    emitter.label("__rt_htmlsc_bad1");
    emitter.instruction("tbnz x14, #2, __rt_htmlsc_loop");                      // ENT_IGNORE: drop it
    emitter.instruction("tbz x14, #3, __rt_htmlsc_invalid");                    // neither flag: the empty string
    emitter.instruction("mov w13, #239");                                       // ENT_SUBSTITUTE: write U+FFFD, EF BF BD
    emitter.instruction("strb w13, [x9], #1");
    emitter.instruction("mov w13, #191");
    emitter.instruction("strb w13, [x9], #1");
    emitter.instruction("mov w13, #189");
    emitter.instruction("strb w13, [x9], #1");
    emitter.instruction("b __rt_htmlsc_loop");
    emitter.label("__rt_htmlsc_invalid");
    emitter.instruction("mov x9, x10");                                         // PHP returns the empty string
    emitter.instruction("b __rt_htmlsc_done");
}

/// The 64-bit name of a 32-bit x86_64 register, for an address expression.
fn wide(reg: &str) -> &str {
    match reg {
        "eax" => "rax",
        "edi" => "rdi",
        "edx" => "rdx",
        "r10d" => "r10",
        other => other,
    }
}
