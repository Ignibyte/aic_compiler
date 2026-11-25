//! Purpose:
//! Emits the `__rt_stream_socket_accept` runtime helper, which accepts a
//! pending connection on a listening socket and captures the peer address
//! for the optional `$peer_name` out-parameter.
//!
//! Called from:
//! - `crate::codegen_support::runtime::emitters::emit_runtime()` via `crate::codegen_support::runtime::io`.
//! - The `stream_socket_accept` builtin emitter, which threads in the
//!   timeout (in microseconds; `-1` = infinite wait) and reads the stashed
//!   peer address out of `_accept_peer_ptr` / `_accept_peer_len` when the
//!   user passed a `&$peer_name` variable.
//!
//! Key details:
//! - When `timeout_us >= 0` the helper gates `accept()` behind a one-entry
//!   `poll()` (macOS) / `ppoll()` (Linux), as php-src does, so any descriptor
//!   number works. A timeout (`poll` returns 0) lowers to a -1 result that the
//!   builtin boxes as PHP `false`.
//! - The captured `sockaddr` is dispatched on its family byte and rendered
//!   through `__rt_format_sockaddr_in` (IPv4) or
//!   `__rt_format_sockaddr_unix` (Unix-domain). Anything else (AF_INET6
//!   today, future families) lowers to an empty string. A failed accept
//!   clears both globals.

use crate::codegen_support::{
    abi,
    emit::Emitter,
    platform::{Arch, Platform},
};

/// Byte offset of the address-family discriminator inside a freshly populated
/// sockaddr buffer; macOS keeps a 1-byte `sa_len` ahead of the family byte
/// (BSD layout), while Linux puts the 16-bit family at offset 0 directly.
fn family_byte_offset(platform: Platform) -> i64 {
    match platform {
        Platform::MacOS => 1,
        Platform::Linux => 0,
        Platform::Windows => panic!("Windows target is not yet supported (see issue #379)"),
    }
}

/// stream_socket_accept: accept a pending connection on a listening socket,
/// optionally gated by a microsecond timeout, capturing the peer address.
/// Input:  AArch64 x0 = listening fd, x1 = timeout_us (-1 = infinite)
///         x86_64  rdi = listening fd, rsi = timeout_us (-1 = infinite)
/// Output: accepted descriptor, or -1 on failure / timeout
pub fn emit_stream_socket_accept(emitter: &mut Emitter) {
    if emitter.target.arch == Arch::X86_64 {
        emit_stream_socket_accept_linux_x86_64(emitter);
        return;
    }

    let plat = emitter.platform;
    let linux = plat == Platform::Linux;
    let family_off = 32 + family_byte_offset(plat);
    emitter.blank();
    emitter.comment("--- runtime: stream_socket_accept ---");
    emitter.label_global("__rt_stream_socket_accept");

    // Frame (224 bytes):
    //   [0..16)    saved x29 / x30
    //   [16)       saved fd
    //   [24)       saved timeout_us
    //   [32..160)  sockaddr buffer (sockaddr_in / sockaddr_un / sockaddr_in6 fit)
    //   [160)      addrlen (4 + 4 pad)
    //   [168)      pollfd: fd (4), events (2), revents (2)
    //   [176)      padding
    //   [184)      timespec: seconds field (Linux)
    //   [192)      timespec: nanoseconds field (Linux)
    //   [200)      accepted fd
    //   [208..224) padding for 16-byte alignment
    emitter.instruction("sub sp, sp, #224");                                    // allocate the helper frame
    emitter.instruction("stp x29, x30, [sp, #0]");                              // save frame pointer and return address
    emitter.instruction("add x29, sp, #0");                                     // establish the helper frame pointer
    emitter.instruction("str x0, [sp, #16]");                                   // save the listening fd
    emitter.instruction("str x1, [sp, #24]");                                   // save the timeout in microseconds

    // -- timeout < 0 skips the poll gate and blocks indefinitely --
    emitter.instruction("cmp x1, #0");                                          // is the timeout negative (infinite)?
    emitter.instruction("b.lt __rt_ssa_blocking_accept");                       // jump straight to accept for the infinite case

    // -- one pollfd for the listening descriptor: events = POLLIN, revents = 0 --
    emitter.instruction("cmp w0, #0");                                          // poll ignores a negative descriptor instead of failing
    emitter.instruction("b.lt __rt_ssa_fail_clear");                            // not a descriptor: bail out
    emitter.instruction("str w0, [sp, #168]");                                  // pollfd.fd = the listening descriptor
    emitter.instruction("mov w9, #1");                                          // POLLIN = 1 in the low half, revents = 0 in the high half
    emitter.instruction("str w9, [sp, #172]");                                  // pollfd.events and pollfd.revents

    // -- poll(&pollfd, 1, timeout_ms) on macOS; ppoll(&pollfd, 1, &timespec, NULL, 0) on Linux --
    emitter.instruction("ldr x9, [sp, #24]");                                   // reload the timeout in microseconds
    if linux {
        emitter.instruction("mov x10, #0x4240");                                // low 16 bits of 1_000_000 (0xF4240)
        emitter.instruction("movk x10, #0xF, lsl #16");                         // upper bits make x10 = 1_000_000 (one second in us)
        emitter.instruction("udiv x11, x9, x10");                               // seconds = timeout_us / 1_000_000
        emitter.instruction("msub x12, x11, x10, x9");                          // micro_remainder = timeout_us - seconds*1_000_000
        emitter.instruction("mov x10, #1000");                                  // a microsecond holds 1000 nanoseconds
        emitter.instruction("mul x12, x12, x10");                               // convert microseconds to nanoseconds
        emitter.instruction("str x11, [sp, #184]");                             // store the timespec seconds field
        emitter.instruction("str x12, [sp, #192]");                             // store the timespec nanoseconds field
        emitter.instruction("add x2, sp, #184");                                // pointer to the timespec
        emitter.instruction("mov x3, #0");                                      // signal mask pointer (NULL)
        emitter.instruction("mov x4, #0");                                      // signal mask size
    } else {
        emitter.instruction("mov x10, #1000");                                  // a millisecond holds 1000 microseconds
        emitter.instruction("udiv x2, x9, x10");                                // timeout in milliseconds, truncated as php-src does
    }
    emitter.instruction("add x0, sp, #168");                                    // pointer to the pollfd
    emitter.instruction("mov x1, #1");                                          // one descriptor
    emitter.syscall(230);
    emitter.instruction("cmp x0, #0");                                          // did poll fail or time out?
    emitter.instruction("b.le __rt_ssa_fail_clear");                            // 0 = timeout, <0 = error → report failure

    emitter.label("__rt_ssa_blocking_accept");
    // -- accept(fd, &sockaddr, &addrlen) --
    emitter.instruction("mov w9, #128");                                        // sockaddr buffer capacity
    emitter.instruction("str w9, [sp, #160]");                                  // addrlen in/out parameter = 128
    emitter.instruction("ldr x0, [sp, #16]");                                   // reload the listening fd
    emitter.instruction("add x1, sp, #32");                                     // pointer to the peer sockaddr buffer
    emitter.instruction("add x2, sp, #160");                                    // pointer to the addrlen parameter
    emitter.syscall(30);
    if plat.needs_cmp_before_error_branch() {
        emitter.instruction("cmp x0, #0");                                      // Linux: a negative descriptor means failure
    }
    emitter.instruction(&plat.branch_on_syscall_success("__rt_ssa_accept_ok")); // continue when accept succeeded
    emitter.instruction("b __rt_ssa_fail_clear");                               // accept() failed: clear globals and report -1

    emitter.label("__rt_ssa_accept_ok");
    emitter.instruction("str x0, [sp, #200]");                                  // save the accepted fd

    // -- dispatch on the captured sockaddr's address family --
    emitter.instruction("ldr w9, [sp, #160]");                                  // reload addrlen returned by accept
    emitter.instruction("cmp w9, #2");                                          // is there at least a family-byte's worth of data?
    emitter.instruction("b.lt __rt_ssa_empty_peer");                            // no peer info: empty $peer_name
    let af_inet6 = plat.af_inet6();
    emitter.instruction(&format!("ldrb w10, [sp, #{}]", family_off));           // load the address-family discriminator
    emitter.instruction("cmp w10, #2");                                         // AF_INET = 2 on both platforms
    emitter.instruction("b.eq __rt_ssa_peer_inet");                             // IPv4 peer: format A.B.C.D:port
    emitter.instruction("cmp w10, #1");                                         // AF_UNIX = 1 on both platforms
    emitter.instruction("b.eq __rt_ssa_peer_unix");                             // Unix-domain peer: format sun_path
    emitter.instruction(&format!("cmp w10, #{}", af_inet6));                    // AF_INET6 = 30 on macOS / 10 on Linux
    emitter.instruction("b.eq __rt_ssa_peer_inet6");                            // IPv6 peer: format [ipv6]:port
    emitter.instruction("b __rt_ssa_empty_peer");                               // unsupported family: empty $peer_name

    // -- AF_INET peer: render via __rt_format_sockaddr_in --
    emitter.label("__rt_ssa_peer_inet");
    emitter.instruction("add x0, sp, #32");                                     // pointer to the captured peer sockaddr
    emitter.instruction("bl __rt_format_sockaddr_in");                          // x1 = address string, x2 = length
    emitter.instruction("b __rt_ssa_stash_peer");                               // common stash + return

    // -- AF_UNIX peer: render via __rt_format_sockaddr_unix --
    emitter.label("__rt_ssa_peer_unix");
    emitter.instruction("add x0, sp, #32");                                     // pointer to the captured peer sockaddr
    emitter.instruction("ldr w1, [sp, #160]");                                  // addrlen returned by accept
    emitter.instruction("bl __rt_format_sockaddr_unix");                        // x1 = sun_path string, x2 = length
    emitter.instruction("b __rt_ssa_stash_peer");                               // common stash + return

    // -- AF_INET6 peer: render via __rt_format_sockaddr_in6 --
    emitter.label("__rt_ssa_peer_inet6");
    emitter.instruction("add x0, sp, #32");                                     // pointer to the captured peer sockaddr_in6
    emitter.instruction("bl __rt_format_sockaddr_in6");                         // x1 = address string, x2 = length
    emitter.instruction("b __rt_ssa_stash_peer");                               // common stash + return

    // -- empty $peer_name: connected/unnamed peer or unsupported family --
    emitter.label("__rt_ssa_empty_peer");
    emitter.instruction("mov x0, #1");                                          // alloc one byte so the heap header is well-formed
    emitter.instruction("bl __rt_heap_alloc");                                  // x0 = single-byte buffer
    emitter.instruction("mov x9, #1");                                          // heap kind 1 = persisted elephc string
    emitter.instruction("str x9, [x0, #-8]");                                   // stamp the buffer as an owned string
    emitter.instruction("mov x1, x0");                                          // empty address pointer
    emitter.instruction("mov x2, #0");                                          // empty address length

    emitter.label("__rt_ssa_stash_peer");
    abi::emit_symbol_address(emitter, "x9", "_accept_peer_ptr");
    emitter.instruction("str x1, [x9]");                                        // stash the peer address pointer
    abi::emit_symbol_address(emitter, "x9", "_accept_peer_len");
    emitter.instruction("str x2, [x9]");                                        // stash the peer address length
    emitter.instruction("ldr x0, [sp, #200]");                                  // return the accepted descriptor
    emitter.instruction("ldp x29, x30, [sp, #0]");                              // restore frame pointer and return address
    emitter.instruction("add sp, sp, #224");                                    // release the helper frame
    emitter.instruction("ret");                                                 // return the accepted fd

    emitter.label("__rt_ssa_fail_clear");
    abi::emit_symbol_address(emitter, "x9", "_accept_peer_ptr");
    emitter.instruction("str xzr, [x9]");                                       // clear the stashed peer address pointer
    abi::emit_symbol_address(emitter, "x9", "_accept_peer_len");
    emitter.instruction("str xzr, [x9]");                                       // clear the stashed peer address length
    emitter.instruction("mov x0, #-1");                                         // -1 reports a failed / timed-out accept
    emitter.instruction("ldp x29, x30, [sp, #0]");                              // restore frame pointer and return address
    emitter.instruction("add sp, sp, #224");                                    // release the helper frame
    emitter.instruction("ret");                                                 // return the failure result
}

/// Emits the Linux x86_64 stream runtime helper for stream socket accept.
fn emit_stream_socket_accept_linux_x86_64(emitter: &mut Emitter) {
    let family_off = family_byte_offset(Platform::Linux);
    emitter.blank();
    emitter.comment("--- runtime: stream_socket_accept ---");
    emitter.label_global("__rt_stream_socket_accept");

    // Frame (rbp-relative):
    //   [-8]   saved fd
    //   [-16]  saved timeout_us
    //   [-144) sockaddr buffer (128 bytes)
    //   [-152) addrlen (4 + 4 pad)
    //   [-160) pollfd: fd (4), events (2), revents (2)
    //   [-168) padding
    //   [-184) timespec: seconds field
    //   [-176) timespec: nanoseconds field
    //   [-192) accepted fd
    emitter.instruction("push rbp");                                            // preserve the caller frame pointer
    emitter.instruction("mov rbp, rsp");                                        // establish the helper frame pointer
    emitter.instruction("sub rsp, 192");                                        // allocate the helper frame
    emitter.instruction("mov QWORD PTR [rbp - 8], rdi");                        // save the listening fd
    emitter.instruction("mov QWORD PTR [rbp - 16], rsi");                       // save the timeout in microseconds

    // -- timeout < 0 skips the poll gate and blocks indefinitely --
    emitter.instruction("cmp rsi, 0");                                          // is the timeout negative (infinite)?
    emitter.instruction("jl __rt_ssa_blocking_accept_x86");                     // jump straight to accept for the infinite case

    // -- one pollfd for the listening descriptor: events = POLLIN, revents = 0 --
    emitter.instruction("test edi, edi");                                       // poll ignores a negative descriptor instead of failing
    emitter.instruction("js __rt_ssa_fail_clear_x86");                          // not a descriptor: bail out
    emitter.instruction("mov DWORD PTR [rbp - 160], edi");                      // pollfd.fd = the listening descriptor
    emitter.instruction("mov DWORD PTR [rbp - 156], 1");                        // pollfd.events = POLLIN, pollfd.revents = 0

    // -- build the timespec from timeout_us --
    emitter.instruction("mov rax, QWORD PTR [rbp - 16]");                       // reload the timeout in microseconds
    emitter.instruction("xor edx, edx");                                        // clear the division high word
    emitter.instruction("mov rcx, 1000000");                                    // one second in microseconds
    emitter.instruction("div rcx");                                             // rax = seconds, rdx = micro_remainder
    emitter.instruction("mov QWORD PTR [rbp - 184], rax");                      // store the timespec seconds field
    emitter.instruction("imul rdx, rdx, 1000");                                 // convert microseconds to nanoseconds
    emitter.instruction("mov QWORD PTR [rbp - 176], rdx");                      // store the timespec nanoseconds field

    // -- ppoll(&pollfd, nfds=1, &timespec, sigmask=NULL, sigsetsize=0) --
    emitter.instruction("lea rdi, [rbp - 160]");                                // pointer to the pollfd
    emitter.instruction("mov esi, 1");                                          // one descriptor
    emitter.instruction("lea rdx, [rbp - 184]");                                // pointer to the timespec
    emitter.instruction("xor r10d, r10d");                                      // signal mask pointer (NULL)
    emitter.instruction("xor r8d, r8d");                                        // signal mask size
    emitter.instruction("mov eax, 271");                                        // Linux x86_64 syscall 271 = ppoll
    emitter.instruction("syscall");                                             // wait for a pending connection
    emitter.instruction("cmp rax, 0");                                          // did ppoll fail or time out?
    emitter.instruction("jle __rt_ssa_fail_clear_x86");                         // 0 = timeout, <0 = error → report failure

    emitter.label("__rt_ssa_blocking_accept_x86");
    // -- accept(fd, &sockaddr, &addrlen) --
    emitter.instruction("mov DWORD PTR [rbp - 152], 128");                      // addrlen in/out parameter = 128
    emitter.instruction("mov rdi, QWORD PTR [rbp - 8]");                        // reload the listening fd
    emitter.instruction("lea rsi, [rbp - 144]");                                // pointer to the peer sockaddr buffer
    emitter.instruction("lea rdx, [rbp - 152]");                                // pointer to the addrlen parameter
    emitter.instruction("mov eax, 43");                                         // Linux x86_64 syscall 43 = accept
    emitter.instruction("syscall");                                             // accept the connection
    emitter.instruction("test rax, rax");                                       // did accept() return a valid descriptor?
    emitter.instruction("js __rt_ssa_fail_clear_x86");                          // accept() failed: clear globals and report -1
    emitter.instruction("mov QWORD PTR [rbp - 192], rax");                      // save the accepted fd

    // -- dispatch on the captured sockaddr's address family --
    emitter.instruction("movsxd r10, DWORD PTR [rbp - 152]");                   // reload addrlen returned by accept (signed widen)
    emitter.instruction("cmp r10, 2");                                          // is there at least a family-byte's worth of data?
    emitter.instruction("jl __rt_ssa_empty_peer_x86");                          // no peer info: empty $peer_name
    let family_addr_disp = -144 + family_off;
    emitter.instruction(&format!(                                               // load the address-family discriminator
        "movzx r11d, BYTE PTR [rbp - {}]",
        -family_addr_disp
    ));
    let af_inet6_linux = Platform::Linux.af_inet6();
    emitter.instruction("cmp r11d, 2");                                         // AF_INET = 2 on Linux
    emitter.instruction("je __rt_ssa_peer_inet_x86");                           // IPv4 peer: format A.B.C.D:port
    emitter.instruction("cmp r11d, 1");                                         // AF_UNIX = 1 on Linux
    emitter.instruction("je __rt_ssa_peer_unix_x86");                           // Unix-domain peer: format sun_path
    emitter.instruction(&format!("cmp r11d, {}", af_inet6_linux));              // AF_INET6 = 10 on Linux
    emitter.instruction("je __rt_ssa_peer_inet6_x86");                          // IPv6 peer: format [ipv6]:port
    emitter.instruction("jmp __rt_ssa_empty_peer_x86");                         // unsupported family: empty $peer_name

    // -- AF_INET peer: render via __rt_format_sockaddr_in --
    emitter.label("__rt_ssa_peer_inet_x86");
    emitter.instruction("lea rdi, [rbp - 144]");                                // pointer to the captured peer sockaddr
    emitter.instruction("call __rt_format_sockaddr_in");                        // rax = address string, rdx = length
    emitter.instruction("jmp __rt_ssa_stash_peer_x86");                         // common stash + return

    // -- AF_INET6 peer: render via __rt_format_sockaddr_in6 --
    emitter.label("__rt_ssa_peer_inet6_x86");
    emitter.instruction("lea rdi, [rbp - 144]");                                // pointer to the captured peer sockaddr_in6
    emitter.instruction("call __rt_format_sockaddr_in6");                       // rax = address string, rdx = length
    emitter.instruction("jmp __rt_ssa_stash_peer_x86");                         // common stash + return

    // -- AF_UNIX peer: render via __rt_format_sockaddr_unix --
    emitter.label("__rt_ssa_peer_unix_x86");
    emitter.instruction("lea rdi, [rbp - 144]");                                // pointer to the captured peer sockaddr
    emitter.instruction("movsxd rsi, DWORD PTR [rbp - 152]");                   // addrlen returned by accept
    emitter.instruction("call __rt_format_sockaddr_unix");                      // rax = sun_path string, rdx = length
    emitter.instruction("jmp __rt_ssa_stash_peer_x86");                         // common stash + return

    // -- empty $peer_name: connected/unnamed peer or unsupported family --
    emitter.label("__rt_ssa_empty_peer_x86");
    emitter.instruction("mov rax, 1");                                          // alloc one byte so the heap header is well-formed
    emitter.instruction("call __rt_heap_alloc");                                // rax = single-byte buffer
    emitter.instruction(&format!("mov r10, 0x{:x}", crate::codegen_support::sentinels::x86_64_heap_kind_word(1))); // owned-string heap-kind word with the x86_64 heap marker
    emitter.instruction("mov QWORD PTR [rax - 8], r10");                        // stamp the buffer as an owned string
    emitter.instruction("xor edx, edx");                                        // empty address length

    emitter.label("__rt_ssa_stash_peer_x86");
    abi::emit_symbol_address(emitter, "r10", "_accept_peer_ptr");               // address of the stashed-pointer global
    emitter.instruction("mov QWORD PTR [r10], rax");                            // stash the peer address pointer
    abi::emit_symbol_address(emitter, "r10", "_accept_peer_len");               // address of the stashed-length global
    emitter.instruction("mov QWORD PTR [r10], rdx");                            // stash the peer address length
    emitter.instruction("mov rax, QWORD PTR [rbp - 192]");                      // return the accepted descriptor
    emitter.instruction("add rsp, 192");                                        // release the helper frame
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer
    emitter.instruction("ret");                                                 // return the accepted fd

    emitter.label("__rt_ssa_fail_clear_x86");
    abi::emit_symbol_address(emitter, "r10", "_accept_peer_ptr");               // address of the stashed-pointer global
    emitter.instruction("mov QWORD PTR [r10], 0");                              // clear the stashed peer address pointer
    abi::emit_symbol_address(emitter, "r10", "_accept_peer_len");               // address of the stashed-length global
    emitter.instruction("mov QWORD PTR [r10], 0");                              // clear the stashed peer address length
    emitter.instruction("mov rax, -1");                                         // -1 reports a failed / timed-out accept
    emitter.instruction("add rsp, 192");                                        // release the helper frame
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer
    emitter.instruction("ret");                                                 // return the failure result
}
