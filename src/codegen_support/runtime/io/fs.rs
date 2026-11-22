//! Purpose:
//! Emits the `__rt_unlink`, `__rt_cstr` runtime helper assembly for fs.
//! Keeps PHP filesystem/resource behavior, libc calls, and target-specific ABI variants in one focused emitter.
//!
//! Called from:
//! - `crate::codegen_support::runtime::emitters::emit_runtime()` via `crate::codegen_support::runtime::io`.
//!
//! Key details:
//! - I/O helpers bridge PHP strings, resources, descriptors, and libc calls while returning runtime arrays or pointer/length strings.
//! - `__rt_rename` falls back to copy, chmod and unlink when libc `rename()` reports `EXDEV`.

use crate::codegen_support::{emit::Emitter, platform::{Arch, Platform}};

/// POSIX `EXDEV`, the errno `rename()` sets when the two paths are on different filesystems.
/// The value is 18 on every platform in the supported target matrix.
const EXDEV: i32 = 18;

/// Returns the libc symbol that yields the address of the thread-local `errno` slot.
///
/// Same platform split as the other runtime helpers that read errno after a libc call
/// (`crate::codegen_support::runtime::io::streams_ext`).
fn errno_location_symbol(emitter: &Emitter) -> &'static str {
    match emitter.platform {
        Platform::MacOS => "__error",
        Platform::Linux => "__errno_location",
        Platform::Windows => panic!("Windows target is not yet supported (see issue #379)"),
    }
}

/// Emits all filesystem runtime helpers: `__rt_unlink`, `__rt_mkdir`, `__rt_rmdir`,
/// `__rt_chdir`, `__rt_rename`, and `__rt_copy`.
///
/// Dispatches to `emit_fs_linux_x86_64` on x86_64 Linux; emits ARM64 syscall-based
/// helpers on all other targets. Each helper takes PHP string path arguments (x1=ptr,
/// x2=len) and returns x0=1 on success, 0 on failure.
pub fn emit_fs(emitter: &mut Emitter) {
    if emitter.target.arch == Arch::X86_64 {
        emit_fs_linux_x86_64(emitter);
        return;
    }

    // ================================================================
    // __rt_unlink: delete a file
    // Input:  x1/x2=path
    // Output: x0=1 on success, 0 on failure
    // ================================================================
    emitter.blank();
    emitter.comment("--- runtime: unlink ---");
    emitter.label_global("__rt_unlink");

    // -- set up stack frame --
    emitter.instruction("sub sp, sp, #16");                                     // allocate 16 bytes on the stack
    emitter.instruction("stp x29, x30, [sp]");                                  // save frame pointer and return address
    emitter.instruction("mov x29, sp");                                         // establish new frame pointer

    // -- null-terminate path and call unlink --
    emitter.instruction("bl __rt_cstr");                                        // convert path to C string, x0=cstr
    emitter.syscall(10);

    // -- return success/failure --
    emitter.instruction("cmp x0, #0");                                          // check syscall result
    emitter.instruction("cset x0, eq");                                         // x0 = 1 if unlink succeeded

    // -- restore frame and return --
    emitter.instruction("ldp x29, x30, [sp]");                                  // restore frame pointer and return address
    emitter.instruction("add sp, sp, #16");                                     // deallocate stack frame
    emitter.instruction("ret");                                                 // return to caller

    // ================================================================
    // __rt_mkdir: create a directory with PHP's default permissions
    // Input:  x1/x2=path
    // Output: x0=1 on success, 0 on failure
    //
    // __rt_mkdir_ex: the same, with PHP's $permissions and $recursive
    // Input:  x1/x2=path, x3=mode, x4=recursive
    // Output: x0=1 on success, 0 on failure
    //
    // The mode is PHP's 0777 default rather than the 0755 this used to hard-code: the
    // syscall applies the process umask, which is what PHP relies on, so the usual
    // umask 022 still yields 0755 while `umask(0)` now behaves as it does in PHP
    // (issue #506).
    // ================================================================
    emitter.blank();
    emitter.comment("--- runtime: mkdir ---");
    emitter.label_global("__rt_mkdir");
    emitter.instruction("mov x3, #0x1FF");                                      // PHP's default $permissions, 0777, masked by umask
    emitter.instruction("mov x4, #0");                                          // PHP's default $recursive, false
    emitter.instruction("b __rt_mkdir_ex");                                     // share one body with the argument-taking entry point

    emitter.blank();
    emitter.comment("--- runtime: mkdir (with PHP $permissions and $recursive) ---");
    emitter.label_global("__rt_mkdir_ex");

    // -- set up stack frame --
    emitter.instruction("sub sp, sp, #48");                                     // allocate slots for mode, recursive, path base and cursor
    emitter.instruction("stp x29, x30, [sp, #32]");                             // save frame pointer and return address
    emitter.instruction("add x29, sp, #32");                                    // establish new frame pointer
    emitter.instruction("str x3, [sp, #0]");                                    // save the requested mode across the cstr call
    emitter.instruction("str x4, [sp, #8]");                                    // save the recursive flag across the cstr call

    // -- null-terminate the path --
    emitter.instruction("bl __rt_cstr");                                        // convert path to C string, x0=cstr
    emitter.instruction("str x0, [sp, #16]");                                   // save the C path base
    emitter.instruction("ldr x4, [sp, #8]");                                    // reload the recursive flag
    emitter.instruction("cbz x4, __rt_mkdir_final");                            // non-recursive: create the path itself and report
    emitter.instruction("str x0, [sp, #24]");                                   // recursive: start the separator walk at the base

    // -- create each parent component, ignoring the ones that already exist --
    emitter.label("__rt_mkdir_walk");
    emitter.instruction("ldr x9, [sp, #24]");                                   // reload the walk cursor
    emitter.instruction("ldrb w10, [x9]");                                      // read the byte under the cursor
    emitter.instruction("cbz w10, __rt_mkdir_final");                           // end of path: the last component is created below
    emitter.instruction("cmp w10, #47");                                        // '/' separates the components
    emitter.instruction("b.ne __rt_mkdir_walk_next");                           // an ordinary byte: keep scanning
    emitter.instruction("ldr x11, [sp, #16]");                                  // reload the path base
    emitter.instruction("cmp x9, x11");                                         // is this the leading '/' of an absolute path?
    emitter.instruction("b.eq __rt_mkdir_walk_next");                           // the root always exists, and mkdir("") is not a request
    emitter.instruction("strb wzr, [x9]");                                      // terminate the path at this separator
    emitter.instruction("ldr x0, [sp, #16]");                                   // pass the parent prefix to mkdir
    emitter.instruction("ldr x1, [sp, #0]");                                    // pass the requested mode
    emitter.syscall(136);
    emitter.instruction("ldr x9, [sp, #24]");                                   // reload the cursor, which the syscall may have clobbered
    emitter.instruction("mov w10, #47");                                        // restore the separator byte
    emitter.instruction("strb w10, [x9]");                                      // put '/' back so the path is whole again
    emitter.label("__rt_mkdir_walk_next");
    emitter.instruction("ldr x9, [sp, #24]");                                   // reload the walk cursor
    emitter.instruction("add x9, x9, #1");                                      // advance one byte
    emitter.instruction("str x9, [sp, #24]");                                   // store the advanced cursor
    emitter.instruction("b __rt_mkdir_walk");                                   // keep walking the path

    // -- create the requested directory itself, whose result is the return value --
    emitter.label("__rt_mkdir_final");
    emitter.instruction("ldr x0, [sp, #16]");                                   // pass the whole path to mkdir
    emitter.instruction("ldr x1, [sp, #0]");                                    // pass the requested mode
    emitter.syscall(136);

    // -- return success/failure --
    emitter.instruction("cmp x0, #0");                                          // check syscall result
    emitter.instruction("cset x0, eq");                                         // x0 = 1 if mkdir succeeded

    // -- restore frame and return --
    emitter.instruction("ldp x29, x30, [sp, #32]");                             // restore frame pointer and return address
    emitter.instruction("add sp, sp, #48");                                     // deallocate stack frame
    emitter.instruction("ret");                                                 // return to caller

    // ================================================================
    // __rt_rmdir: remove a directory
    // Input:  x1/x2=path
    // Output: x0=1 on success, 0 on failure
    // ================================================================
    emitter.blank();
    emitter.comment("--- runtime: rmdir ---");
    emitter.label_global("__rt_rmdir");

    // -- set up stack frame --
    emitter.instruction("sub sp, sp, #16");                                     // allocate 16 bytes on the stack
    emitter.instruction("stp x29, x30, [sp]");                                  // save frame pointer and return address
    emitter.instruction("mov x29, sp");                                         // establish new frame pointer

    // -- null-terminate path and call rmdir --
    emitter.instruction("bl __rt_cstr");                                        // convert path to C string, x0=cstr
    emitter.syscall(137);

    // -- return success/failure --
    emitter.instruction("cmp x0, #0");                                          // check syscall result
    emitter.instruction("cset x0, eq");                                         // x0 = 1 if rmdir succeeded

    // -- restore frame and return --
    emitter.instruction("ldp x29, x30, [sp]");                                  // restore frame pointer and return address
    emitter.instruction("add sp, sp, #16");                                     // deallocate stack frame
    emitter.instruction("ret");                                                 // return to caller

    // ================================================================
    // __rt_chdir: change working directory
    // Input:  x1/x2=path
    // Output: x0=1 on success, 0 on failure
    // ================================================================
    emitter.blank();
    emitter.comment("--- runtime: chdir ---");
    emitter.label_global("__rt_chdir");

    // -- set up stack frame --
    emitter.instruction("sub sp, sp, #16");                                     // allocate 16 bytes on the stack
    emitter.instruction("stp x29, x30, [sp]");                                  // save frame pointer and return address
    emitter.instruction("mov x29, sp");                                         // establish new frame pointer

    // -- null-terminate path and call chdir --
    emitter.instruction("bl __rt_cstr");                                        // convert path to C string, x0=cstr
    emitter.syscall(12);

    // -- return success/failure --
    emitter.instruction("cmp x0, #0");                                          // check syscall result
    emitter.instruction("cset x0, eq");                                         // x0 = 1 if chdir succeeded

    // -- restore frame and return --
    emitter.instruction("ldp x29, x30, [sp]");                                  // restore frame pointer and return address
    emitter.instruction("add sp, sp, #16");                                     // deallocate stack frame
    emitter.instruction("ret");                                                 // return to caller

    // ================================================================
    // __rt_rename: rename a file or directory
    // Input:  x1/x2=from path, x3/x4=to path
    // Output: x0=1 on success, 0 on failure
    // Across filesystems (EXDEV) a regular file is copied, given the source's
    // permissions and unlinked, as PHP's plain-files rename does.
    // ================================================================
    emitter.blank();
    emitter.comment("--- runtime: rename ---");
    emitter.label_global("__rt_rename");

    // -- set up stack frame --
    emitter.instruction("sub sp, sp, #64");                                     // allocate the frame, both path pairs and two scratch slots
    emitter.instruction("stp x29, x30, [sp, #48]");                             // save frame pointer and return address
    emitter.instruction("add x29, sp, #48");                                    // establish new frame pointer

    // -- keep both elephc paths for the cross-filesystem fallback --
    emitter.instruction("stp x1, x2, [sp, #16]");                               // save 'from' path ptr and len on stack
    emitter.instruction("stp x3, x4, [sp, #32]");                               // save 'to' path ptr and len on stack

    // -- null-terminate both paths and call libc rename(), which sets errno --
    emitter.instruction("bl __rt_cstr");                                        // convert 'from' to C string in _cstr_buf
    emitter.instruction("str x0, [sp, #0]");                                    // save source cstr pointer
    emitter.instruction("ldp x1, x2, [sp, #32]");                               // reload 'to' path ptr and len
    emitter.instruction("bl __rt_cstr2");                                       // convert 'to' to C string in _cstr_buf2
    emitter.instruction("mov x1, x0");                                          // destination cstr is the second rename() argument
    emitter.instruction("ldr x0, [sp, #0]");                                    // source cstr is the first rename() argument
    emitter.bl_c("rename");
    emitter.instruction("cmp w0, #0");                                          // did rename() return success as a C int?
    emitter.instruction("b.eq __rt_rename_ok");                                 // renamed in place

    // -- only a cross-filesystem failure falls back to copy and unlink --
    emitter.bl_c(errno_location_symbol(emitter));
    emitter.instruction("ldr w9, [x0]");                                        // load the errno rename() left
    emitter.instruction(&format!("cmp w9, #{}", EXDEV));                        // did it fail because the paths are on different filesystems?
    emitter.instruction("b.ne __rt_rename_fail");                               // any other failure is final
    emitter.instruction("ldp x1, x2, [sp, #16]");                               // reload 'from' path ptr and len
    emitter.instruction("bl __rt_fileperms");                                   // x0 = source st_mode, x1 = stat success flag
    emitter.instruction("cbz x1, __rt_rename_fail");                            // a source that cannot be stat'ed cannot be copied
    emitter.instruction("and w9, w0, #0xf000");                                 // isolate the file-type bits of st_mode
    emitter.instruction("mov w10, #0x8000");                                    // S_IFREG
    emitter.instruction("cmp w9, w10");                                         // only a regular file is copied across filesystems
    emitter.instruction("b.ne __rt_rename_fail");                               // a directory or special file stays where it is
    emitter.instruction("and x0, x0, #0xfff");                                  // keep the permission bits for the copy
    emitter.instruction("str x0, [sp, #8]");                                    // save them across the copy
    emitter.instruction("ldp x1, x2, [sp, #16]");                               // copy from the source path
    emitter.instruction("ldp x3, x4, [sp, #32]");                               // to the destination path
    emitter.instruction("bl __rt_copy");                                        // x0 = 1 when the destination holds the source's bytes
    emitter.instruction("cbz x0, __rt_rename_fail");                            // leave the source in place when the copy failed
    emitter.instruction("ldp x1, x2, [sp, #32]");                               // chmod the destination path
    emitter.instruction("ldr x3, [sp, #8]");                                    // to the source's permission bits
    emitter.instruction("bl __rt_chmod");                                       // give the copy the source's permissions
    emitter.instruction("ldp x1, x2, [sp, #16]");                               // reload 'from' path ptr and len
    emitter.instruction("bl __rt_unlink");                                      // x0 = 1 when the source is gone, completing the move
    emitter.instruction("b __rt_rename_done");                                  // return the unlink result

    emitter.label("__rt_rename_ok");
    emitter.instruction("mov x0, #1");                                          // rename succeeded
    emitter.instruction("b __rt_rename_done");                                  // share the epilogue
    emitter.label("__rt_rename_fail");
    emitter.instruction("mov x0, #0");                                          // rename failed

    // -- restore frame and return --
    emitter.label("__rt_rename_done");
    emitter.instruction("ldp x29, x30, [sp, #48]");                             // restore frame pointer and return address
    emitter.instruction("add sp, sp, #64");                                     // deallocate stack frame
    emitter.instruction("ret");                                                 // return to caller

    // ================================================================
    // __rt_copy: copy a file
    // Input:  x1/x2=from path, x3/x4=to path
    // Output: x0=1 on success, 0 on failure
    // ================================================================
    emitter.blank();
    emitter.comment("--- runtime: copy ---");
    emitter.label_global("__rt_copy");

    // -- set up stack frame --
    emitter.instruction("sub sp, sp, #48");                                     // allocate 48 bytes on the stack
    emitter.instruction("stp x29, x30, [sp, #32]");                             // save frame pointer and return address
    emitter.instruction("add x29, sp, #32");                                    // establish new frame pointer

    // -- save destination path for after reading source --
    emitter.instruction("stp x3, x4, [sp, #16]");                               // save 'to' path ptr and len on stack

    // -- read source file contents --
    emitter.instruction("bl __rt_file_get_contents");                           // read source, x1=data ptr, x2=data len

    // -- write contents to destination file --
    emitter.instruction("mov x3, x1");                                          // move data ptr to x3 (data arg)
    emitter.instruction("mov x4, x2");                                          // move data len to x4 (data arg)
    emitter.instruction("ldp x1, x2, [sp, #16]");                               // reload destination path ptr and len
    emitter.instruction("bl __rt_file_put_contents");                           // write data to dest file, x0=bytes written

    // -- return 1 if bytes were written --
    emitter.instruction("cmp x0, #0");                                          // check if any bytes were written
    emitter.instruction("cset x0, gt");                                         // x0 = 1 if bytes written > 0

    // -- restore frame and return --
    emitter.instruction("ldp x29, x30, [sp, #32]");                             // restore frame pointer and return address
    emitter.instruction("add sp, sp, #48");                                     // deallocate stack frame
    emitter.instruction("ret");                                                 // return to caller
}

/// Emits x86_64 Linux variants of all filesystem helpers using libc calls.
/// Uses a stack-based frame (rbp/rsp convention) instead of the ARM64 link-register frame.
/// Return value convention matches `emit_fs`: x0=1 on success, 0 on failure.
fn emit_fs_linux_x86_64(emitter: &mut Emitter) {
    emitter.blank();
    emitter.comment("--- runtime: unlink ---");
    emitter.label_global("__rt_unlink");
    emit_single_path_libc_bool_helper(emitter, "unlink", None);

    emitter.blank();
    emitter.comment("--- runtime: mkdir ---");
    emitter.label_global("__rt_mkdir");
    emitter.instruction("mov rcx, 0x1FF");                                      // PHP's default $permissions, 0777, masked by umask
    emitter.instruction("xor r8d, r8d");                                        // PHP's default $recursive, false
    emitter.instruction("jmp __rt_mkdir_ex");                                   // share one body with the argument-taking entry point

    emitter.blank();
    emitter.comment("--- runtime: mkdir (with PHP $permissions and $recursive) ---");
    emitter.label_global("__rt_mkdir_ex");
    emit_mkdir_ex_linux_x86_64(emitter);

    emitter.blank();
    emitter.comment("--- runtime: rmdir ---");
    emitter.label_global("__rt_rmdir");
    emit_single_path_libc_bool_helper(emitter, "rmdir", None);

    emitter.blank();
    emitter.comment("--- runtime: chdir ---");
    emitter.label_global("__rt_chdir");
    emit_single_path_libc_bool_helper(emitter, "chdir", None);

    emitter.blank();
    emitter.comment("--- runtime: rename ---");
    emitter.label_global("__rt_rename");
    emitter.instruction("push rbp");                                            // preserve the caller frame pointer while rename uses temporary path slots
    emitter.instruction("mov rbp, rsp");                                        // establish a stable frame base for the path pairs and C-string temporaries
    emitter.instruction("sub rsp, 48");                                         // reserve aligned stack space for both elephc paths and both C-string pointers
    emitter.instruction("mov QWORD PTR [rbp - 8], rdi");                        // save the destination elephc path pointer while converting the source path
    emitter.instruction("mov QWORD PTR [rbp - 16], rsi");                       // save the destination elephc path length while converting the source path
    emitter.instruction("mov QWORD PTR [rbp - 40], rax");                       // keep the source elephc path pointer for the cross-filesystem fallback
    emitter.instruction("mov QWORD PTR [rbp - 48], rdx");                       // keep the source elephc path length for the cross-filesystem fallback
    emitter.instruction("call __rt_cstr");                                      // convert the source elephc path in rax/rdx into a null-terminated C string
    emitter.instruction("mov QWORD PTR [rbp - 24], rax");                       // save the source C-string pointer for the later libc rename() call
    emitter.instruction("mov rax, QWORD PTR [rbp - 8]");                        // reload the destination elephc path pointer before converting it to a C string
    emitter.instruction("mov rdx, QWORD PTR [rbp - 16]");                       // reload the destination elephc path length before converting it to a C string
    emitter.instruction("call __rt_cstr2");                                     // convert the destination elephc path into the secondary null-terminated C string buffer
    emitter.instruction("mov QWORD PTR [rbp - 32], rax");                       // save the destination C-string pointer for the later libc rename() call
    emitter.instruction("mov rdi, QWORD PTR [rbp - 24]");                       // pass the source C-string pointer as the first libc rename() argument
    emitter.instruction("mov rsi, QWORD PTR [rbp - 32]");                       // pass the destination C-string pointer as the second libc rename() argument
    emitter.instruction("call rename");                                         // rename or move the file-system path through libc rename()
    emitter.instruction("cmp eax, 0");                                          // a successful libc rename() call returns zero as a C int
    emitter.instruction("je __rt_rename_ok_linux_x86_64");                      // renamed in place
    emitter.instruction("call __errno_location");                               // rax = address of the errno rename() left
    emitter.instruction(&format!("cmp DWORD PTR [rax], {}", EXDEV));            // did it fail because the paths are on different filesystems?
    emitter.instruction("jne __rt_rename_fail_linux_x86_64");                   // any other failure is final
    emitter.instruction("mov rax, QWORD PTR [rbp - 40]");                       // stat the source path
    emitter.instruction("mov rdx, QWORD PTR [rbp - 48]");                       // with its exact byte length
    emitter.instruction("call __rt_fileperms");                                 // rax = source st_mode, rdx = stat success flag
    emitter.instruction("test rdx, rdx");                                       // could the source be stat'ed?
    emitter.instruction("jz __rt_rename_fail_linux_x86_64");                    // a source that cannot be stat'ed cannot be copied
    emitter.instruction("mov ecx, eax");                                        // copy st_mode to test its file-type bits
    emitter.instruction("and ecx, 0xf000");                                     // isolate the file-type bits of st_mode
    emitter.instruction("cmp ecx, 0x8000");                                     // only a regular file (S_IFREG) is copied across filesystems
    emitter.instruction("jne __rt_rename_fail_linux_x86_64");                   // a directory or special file stays where it is
    emitter.instruction("and eax, 0xfff");                                      // keep the permission bits for the copy
    emitter.instruction("mov QWORD PTR [rbp - 24], rax");                       // save them across the copy (the source C string is no longer needed)
    emitter.instruction("mov rax, QWORD PTR [rbp - 40]");                       // copy from the source elephc path pointer
    emitter.instruction("mov rdx, QWORD PTR [rbp - 48]");                       // and its length
    emitter.instruction("mov rdi, QWORD PTR [rbp - 8]");                        // to the destination elephc path pointer
    emitter.instruction("mov rsi, QWORD PTR [rbp - 16]");                       // and its length
    emitter.instruction("call __rt_copy");                                      // rax = 1 when the destination holds the source's bytes
    emitter.instruction("test rax, rax");                                       // did the copy succeed?
    emitter.instruction("jz __rt_rename_fail_linux_x86_64");                    // leave the source in place when the copy failed
    emitter.instruction("mov rax, QWORD PTR [rbp - 8]");                        // chmod the destination elephc path pointer
    emitter.instruction("mov rdx, QWORD PTR [rbp - 16]");                       // and its length
    emitter.instruction("mov rdi, QWORD PTR [rbp - 24]");                       // to the source's permission bits
    emitter.instruction("call __rt_chmod");                                     // give the copy the source's permissions
    emitter.instruction("mov rax, QWORD PTR [rbp - 40]");                       // unlink the source elephc path pointer
    emitter.instruction("mov rdx, QWORD PTR [rbp - 48]");                       // and its length
    emitter.instruction("call __rt_unlink");                                    // rax = 1 when the source is gone, completing the move
    emitter.instruction("jmp __rt_rename_done_linux_x86_64");                   // return the unlink result
    emitter.label("__rt_rename_ok_linux_x86_64");
    emitter.instruction("mov eax, 1");                                          // rename succeeded
    emitter.instruction("jmp __rt_rename_done_linux_x86_64");                   // share the epilogue
    emitter.label("__rt_rename_fail_linux_x86_64");
    emitter.instruction("xor eax, eax");                                        // rename failed
    emitter.label("__rt_rename_done_linux_x86_64");
    emitter.instruction("add rsp, 48");                                         // release the aligned stack locals used by rename()
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer
    emitter.instruction("ret");                                                 // return the rename() success predicate to the caller

    emitter.blank();
    emitter.comment("--- runtime: copy ---");
    emitter.label_global("__rt_copy");
    emitter.instruction("push rbp");                                            // preserve the caller frame pointer while copy() uses path and payload spill slots
    emitter.instruction("mov rbp, rsp");                                        // establish a stable frame base for the saved destination path and copied file payload
    emitter.instruction("sub rsp, 32");                                         // reserve aligned stack space for the destination path pair and copied payload pair
    emitter.instruction("mov QWORD PTR [rbp - 8], rdi");                        // save the destination elephc path pointer while the source file is read into owned storage
    emitter.instruction("mov QWORD PTR [rbp - 16], rsi");                       // save the destination elephc path length while the source file is read into owned storage
    emitter.instruction("call __rt_file_get_contents");                         // read the source file into an owned elephc string before writing it to the destination path
    emitter.instruction("mov QWORD PTR [rbp - 24], rax");                       // preserve the copied file payload pointer across the destination-path reload and write helper call
    emitter.instruction("mov QWORD PTR [rbp - 32], rdx");                       // preserve the copied file payload length across the destination-path reload and write helper call
    emitter.instruction("mov rax, QWORD PTR [rbp - 8]");                        // reload the destination elephc path pointer into the primary x86_64 string argument register
    emitter.instruction("mov rdx, QWORD PTR [rbp - 16]");                       // reload the destination elephc path length into the primary x86_64 string length register
    emitter.instruction("mov rdi, QWORD PTR [rbp - 24]");                       // pass the copied file payload pointer as the data pointer argument to file_put_contents()
    emitter.instruction("mov rsi, QWORD PTR [rbp - 32]");                       // pass the copied file payload length as the data length argument to file_put_contents()
    emitter.instruction("call __rt_file_put_contents");                         // write the copied file payload into the destination path through the shared file_put_contents() helper
    emitter.instruction("cmp rax, 0");                                          // treat zero-byte writes as success so empty files can still be copied correctly
    emitter.instruction("setge al");                                            // convert the signed write result into a boolean success byte where any non-negative byte count is success
    emitter.instruction("movzx rax, al");                                       // widen the boolean success byte into the canonical integer result register
    emitter.instruction("add rsp, 32");                                         // release the aligned stack locals used by copy()
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer before returning the copy() success predicate
    emitter.instruction("ret");                                                 // return the copy() success predicate to the caller

}

/// Emits a leaf helper for single-path libc filesystem functions on x86_64.
///
/// Takes an optional setup instruction string inserted before the libc call to populate
/// extra arguments (e.g., mode for `mkdir`). The C path is passed via `__rt_cstr`
/// output in `rax`; the libc result is compared against 0 and returned as 1 (success) or
/// 0 (failure) in `rax`.

/// Emits the x86_64 body shared by `__rt_mkdir` and `__rt_mkdir_ex`.
///
/// Input: rax/rdx = path (the `__rt_cstr` ABI), rcx = mode, r8 = recursive.
/// Output: rax = 1 on success, 0 on failure.
///
/// The recursive walk creates each parent in turn by writing a NUL over the separator,
/// calling `mkdir`, and putting the separator back. Failures on the parents are ignored —
/// they are usually `EEXIST` — and only the final component decides the return value, which
/// is what PHP reports.
fn emit_mkdir_ex_linux_x86_64(emitter: &mut Emitter) {
    emitter.instruction("push rbp");                                            // preserve the caller frame pointer while the helper makes libc calls
    emitter.instruction("mov rbp, rsp");                                        // establish a stable frame base for the call-aligned helper body
    emitter.instruction("sub rsp, 32");                                         // reserve slots for mode, recursive, path base and cursor
    emitter.instruction("mov QWORD PTR [rbp - 8], rcx");                        // save the requested mode across the cstr call
    emitter.instruction("mov QWORD PTR [rbp - 16], r8");                        // save the recursive flag across the cstr call
    emitter.instruction("call __rt_cstr");                                      // convert the elephc path in rax/rdx into a null-terminated C string in rax
    emitter.instruction("mov QWORD PTR [rbp - 24], rax");                       // save the C path base
    emitter.instruction("mov rax, QWORD PTR [rbp - 16]");                       // reload the recursive flag
    emitter.instruction("test rax, rax");                                       // was recursive creation requested?
    emitter.instruction("jz __rt_mkdir_final_x86");                             // non-recursive: create the path itself and report
    emitter.instruction("mov rax, QWORD PTR [rbp - 24]");                       // recursive: start the separator walk at the base
    emitter.instruction("mov QWORD PTR [rbp - 32], rax");                       // store the initial walk cursor

    emitter.label("__rt_mkdir_walk_x86");
    emitter.instruction("mov r9, QWORD PTR [rbp - 32]");                        // reload the walk cursor
    emitter.instruction("movzx eax, BYTE PTR [r9]");                            // read the byte under the cursor
    emitter.instruction("test al, al");                                         // end of the path?
    emitter.instruction("jz __rt_mkdir_final_x86");                             // the last component is created below
    emitter.instruction("cmp al, 47");                                          // '/' separates the components
    emitter.instruction("jne __rt_mkdir_walk_next_x86");                        // an ordinary byte: keep scanning
    emitter.instruction("cmp r9, QWORD PTR [rbp - 24]");                        // is this the leading '/' of an absolute path?
    emitter.instruction("je __rt_mkdir_walk_next_x86");                         // the root always exists, and mkdir("") is not a request
    emitter.instruction("mov BYTE PTR [r9], 0");                                // terminate the path at this separator
    emitter.instruction("mov rdi, QWORD PTR [rbp - 24]");                       // pass the parent prefix to libc mkdir()
    emitter.instruction("mov rsi, QWORD PTR [rbp - 8]");                        // pass the requested mode
    emitter.instruction("call mkdir");                                          // create the parent, ignoring an existing one
    emitter.instruction("mov r9, QWORD PTR [rbp - 32]");                        // reload the cursor, which the call clobbered
    emitter.instruction("mov BYTE PTR [r9], 47");                               // put '/' back so the path is whole again
    emitter.label("__rt_mkdir_walk_next_x86");
    emitter.instruction("mov r9, QWORD PTR [rbp - 32]");                        // reload the walk cursor
    emitter.instruction("inc r9");                                              // advance one byte
    emitter.instruction("mov QWORD PTR [rbp - 32], r9");                        // store the advanced cursor
    emitter.instruction("jmp __rt_mkdir_walk_x86");                             // keep walking the path

    emitter.label("__rt_mkdir_final_x86");
    emitter.instruction("mov rdi, QWORD PTR [rbp - 24]");                       // pass the whole path to libc mkdir()
    emitter.instruction("mov rsi, QWORD PTR [rbp - 8]");                        // pass the requested mode
    emitter.instruction("call mkdir");                                          // create the requested directory itself
    emitter.instruction("cmp eax, 0");                                          // libc mkdir() returns zero as a C int on success
    emitter.instruction("sete al");                                             // convert the success code into a boolean byte
    emitter.instruction("movzx rax, al");                                       // widen the boolean byte into the canonical integer result register
    emitter.instruction("add rsp, 32");                                         // release the helper stack slots
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer after the libc helper returns
    emitter.instruction("ret");                                                 // return the file-system success predicate to the caller
}

/// Emits an x86_64 path-to-C-string libc call with aligned stack and boolean result normalization.
fn emit_single_path_libc_bool_helper(emitter: &mut Emitter, symbol: &str, extra_setup: Option<&str>) {
    emitter.instruction("push rbp");                                            // preserve the caller frame pointer while the helper makes libc calls
    emitter.instruction("mov rbp, rsp");                                        // establish a stable frame base for the call-aligned helper body
    emitter.instruction("call __rt_cstr");                                      // convert the elephc path in rax/rdx into a null-terminated C string in rax
    emitter.instruction("mov rdi, rax");                                        // pass the C path pointer as the first libc argument
    if let Some(setup) = extra_setup {
        emitter.instruction(setup);                                             // populate any additional libc arguments required by this helper
    }
    emitter.instruction(&format!("call {}", symbol));                           // invoke the matching libc file-system helper on Linux x86_64
    emitter.instruction("cmp eax, 0");                                          // libc path helpers return zero as a C int on success
    emitter.instruction("sete al");                                             // convert the success code into a boolean byte
    emitter.instruction("movzx rax, al");                                       // widen the boolean byte into the canonical integer result register
    emitter.instruction("pop rbp");                                             // restore the caller frame pointer after the libc helper returns
    emitter.instruction("ret");                                                 // return the file-system success predicate to the caller
}
