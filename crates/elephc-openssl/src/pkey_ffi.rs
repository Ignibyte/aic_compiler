//! Purpose:
//! Declares the libcrypto key, PEM, BIO, digest-signature and parameter C ABI the
//! asymmetric key bridge uses.
//!
//! Called from:
//! - `crate::pkey`.
//!
//! Key details:
//! - Opaque handles only, like `crate::ffi`; no bindgen and no `-sys` crate.
//! - `BIO_get_mem_data` is a macro over `BIO_ctrl`, so its control code is declared here.
//! - Every pointer returned by a `*_new`, `*_read_*` or `*_get_pubkey` call is a new
//!   reference the caller frees.

#![allow(non_camel_case_types)]

use std::os::raw::{c_char, c_int, c_long, c_uchar, c_void};

/// Opaque OpenSSL I/O stream.
#[repr(C)]
pub(crate) struct BIO {
    _private: [u8; 0],
}

/// Opaque `BIO_METHOD` table.
#[repr(C)]
pub(crate) struct BIO_METHOD {
    _private: [u8; 0],
}

/// Opaque asymmetric key.
#[repr(C)]
pub(crate) struct EVP_PKEY {
    _private: [u8; 0],
}

/// Opaque per-key operation context.
#[repr(C)]
pub(crate) struct EVP_PKEY_CTX {
    _private: [u8; 0],
}

/// Opaque message digest implementation.
#[repr(C)]
pub(crate) struct EVP_MD {
    _private: [u8; 0],
}

/// Opaque digest operation context.
#[repr(C)]
pub(crate) struct EVP_MD_CTX {
    _private: [u8; 0],
}

/// Opaque X.509 certificate.
#[repr(C)]
pub(crate) struct X509 {
    _private: [u8; 0],
}

/// Opaque big number.
#[repr(C)]
pub(crate) struct BIGNUM {
    _private: [u8; 0],
}

/// Opaque ASN.1 object identifier.
#[repr(C)]
pub(crate) struct ASN1_OBJECT {
    _private: [u8; 0],
}

/// The `pem_password_cb` callback type.
pub(crate) type PemPasswordCb =
    extern "C" fn(buf: *mut c_char, size: c_int, rwflag: c_int, userdata: *mut c_void) -> c_int;

/// `BIO_CTRL_INFO`, the control `BIO_get_mem_data` expands to.
pub(crate) const BIO_CTRL_INFO: c_int = 3;
/// `EVP_PKEY_RSA`.
pub(crate) const EVP_PKEY_RSA: c_int = 6;
/// `EVP_PKEY_DSA`.
pub(crate) const EVP_PKEY_DSA: c_int = 116;
/// `EVP_PKEY_DH`.
pub(crate) const EVP_PKEY_DH: c_int = 28;
/// `EVP_PKEY_EC`.
pub(crate) const EVP_PKEY_EC: c_int = 408;
/// `EVP_PKEY_X25519`.
pub(crate) const EVP_PKEY_X25519: c_int = 1034;
/// `EVP_PKEY_ED25519`.
pub(crate) const EVP_PKEY_ED25519: c_int = 1087;
/// `EVP_PKEY_X448`.
pub(crate) const EVP_PKEY_X448: c_int = 1035;
/// `EVP_PKEY_ED448`.
pub(crate) const EVP_PKEY_ED448: c_int = 1088;
/// `RSA_PKCS1_PADDING`.
pub(crate) const RSA_PKCS1_PADDING: c_int = 1;
/// `RSA_PKCS1_PSS_PADDING`.
pub(crate) const RSA_PKCS1_PSS_PADDING: c_int = 6;
/// `RSA_PSS_SALTLEN_DIGEST`: the salt is as long as the digest.
pub(crate) const RSA_PSS_SALTLEN_DIGEST: c_int = -1;

extern "C" {
    /// Returns the memory BIO method table.
    pub(crate) fn BIO_s_mem() -> *const BIO_METHOD;
    /// Creates a BIO of the given method.
    pub(crate) fn BIO_new(method: *const BIO_METHOD) -> *mut BIO;
    /// Creates a read-only memory BIO over caller-owned bytes.
    pub(crate) fn BIO_new_mem_buf(buf: *const c_void, len: c_int) -> *mut BIO;
    /// Frees a BIO.
    pub(crate) fn BIO_free(bio: *mut BIO) -> c_int;
    /// Sends a control request; `BIO_CTRL_INFO` reads a memory BIO's buffer.
    pub(crate) fn BIO_ctrl(bio: *mut BIO, cmd: c_int, larg: c_long, parg: *mut c_void) -> c_long;

    /// Reads a SubjectPublicKeyInfo PEM block.
    pub(crate) fn PEM_read_bio_PUBKEY(
        bio: *mut BIO,
        out: *mut *mut EVP_PKEY,
        cb: Option<PemPasswordCb>,
        userdata: *mut c_void,
    ) -> *mut EVP_PKEY;
    /// Reads a private key PEM block, decrypting it through `cb`.
    pub(crate) fn PEM_read_bio_PrivateKey(
        bio: *mut BIO,
        out: *mut *mut EVP_PKEY,
        cb: Option<PemPasswordCb>,
        userdata: *mut c_void,
    ) -> *mut EVP_PKEY;
    /// Reads an X.509 certificate PEM block.
    pub(crate) fn PEM_read_bio_X509(
        bio: *mut BIO,
        out: *mut *mut X509,
        cb: Option<PemPasswordCb>,
        userdata: *mut c_void,
    ) -> *mut X509;
    /// Writes a SubjectPublicKeyInfo PEM block.
    pub(crate) fn PEM_write_bio_PUBKEY(bio: *mut BIO, key: *const EVP_PKEY) -> c_int;
    /// Writes an unencrypted PKCS#8 private key PEM block when `cipher` is null.
    pub(crate) fn PEM_write_bio_PrivateKey(
        bio: *mut BIO,
        key: *const EVP_PKEY,
        cipher: *const c_void,
        kstr: *const c_uchar,
        klen: c_int,
        cb: Option<PemPasswordCb>,
        userdata: *mut c_void,
    ) -> c_int;

    /// Returns a new reference to a certificate's public key.
    pub(crate) fn X509_get_pubkey(cert: *mut X509) -> *mut EVP_PKEY;
    /// Frees a certificate.
    pub(crate) fn X509_free(cert: *mut X509);

    /// Frees a key.
    pub(crate) fn EVP_PKEY_free(key: *mut EVP_PKEY);
    /// Returns the key's base algorithm id (`EVP_PKEY_RSA`, `EVP_PKEY_EC`, ...).
    pub(crate) fn EVP_PKEY_get_base_id(key: *const EVP_PKEY) -> c_int;
    /// Returns the key's size in bits.
    pub(crate) fn EVP_PKEY_get_bits(key: *const EVP_PKEY) -> c_int;
    /// Reads one big-number parameter into a new `BIGNUM`.
    pub(crate) fn EVP_PKEY_get_bn_param(
        key: *const EVP_PKEY,
        name: *const c_char,
        out: *mut *mut BIGNUM,
    ) -> c_int;
    /// Reads one UTF-8 string parameter into a caller buffer.
    pub(crate) fn EVP_PKEY_get_utf8_string_param(
        key: *const EVP_PKEY,
        name: *const c_char,
        out: *mut c_char,
        max: usize,
        out_len: *mut usize,
    ) -> c_int;
    /// Selects the RSA padding of a signature context.
    pub(crate) fn EVP_PKEY_CTX_set_rsa_padding(ctx: *mut EVP_PKEY_CTX, padding: c_int) -> c_int;
    /// Selects the RSA-PSS salt length of a signature context.
    pub(crate) fn EVP_PKEY_CTX_set_rsa_pss_saltlen(ctx: *mut EVP_PKEY_CTX, len: c_int) -> c_int;

    /// Looks a digest up by name or alias, as PHP's `openssl_sign()` does.
    pub(crate) fn EVP_get_digestbyname(name: *const c_char) -> *const EVP_MD;
    /// Creates a digest operation context.
    pub(crate) fn EVP_MD_CTX_new() -> *mut EVP_MD_CTX;
    /// Frees a digest operation context.
    pub(crate) fn EVP_MD_CTX_free(ctx: *mut EVP_MD_CTX);
    /// Starts a digest-and-sign operation.
    pub(crate) fn EVP_DigestSignInit(
        ctx: *mut EVP_MD_CTX,
        pctx: *mut *mut EVP_PKEY_CTX,
        md: *const EVP_MD,
        engine: *mut c_void,
        key: *mut EVP_PKEY,
    ) -> c_int;
    /// Signs `tbs` in one call; a null `sig` asks for the maximum size.
    pub(crate) fn EVP_DigestSign(
        ctx: *mut EVP_MD_CTX,
        sig: *mut c_uchar,
        siglen: *mut usize,
        tbs: *const c_uchar,
        tbslen: usize,
    ) -> c_int;
    /// Starts a digest-and-verify operation.
    pub(crate) fn EVP_DigestVerifyInit(
        ctx: *mut EVP_MD_CTX,
        pctx: *mut *mut EVP_PKEY_CTX,
        md: *const EVP_MD,
        engine: *mut c_void,
        key: *mut EVP_PKEY,
    ) -> c_int;
    /// Verifies `sig` over `tbs` in one call: 1 valid, 0 invalid, negative on error.
    pub(crate) fn EVP_DigestVerify(
        ctx: *mut EVP_MD_CTX,
        sig: *const c_uchar,
        siglen: usize,
        tbs: *const c_uchar,
        tbslen: usize,
    ) -> c_int;

    /// Returns the number of significant bits.
    pub(crate) fn BN_num_bits(bn: *const BIGNUM) -> c_int;
    /// Writes the big-endian magnitude; returns its length.
    pub(crate) fn BN_bn2bin(bn: *const BIGNUM, out: *mut c_uchar) -> c_int;
    /// Frees a big number.
    pub(crate) fn BN_free(bn: *mut BIGNUM);

    /// Maps a short or long object name to its NID.
    pub(crate) fn OBJ_txt2nid(name: *const c_char) -> c_int;
    /// Returns the static object for a NID.
    pub(crate) fn OBJ_nid2obj(nid: c_int) -> *const ASN1_OBJECT;
    /// Renders an object; `no_name = 1` gives the dotted OID.
    pub(crate) fn OBJ_obj2txt(
        buf: *mut c_char,
        len: c_int,
        obj: *const ASN1_OBJECT,
        no_name: c_int,
    ) -> c_int;

    /// Clears this thread's libcrypto error queue.
    pub(crate) fn ERR_clear_error();
}
