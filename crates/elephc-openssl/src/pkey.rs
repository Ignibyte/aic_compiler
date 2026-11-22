//! Purpose:
//! Ports PHP 8.5's asymmetric key path over libcrypto: PEM key loading, `openssl_sign`,
//! `openssl_verify` and the fields of `openssl_pkey_get_details`.
//!
//! Called from:
//! - `crate::pkey_abi` for the `elephc_openssl_pkey_*` C ABI.
//!
//! Key details:
//! - A loaded key is handed back as normalized PEM (SubjectPublicKeyInfo for a public
//!   key, unencrypted PKCS#8 for a private one). The PHP object keeps that text and every
//!   operation parses it again, so the bridge holds no key between calls.
//! - Public keys load from a certificate first and a `PUBLIC KEY` block second, private
//!   keys from any PEM private key block, decrypted with the passphrase. These are the
//!   forms `php_openssl_pkey_from_zval` reads from a string.
//! - The passphrase goes through a callback that never prompts: an encrypted key with a
//!   wrong or empty passphrase fails instead of reading the terminal.
//! - Digest names resolve through `EVP_get_digestbyname`, as php-src does, so aliases
//!   such as `RSA-SHA256` and any letter case work.
//! - Sign and verify check the digest before the padding, and a padding is accepted for
//!   RSA keys only: PKCS#1 v1.5 (1) or PSS with a digest-length salt (6).

use std::ffi::CString;
use std::os::raw::{c_char, c_int, c_void};

use crate::pkey_ffi::{
    BIO_ctrl, BIO_free, BIO_new, BIO_new_mem_buf, BIO_s_mem, BN_bn2bin, BN_free, BN_num_bits,
    ERR_clear_error, EVP_DigestSign, EVP_DigestSignInit, EVP_DigestVerify,
    EVP_DigestVerifyInit, EVP_MD_CTX_free, EVP_MD_CTX_new, EVP_PKEY_CTX_set_rsa_padding,
    EVP_PKEY_CTX_set_rsa_pss_saltlen, EVP_PKEY_free, EVP_PKEY_get_base_id, EVP_PKEY_get_bits,
    EVP_PKEY_get_bn_param, EVP_PKEY_get_utf8_string_param, EVP_get_digestbyname, OBJ_nid2obj,
    OBJ_obj2txt, OBJ_txt2nid, PEM_read_bio_PUBKEY, PEM_read_bio_PrivateKey, PEM_read_bio_X509,
    PEM_write_bio_PUBKEY, PEM_write_bio_PrivateKey, X509_free, X509_get_pubkey, BIGNUM, BIO,
    BIO_CTRL_INFO, EVP_MD, EVP_MD_CTX, EVP_PKEY, EVP_PKEY_CTX, EVP_PKEY_DH, EVP_PKEY_DSA,
    EVP_PKEY_EC, EVP_PKEY_ED25519, EVP_PKEY_ED448, EVP_PKEY_RSA, EVP_PKEY_X25519,
    EVP_PKEY_X448, RSA_PKCS1_PADDING, RSA_PKCS1_PSS_PADDING, RSA_PSS_SALTLEN_DIGEST,
};

/// Why a sign or verify call was refused before libcrypto ran it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PkeyError {
    /// The PEM text no longer parses as a key of the expected kind.
    BadKey,
    /// The digest name is unknown to libcrypto.
    UnknownDigest,
    /// A padding was given for a key that is not RSA.
    PaddingNeedsRsa,
    /// The padding is neither PKCS#1 v1.5 nor PSS.
    UnknownPadding,
    /// libcrypto refused the operation.
    Failed,
}

/// Owns one memory BIO.
struct Bio(*mut BIO);

impl Bio {
    /// Wraps caller bytes in a read-only memory BIO.
    fn reading(bytes: &[u8]) -> Option<Self> {
        let len = c_int::try_from(bytes.len()).ok()?;
        let bio = unsafe { BIO_new_mem_buf(bytes.as_ptr().cast::<c_void>(), len) };
        (!bio.is_null()).then_some(Self(bio))
    }

    /// Creates an empty, writable memory BIO.
    fn writing() -> Option<Self> {
        let bio = unsafe { BIO_new(BIO_s_mem()) };
        (!bio.is_null()).then_some(Self(bio))
    }

    /// Copies out everything written so far.
    fn contents(&self) -> Vec<u8> {
        let mut data: *mut c_char = std::ptr::null_mut();
        let len = unsafe {
            BIO_ctrl(self.0, BIO_CTRL_INFO, 0, (&mut data as *mut *mut c_char).cast::<c_void>())
        };
        if len <= 0 || data.is_null() {
            return Vec::new();
        }
        unsafe { std::slice::from_raw_parts(data.cast::<u8>(), len as usize) }.to_vec()
    }
}

impl Drop for Bio {
    fn drop(&mut self) {
        unsafe { BIO_free(self.0) };
    }
}

/// Owns one key reference.
struct Key(*mut EVP_PKEY);

impl Key {
    /// Takes ownership of a key libcrypto returned, or `None` for null.
    fn from_raw(key: *mut EVP_PKEY) -> Option<Self> {
        (!key.is_null()).then_some(Self(key))
    }

    /// The key's base algorithm id.
    fn base_id(&self) -> c_int {
        unsafe { EVP_PKEY_get_base_id(self.0) }
    }

    /// Writes the key's public half as a `PUBLIC KEY` PEM block.
    fn public_pem(&self) -> Option<Vec<u8>> {
        let bio = Bio::writing()?;
        (unsafe { PEM_write_bio_PUBKEY(bio.0, self.0) } == 1).then(|| bio.contents())
    }

    /// Writes the key as an unencrypted PKCS#8 `PRIVATE KEY` PEM block.
    fn private_pem(&self) -> Option<Vec<u8>> {
        let bio = Bio::writing()?;
        let written = unsafe {
            PEM_write_bio_PrivateKey(
                bio.0,
                self.0,
                std::ptr::null(),
                std::ptr::null(),
                0,
                None,
                std::ptr::null_mut(),
            )
        };
        (written == 1).then(|| bio.contents())
    }

    /// Reads one big-number parameter as big-endian bytes, `None` when the key lacks it.
    fn bn_param(&self, name: &str) -> Option<Vec<u8>> {
        let name = CString::new(name).ok()?;
        let mut bn: *mut BIGNUM = std::ptr::null_mut();
        if unsafe { EVP_PKEY_get_bn_param(self.0, name.as_ptr(), &mut bn) } != 1 || bn.is_null() {
            return None;
        }
        let len = (unsafe { BN_num_bits(bn) } as usize).div_ceil(8);
        let mut out = vec![0u8; len];
        let written = unsafe { BN_bn2bin(bn, out.as_mut_ptr()) };
        unsafe { BN_free(bn) };
        out.truncate(written.max(0) as usize);
        Some(out)
    }

    /// Reads one UTF-8 string parameter.
    fn string_param(&self, name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let mut buf = [0 as c_char; 128];
        let mut len = 0usize;
        let found = unsafe {
            EVP_PKEY_get_utf8_string_param(self.0, name.as_ptr(), buf.as_mut_ptr(), buf.len(), &mut len)
        };
        if found != 1 {
            return None;
        }
        let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), len) };
        String::from_utf8(bytes.to_vec()).ok()
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        unsafe { EVP_PKEY_free(self.0) };
    }
}

/// Owns one digest operation context.
struct DigestContext(*mut EVP_MD_CTX);

impl DigestContext {
    /// Creates a context, `None` when libcrypto is out of memory.
    fn new() -> Option<Self> {
        let ctx = unsafe { EVP_MD_CTX_new() };
        (!ctx.is_null()).then_some(Self(ctx))
    }
}

impl Drop for DigestContext {
    fn drop(&mut self) {
        unsafe { EVP_MD_CTX_free(self.0) };
    }
}

/// The passphrase a `pem_password_cb` hands to libcrypto.
struct Passphrase<'a>(&'a [u8]);

/// Copies the passphrase into libcrypto's buffer and never prompts. An empty passphrase
/// returns 0, which libcrypto reads as "no passphrase", so an encrypted key fails to load.
extern "C" fn passphrase_callback(
    buf: *mut c_char,
    size: c_int,
    _rwflag: c_int,
    userdata: *mut c_void,
) -> c_int {
    if buf.is_null() || userdata.is_null() || size <= 0 {
        return 0;
    }
    let passphrase = unsafe { &*userdata.cast::<Passphrase<'_>>() };
    let len = passphrase.0.len().min(size as usize);
    unsafe { std::ptr::copy_nonoverlapping(passphrase.0.as_ptr(), buf.cast::<u8>(), len) };
    len as c_int
}

/// Parses a private key PEM block, decrypting it with `passphrase`.
fn read_private(pem: &[u8], passphrase: &[u8]) -> Option<Key> {
    crate::catalog::ensure_default_provider();
    let bio = Bio::reading(pem)?;
    let mut userdata = Passphrase(passphrase);
    let key = unsafe {
        PEM_read_bio_PrivateKey(
            bio.0,
            std::ptr::null_mut(),
            Some(passphrase_callback),
            (&mut userdata as *mut Passphrase<'_>).cast::<c_void>(),
        )
    };
    let key = Key::from_raw(key);
    unsafe { ERR_clear_error() };
    key
}

/// Parses a public key from a certificate PEM block or a `PUBLIC KEY` PEM block.
fn read_public(pem: &[u8]) -> Option<Key> {
    crate::catalog::ensure_default_provider();
    let from_certificate = Bio::reading(pem).and_then(|bio| {
        let cert = unsafe {
            PEM_read_bio_X509(bio.0, std::ptr::null_mut(), None, std::ptr::null_mut())
        };
        if cert.is_null() {
            return None;
        }
        let key = unsafe { X509_get_pubkey(cert) };
        unsafe { X509_free(cert) };
        Key::from_raw(key)
    });
    let key = from_certificate.or_else(|| {
        let bio = Bio::reading(pem)?;
        let key = unsafe {
            PEM_read_bio_PUBKEY(bio.0, std::ptr::null_mut(), None, std::ptr::null_mut())
        };
        Key::from_raw(key)
    });
    unsafe { ERR_clear_error() };
    key
}

/// Loads a public key and returns it as normalized `PUBLIC KEY` PEM.
pub(crate) fn load_public(pem: &[u8]) -> Option<Vec<u8>> {
    read_public(pem)?.public_pem()
}

/// Loads a private key, decrypting it with `passphrase`, and returns it as unencrypted
/// PKCS#8 PEM.
pub(crate) fn load_private(pem: &[u8], passphrase: &[u8]) -> Option<Vec<u8>> {
    read_private(pem, passphrase)?.private_pem()
}

/// Resolves a digest name, or reports PHP's "Unknown digest algorithm".
fn digest(name: &[u8]) -> Result<*const EVP_MD, PkeyError> {
    let name = CString::new(name).map_err(|_| PkeyError::UnknownDigest)?;
    let md = unsafe { EVP_get_digestbyname(name.as_ptr()) };
    if md.is_null() {
        Err(PkeyError::UnknownDigest)
    } else {
        Ok(md)
    }
}

/// Checks a padding against the key, as php-src does before it signs or verifies.
fn check_padding(key: &Key, padding: i64) -> Result<(), PkeyError> {
    if padding == 0 {
        return Ok(());
    }
    if key.base_id() != EVP_PKEY_RSA {
        return Err(PkeyError::PaddingNeedsRsa);
    }
    if padding == i64::from(RSA_PKCS1_PADDING) || padding == i64::from(RSA_PKCS1_PSS_PADDING) {
        Ok(())
    } else {
        Err(PkeyError::UnknownPadding)
    }
}

/// Applies an accepted padding to the operation's key context.
fn apply_padding(pctx: *mut EVP_PKEY_CTX, padding: i64) -> Result<(), PkeyError> {
    if padding == 0 || pctx.is_null() {
        return Ok(());
    }
    let padding = padding as c_int;
    if unsafe { EVP_PKEY_CTX_set_rsa_padding(pctx, padding) } <= 0 {
        return Err(PkeyError::Failed);
    }
    if padding == RSA_PKCS1_PSS_PADDING
        && unsafe { EVP_PKEY_CTX_set_rsa_pss_saltlen(pctx, RSA_PSS_SALTLEN_DIGEST) } <= 0
    {
        return Err(PkeyError::Failed);
    }
    Ok(())
}

/// Signs `data` with a normalized private key PEM, as `openssl_sign()` does.
pub(crate) fn sign(
    pem: &[u8],
    data: &[u8],
    digest_name: &[u8],
    padding: i64,
) -> Result<Vec<u8>, PkeyError> {
    let key = read_private(pem, b"").ok_or(PkeyError::BadKey)?;
    let md = digest(digest_name)?;
    check_padding(&key, padding)?;
    let ctx = DigestContext::new().ok_or(PkeyError::Failed)?;
    let mut pctx: *mut EVP_PKEY_CTX = std::ptr::null_mut();
    let result = (|| {
        if unsafe { EVP_DigestSignInit(ctx.0, &mut pctx, md, std::ptr::null_mut(), key.0) } != 1 {
            return Err(PkeyError::Failed);
        }
        apply_padding(pctx, padding)?;
        let mut len = 0usize;
        if unsafe { EVP_DigestSign(ctx.0, std::ptr::null_mut(), &mut len, data.as_ptr(), data.len()) }
            != 1
        {
            return Err(PkeyError::Failed);
        }
        let mut signature = vec![0u8; len];
        if unsafe {
            EVP_DigestSign(ctx.0, signature.as_mut_ptr(), &mut len, data.as_ptr(), data.len())
        } != 1
        {
            return Err(PkeyError::Failed);
        }
        signature.truncate(len);
        Ok(signature)
    })();
    unsafe { ERR_clear_error() };
    result
}

/// Verifies `signature` over `data` with a normalized public key PEM, as
/// `openssl_verify()` does: 1 valid, 0 invalid, negative when libcrypto fails.
pub(crate) fn verify(
    pem: &[u8],
    data: &[u8],
    signature: &[u8],
    digest_name: &[u8],
    padding: i64,
) -> Result<i64, PkeyError> {
    let key = read_public(pem).ok_or(PkeyError::BadKey)?;
    let md = digest(digest_name)?;
    check_padding(&key, padding)?;
    let ctx = DigestContext::new().ok_or(PkeyError::Failed)?;
    let mut pctx: *mut EVP_PKEY_CTX = std::ptr::null_mut();
    let result = (|| {
        if unsafe { EVP_DigestVerifyInit(ctx.0, &mut pctx, md, std::ptr::null_mut(), key.0) } != 1
        {
            return Ok(-1);
        }
        if apply_padding(pctx, padding).is_err() {
            return Ok(-1);
        }
        let verdict = unsafe {
            EVP_DigestVerify(ctx.0, signature.as_ptr(), signature.len(), data.as_ptr(), data.len())
        };
        Ok(i64::from(verdict.clamp(-1, 1)))
    })();
    unsafe { ERR_clear_error() };
    result
}

/// PHP's `OPENSSL_KEYTYPE_*` value for a libcrypto base id, -1 for any other type.
fn php_key_type(base_id: c_int) -> i64 {
    match base_id {
        EVP_PKEY_RSA => 0,
        EVP_PKEY_DSA => 1,
        EVP_PKEY_DH => 2,
        EVP_PKEY_EC => 3,
        EVP_PKEY_X25519 => 4,
        EVP_PKEY_ED25519 => 5,
        EVP_PKEY_X448 => 6,
        EVP_PKEY_ED448 => 7,
        _ => -1,
    }
}

/// Appends one `name=hex` line of the details packet.
fn push_hex(out: &mut Vec<u8>, name: &str, value: &[u8]) {
    out.extend_from_slice(name.as_bytes());
    out.push(b'=');
    for byte in value {
        out.extend_from_slice(format!("{byte:02x}").as_bytes());
    }
    out.push(b'\n');
}

/// The dotted OID of a named curve.
fn curve_oid(curve: &str) -> Option<String> {
    let name = CString::new(curve).ok()?;
    let nid = unsafe { OBJ_txt2nid(name.as_ptr()) };
    if nid == 0 {
        return None;
    }
    let mut buf = [0 as c_char; 128];
    let len = unsafe { OBJ_obj2txt(buf.as_mut_ptr(), buf.len() as c_int, OBJ_nid2obj(nid), 1) };
    if len <= 0 || len as usize >= buf.len() {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), len as usize) };
    String::from_utf8(bytes.to_vec()).ok()
}

/// Builds the `openssl_pkey_get_details()` packet for a normalized key PEM.
///
/// One line per field, in PHP's key order: `bits` and `type` in decimal, every other
/// field as `name=hex`, with `rsa.` or `ec.` naming the nested array. Missing private
/// fields are left out, as php-src leaves them out of a public key's array.
pub(crate) fn details(pem: &[u8], private: bool) -> Option<Vec<u8>> {
    let key = if private { read_private(pem, b"")? } else { read_public(pem)? };
    let base_id = key.base_id();
    let mut out = Vec::new();
    out.extend_from_slice(format!("bits={}\n", unsafe { EVP_PKEY_get_bits(key.0) }).as_bytes());
    push_hex(&mut out, "key", &key.public_pem()?);
    match base_id {
        EVP_PKEY_RSA => {
            let fields = [
                ("n", "n"),
                ("e", "e"),
                ("d", "d"),
                ("p", "rsa-factor1"),
                ("q", "rsa-factor2"),
                ("dmp1", "rsa-exponent1"),
                ("dmq1", "rsa-exponent2"),
                ("iqmp", "rsa-coefficient1"),
            ];
            for (php_name, param) in fields {
                if let Some(value) = key.bn_param(param) {
                    push_hex(&mut out, &format!("rsa.{php_name}"), &value);
                }
            }
        }
        EVP_PKEY_EC => {
            if let Some(curve) = key.string_param("group") {
                push_hex(&mut out, "ec.curve_name", curve.as_bytes());
                if let Some(oid) = curve_oid(&curve) {
                    push_hex(&mut out, "ec.curve_oid", oid.as_bytes());
                }
            }
            for (php_name, param) in [("x", "qx"), ("y", "qy"), ("d", "priv")] {
                if let Some(value) = key.bn_param(param) {
                    push_hex(&mut out, &format!("ec.{php_name}"), &value);
                }
            }
        }
        _ => {}
    }
    out.extend_from_slice(format!("type={}\n", php_key_type(base_id)).as_bytes());
    unsafe { ERR_clear_error() };
    Some(out)
}
