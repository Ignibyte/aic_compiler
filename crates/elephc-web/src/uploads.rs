//! Purpose:
//! Tracks the temporary files a `multipart/form-data` request's uploads were written to, so
//! `is_uploaded_file()` and `move_uploaded_file()` accept only those paths, and removes the ones
//! the handler did not move once the request ends, as PHP does.
//!
//! Called from:
//! - The `--web` prelude: `$_FILES` construction registers each temp file, and the two upload
//!   functions check and forget paths.
//! - `crate::worker::run_handler` and the isolated handler child, after the handler returns.
//!
//! Key details:
//! - One handler runs at a time in every owning process, so the registry needs no lock; it is
//!   reached only through raw pointers, as in `crate::request_state`.

use std::ffi::{c_char, CStr};

/// Temp-file paths of the current request's uploads that are still unmoved.
static mut UPLOADED_FILES: Vec<String> = Vec::new();

/// Reads a borrowed NUL-terminated path, or `None` for a null pointer.
unsafe fn path_from(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    Some(CStr::from_ptr(ptr).to_string_lossy().into_owned())
}

/// Records `path` as a temp file holding one of this request's uploads.
///
/// # Safety
/// `path` is null or a valid NUL-terminated string. Single-threaded per worker.
#[no_mangle]
pub unsafe extern "C" fn elephc_web_upload_register(path: *const c_char) {
    if let Some(path) = path_from(path) {
        (*core::ptr::addr_of_mut!(UPLOADED_FILES)).push(path);
    }
}

/// Returns 1 when `path` is an unmoved upload of this request, else 0.
///
/// # Safety
/// `path` is null or a valid NUL-terminated string. Single-threaded per worker.
#[no_mangle]
pub unsafe extern "C" fn elephc_web_upload_is(path: *const c_char) -> i64 {
    let Some(path) = path_from(path) else {
        return 0;
    };
    i64::from((*core::ptr::addr_of!(UPLOADED_FILES)).contains(&path))
}

/// Forgets `path` after `move_uploaded_file()` moved it, so it is not removed at request end.
///
/// # Safety
/// `path` is null or a valid NUL-terminated string. Single-threaded per worker.
#[no_mangle]
pub unsafe extern "C" fn elephc_web_upload_forget(path: *const c_char) {
    if let Some(path) = path_from(path) {
        (*core::ptr::addr_of_mut!(UPLOADED_FILES)).retain(|uploaded| uploaded != &path);
    }
}

/// Removes the uploads the handler left in place, as PHP does when a request ends.
pub(crate) fn discard_unmoved() {
    // SAFETY: single-threaded per worker; the registry is replaced through a raw pointer.
    let unmoved = unsafe { core::mem::take(&mut *core::ptr::addr_of_mut!(UPLOADED_FILES)) };
    for path in unmoved {
        let _ = std::fs::remove_file(path);
    }
}
