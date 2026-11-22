//! Purpose:
//! Integration tests for the asymmetric key ABI: PEM loading, `openssl_sign`,
//! `openssl_verify` and the `openssl_pkey_get_details` packet.
//!
//! Called from:
//! - `cargo test -p elephc-openssl` through Rust's test harness.
//!
//! Key details:
//! - Native tests run only when `ELEPHC_OPENSSL_LIB_DIR` points at a pinned
//!   OpenSSL 3.6.4 `lib/`, like the cipher tests.
//! - `pkey/` holds throwaway test keys. The RSA signatures and key fields below were
//!   produced by PHP 8.5's `openssl_sign()` and `openssl_pkey_get_details()` over
//!   those keys; PKCS#1 v1.5 signatures are deterministic, ECDSA ones are not.

#![cfg(elephc_openssl_native)]

use std::ffi::CString;

use elephc_openssl::*;

/// The message every golden signature covers.
const MESSAGE: &[u8] = b"The quick brown fox jumps over the lazy dog";
/// PHP's `openssl_sign(MESSAGE, $s, rsa.pem, OPENSSL_ALGO_SHA256)`.
const RSA_SHA256: &str = "894c4e706dba2b23012e570eb5098002435d74ef65095e1ce12c4815b6d26f6220c28f8defebb07f84c3ec064c917c3dd494e7688b611babf3a1506b821d494ac8823d077cc271713f9729abbabe078fb803a5de07f109a59570c973b65f5fd5f7c8f394a83a8c718b229e27b7c388819ed55505f6b9f8bbdf1fafa0e0b363ea";
/// PHP's `openssl_sign(MESSAGE, $s, rsa.pem)`, whose default digest is SHA-1.
const RSA_SHA1: &str = "5427c6a2cf256a5aaca547c3dd27e3919a846b0854d10e951cc6a32fabd1f957ae0e31e151119d531ed587fe128ceaf8c334b9fee3ace1b97fbf5b239893f0d70617d6e837433032284c2ed92977cea29817de89f203a979f77b511d285533980a25e6a382614028e6d6e6c4880ef8c423ec90372e107768c3ab026d3fabe3fd";
/// PHP's `openssl_pkey_get_details(rsa.pem)['rsa']['n']`.
const RSA_N: &str = "b3e4763648426311065f048966051ec2b93ad5d82f20acdcf017d76aea9bef3848b84d0dbc9d92a16e622275f1376295fdafb165ccfa366f84293faa22b5e6e731df3cd35d84e2fcd87c4ce3cdb0e84ceae3e80455172f174c3e0d9f5c0297461fabb42419c81899eff16696b93eec1afc5be402623bfe508ff28d4cfe35174d";
/// PHP's `openssl_pkey_get_details(ec_pub.pem)['ec']['x']` and `['y']`.
const EC_X: &str = "722663094b4c40b5ed1e2d10201a5646aeb806776c76d41724466d067e007103";
const EC_Y: &str = "1edf6c25876ba5b1d39895956f295f3494275836b5061317891be7ae68c4ae68";

/// Lowercase hex of `bytes`.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Bytes from lowercase hex.
fn unhex(text: &str) -> Vec<u8> {
    (0..text.len()).step_by(2).map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap()).collect()
}

/// Copies the last stored result.
fn result(len: i64) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(elephc_openssl_pkey_result(), len as usize) }.to_vec()
}

/// Loads a key through the ABI; `None` when the bridge refuses it.
fn load(private: bool, pem: &str, passphrase: &str) -> Option<Vec<u8>> {
    let len = unsafe {
        elephc_openssl_pkey_load(
            i64::from(private),
            pem.as_ptr(),
            pem.len() as i64,
            passphrase.as_ptr(),
            passphrase.len() as i64,
        )
    };
    (len > 0).then(|| result(len))
}

/// Signs through the ABI; the signature or the negative status.
fn sign(pem: &[u8], digest: &str, padding: i64) -> Result<Vec<u8>, i64> {
    let digest = CString::new(digest).unwrap();
    let len = unsafe {
        elephc_openssl_pkey_sign(
            pem.as_ptr(),
            pem.len() as i64,
            MESSAGE.as_ptr(),
            MESSAGE.len() as i64,
            digest.as_ptr(),
            padding,
        )
    };
    if len > 0 { Ok(result(len)) } else { Err(len) }
}

/// Verifies through the ABI.
fn verify(pem: &[u8], message: &[u8], signature: &[u8], digest: &str, padding: i64) -> i64 {
    let digest = CString::new(digest).unwrap();
    unsafe {
        elephc_openssl_pkey_verify(
            pem.as_ptr(),
            pem.len() as i64,
            message.as_ptr(),
            message.len() as i64,
            signature.as_ptr(),
            signature.len() as i64,
            digest.as_ptr(),
            padding,
        )
    }
}

/// The details packet as `(name, value)` lines.
fn details(pem: &[u8], private: bool) -> Vec<(String, String)> {
    let len = unsafe { elephc_openssl_pkey_details(pem.as_ptr(), pem.len() as i64, i64::from(private)) };
    assert!(len > 0, "details packet");
    String::from_utf8(result(len))
        .unwrap()
        .lines()
        .map(|line| {
            let (name, value) = line.split_once('=').unwrap();
            (name.to_string(), value.to_string())
        })
        .collect()
}

/// Public keys load from a PUBLIC KEY block or a certificate, and normalize to the same PEM.
#[test]
fn public_keys_load_from_spki_and_certificates() {
    let spki = load(false, include_str!("pkey/rsa_pub.pem"), "").expect("spki");
    assert_eq!(spki, include_bytes!("pkey/rsa_pub.pem"));
    let from_cert = load(false, include_str!("pkey/rsa_cert.pem"), "").expect("certificate");
    assert_eq!(from_cert, spki);
    assert_eq!(load(false, include_str!("pkey/rsa.pem"), ""), None, "a private block is no public key");
    assert_eq!(load(false, "garbage", ""), None);
}

/// Private keys load plain or decrypted, normalize to PKCS#8, and never prompt.
#[test]
fn private_keys_load_with_the_right_passphrase_only() {
    let plain = load(true, include_str!("pkey/rsa.pem"), "").expect("plain");
    assert!(plain.starts_with(b"-----BEGIN PRIVATE KEY-----"));
    let decrypted = load(true, include_str!("pkey/rsa_encrypted.pem"), "secret").expect("decrypted");
    assert_eq!(decrypted, plain);
    assert_eq!(load(true, include_str!("pkey/rsa_encrypted.pem"), "wrong"), None);
    assert_eq!(load(true, include_str!("pkey/rsa_encrypted.pem"), ""), None);
    assert_eq!(load(true, include_str!("pkey/rsa_pub.pem"), ""), None);
}

/// RSA PKCS#1 v1.5 signatures match PHP byte for byte, and digest names resolve by alias.
#[test]
fn rsa_signatures_match_php() {
    let key = load(true, include_str!("pkey/rsa.pem"), "").unwrap();
    assert_eq!(hex(&sign(&key, "sha256", 0).unwrap()), RSA_SHA256);
    assert_eq!(hex(&sign(&key, "SHA256", 0).unwrap()), RSA_SHA256);
    assert_eq!(hex(&sign(&key, "RSA-SHA256", 1).unwrap()), RSA_SHA256);
    assert_eq!(hex(&sign(&key, "sha1", 0).unwrap()), RSA_SHA1);
}

/// Verify answers 1, 0 or -1 as PHP does, and PSS round-trips with a digest-length salt.
#[test]
fn verify_answers_like_php() {
    let public = load(false, include_str!("pkey/rsa_pub.pem"), "").unwrap();
    let golden = unhex(RSA_SHA256);
    assert_eq!(verify(&public, MESSAGE, &golden, "sha256", 0), 1);
    assert_eq!(verify(&public, b"tampered", &golden, "sha256", 0), 0);
    assert_eq!(verify(&public, MESSAGE, &golden, "sha1", 0), 0);
    let private = load(true, include_str!("pkey/rsa.pem"), "").unwrap();
    let pss = sign(&private, "sha256", 6).unwrap();
    assert_ne!(hex(&pss), RSA_SHA256);
    assert_eq!(verify(&public, MESSAGE, &pss, "sha256", 6), 1);
    assert_eq!(verify(&public, MESSAGE, &pss, "sha256", 0), 0);
}

/// ECDSA signs and verifies, and a malformed DER signature is -1, as in PHP.
#[test]
fn ecdsa_round_trips() {
    let private = load(true, include_str!("pkey/ec.pem"), "").unwrap();
    let public = load(false, include_str!("pkey/ec_pub.pem"), "").unwrap();
    let signature = sign(&private, "sha256", 0).unwrap();
    assert_eq!(signature[0], 0x30, "a DER SEQUENCE");
    assert_eq!(verify(&public, MESSAGE, &signature, "sha256", 0), 1);
    assert_eq!(verify(&public, b"tampered", &signature, "sha256", 0), 0);
    assert_eq!(verify(&public, MESSAGE, b"garbage", "sha256", 0), -1);
}

/// The refusals php-src reports with a warning come back as distinct statuses, digest first.
#[test]
fn refusals_have_their_own_statuses() {
    let rsa = load(true, include_str!("pkey/rsa.pem"), "").unwrap();
    let ec = load(true, include_str!("pkey/ec.pem"), "").unwrap();
    assert_eq!(sign(&rsa, "nope", 0), Err(PKEY_ERR_UNKNOWN_DIGEST));
    assert_eq!(sign(&rsa, "sha256", 3), Err(PKEY_ERR_UNKNOWN_PADDING));
    assert_eq!(sign(&ec, "sha256", 6), Err(PKEY_ERR_PADDING_NEEDS_RSA));
    assert_eq!(sign(&ec, "nope", 3), Err(PKEY_ERR_UNKNOWN_DIGEST));
    assert_eq!(sign(b"garbage", "sha256", 0), Err(PKEY_ERR_BAD_KEY));
    let public = load(false, include_str!("pkey/rsa_pub.pem"), "").unwrap();
    assert_eq!(verify(&public, MESSAGE, b"x", "nope", 0), PKEY_ERR_UNKNOWN_DIGEST);
    assert_eq!(verify(&public, MESSAGE, b"x", "sha256", 3), PKEY_ERR_UNKNOWN_PADDING);
}

/// The details packet carries PHP's fields, in PHP's key order.
#[test]
fn details_follow_php_fields_and_order() {
    let rsa = load(true, include_str!("pkey/rsa.pem"), "").unwrap();
    let packet = details(&rsa, true);
    let names: Vec<&str> = packet.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(
        names,
        ["bits", "key", "rsa.n", "rsa.e", "rsa.d", "rsa.p", "rsa.q", "rsa.dmp1", "rsa.dmq1", "rsa.iqmp", "type"]
    );
    assert_eq!(packet[0].1, "1024");
    assert_eq!(unhex(&packet[1].1), include_bytes!("pkey/rsa_pub.pem"));
    assert_eq!(packet[2].1, RSA_N);
    assert_eq!(packet[3].1, "010001");
    assert_eq!(packet[10].1, "0");
    let public = load(false, include_str!("pkey/rsa_pub.pem"), "").unwrap();
    assert_eq!(details(&public, false).len(), 5, "bits, key, n, e, type");

    let ec = load(false, include_str!("pkey/ec_pub.pem"), "").unwrap();
    let packet = details(&ec, false);
    let fields: Vec<(&str, String)> = packet.iter().map(|(name, value)| (name.as_str(), value.clone())).collect();
    assert_eq!(fields[0], ("bits", "256".to_string()));
    assert_eq!(fields[2], ("ec.curve_name", hex(b"prime256v1")));
    assert_eq!(fields[3], ("ec.curve_oid", hex(b"1.2.840.10045.3.1.7")));
    assert_eq!(fields[4], ("ec.x", EC_X.to_string()));
    assert_eq!(fields[5], ("ec.y", EC_Y.to_string()));
    assert_eq!(fields[6], ("type", "3".to_string()));
}
