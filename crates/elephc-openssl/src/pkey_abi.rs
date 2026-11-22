//! Purpose:
//! Panic-contained C ABI for the asymmetric key bridge, declared by the compiler's
//! openssl prelude as `extern "elephc_openssl"` functions.
//!
//! Called from:
//! - `crate::openssl_prelude` in the compiler, through the prelude's extern block.
//! - `cargo test -p elephc-openssl`.
//!
//! Key details:
//! - Binary inputs cross as a pointer and an explicit length, so NUL bytes in a
//!   signature or a DER body survive; digest names cross as C strings.
//! - Results are kept in one thread-local buffer: a call returns the result's length,
//!   and the prelude copies the bytes at once through `elephc_openssl_pkey_result()`.
//! - Every entry point catches Rust panics and maps them to `PKEY_ERR_INTERNAL`.

use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::pkey::{details, load_private, load_public, sign, verify, PkeyError};

/// The key could not be parsed again from its normalized PEM.
pub const PKEY_ERR_BAD_KEY: i64 = -10;
/// The digest name is unknown.
pub const PKEY_ERR_UNKNOWN_DIGEST: i64 = -11;
/// A padding was given for a key that is not RSA.
pub const PKEY_ERR_PADDING_NEEDS_RSA: i64 = -12;
/// The padding is neither PKCS#1 v1.5 nor PSS.
pub const PKEY_ERR_UNKNOWN_PADDING: i64 = -13;
/// libcrypto refused the signature.
pub const PKEY_ERR_FAILED: i64 = -14;
/// A null pointer, a negative length or a caught panic.
pub const PKEY_ERR_INTERNAL: i64 = -18;

thread_local! {
    /// The last successful call's result bytes.
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Returns a checked byte slice for one C pointer/length pair.
unsafe fn bytes<'a>(ptr: *const u8, len: i64) -> Option<&'a [u8]> {
    let len = usize::try_from(len).ok()?;
    if len == 0 {
        Some(&[])
    } else if ptr.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(ptr, len))
    }
}

/// Returns a C string's bytes, `None` for a null pointer.
unsafe fn c_string<'a>(ptr: *const c_char) -> Option<&'a [u8]> {
    (!ptr.is_null()).then(|| CStr::from_ptr(ptr).to_bytes())
}

/// Stores a result and returns its length.
fn publish(result: Vec<u8>) -> i64 {
    let len = result.len() as i64;
    RESULT.with(|cell| *cell.borrow_mut() = result);
    len
}

/// Maps an engine refusal to its stable status code.
fn status(error: PkeyError) -> i64 {
    match error {
        PkeyError::BadKey => PKEY_ERR_BAD_KEY,
        PkeyError::UnknownDigest => PKEY_ERR_UNKNOWN_DIGEST,
        PkeyError::PaddingNeedsRsa => PKEY_ERR_PADDING_NEEDS_RSA,
        PkeyError::UnknownPadding => PKEY_ERR_UNKNOWN_PADDING,
        PkeyError::Failed => PKEY_ERR_FAILED,
    }
}

/// Loads a PEM key and stores its normalized PEM; returns its length, or 0 when the
/// text holds no key of the requested kind (`private` 1) or the passphrase is wrong.
///
/// # Safety
///
/// `data` and `passphrase` must point at `len` and `passphrase_len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn elephc_openssl_pkey_load(
    private: i64,
    data: *const u8,
    len: i64,
    passphrase: *const u8,
    passphrase_len: i64,
) -> i64 {
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(data), Some(passphrase)) = (bytes(data, len), bytes(passphrase, passphrase_len))
        else {
            return 0;
        };
        let loaded = if private != 0 { load_private(data, passphrase) } else { load_public(data) };
        loaded.map(publish).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// Signs `data` with a normalized private key PEM and stores the signature; returns its
/// length or a negative `PKEY_ERR_*` status.
///
/// # Safety
///
/// `pem` and `data` must point at `pem_len` and `len` readable bytes, and `digest` at a
/// NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn elephc_openssl_pkey_sign(
    pem: *const u8,
    pem_len: i64,
    data: *const u8,
    len: i64,
    digest: *const c_char,
    padding: i64,
) -> i64 {
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(pem), Some(data), Some(digest)) =
            (bytes(pem, pem_len), bytes(data, len), c_string(digest))
        else {
            return PKEY_ERR_INTERNAL;
        };
        match sign(pem, data, digest, padding) {
            Ok(signature) => publish(signature),
            Err(error) => status(error),
        }
    }))
    .unwrap_or(PKEY_ERR_INTERNAL)
}

/// Verifies a signature with a normalized public key PEM: 1 valid, 0 invalid, -1 when
/// libcrypto fails, or a `PKEY_ERR_*` status.
///
/// # Safety
///
/// `pem`, `data` and `signature` must point at their lengths' readable bytes, and
/// `digest` at a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn elephc_openssl_pkey_verify(
    pem: *const u8,
    pem_len: i64,
    data: *const u8,
    len: i64,
    signature: *const u8,
    signature_len: i64,
    digest: *const c_char,
    padding: i64,
) -> i64 {
    catch_unwind(AssertUnwindSafe(|| {
        let (Some(pem), Some(data), Some(signature), Some(digest)) = (
            bytes(pem, pem_len),
            bytes(data, len),
            bytes(signature, signature_len),
            c_string(digest),
        ) else {
            return PKEY_ERR_INTERNAL;
        };
        verify(pem, data, signature, digest, padding).unwrap_or_else(status)
    }))
    .unwrap_or(PKEY_ERR_INTERNAL)
}

/// Stores the `openssl_pkey_get_details()` packet of a normalized key PEM; returns its
/// length, or 0 when the key no longer parses.
///
/// # Safety
///
/// `pem` must point at `pem_len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn elephc_openssl_pkey_details(pem: *const u8, pem_len: i64, private: i64) -> i64 {
    catch_unwind(AssertUnwindSafe(|| {
        bytes(pem, pem_len).and_then(|pem| details(pem, private != 0)).map(publish).unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// Returns the address of the last stored result, valid until this thread's next call.
#[no_mangle]
pub extern "C" fn elephc_openssl_pkey_result() -> *const u8 {
    RESULT.with(|cell| cell.borrow().as_ptr())
}
