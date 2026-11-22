//! Purpose:
//! End-to-end AOT tests for the asymmetric openssl key surface: key loading,
//! `openssl_sign`, `openssl_verify` and `openssl_pkey_get_details`.
//!
//! Called from:
//! - `cargo test --test codegen_tests_data openssl_keys` through the Rust test harness.
//!
//! Key details:
//! - The keys are the bridge crate's throwaway fixtures in
//!   `crates/elephc-openssl/tests/pkey/`, written into each program as nowdocs.
//! - Every expected output was printed by PHP 8.5.12 running the same program; RSA
//!   PKCS#1 v1.5 signatures are deterministic, so they are compared byte for byte.

use super::*;

/// The test keys as PHP nowdoc assignments, one variable per fixture file.
fn key_assignments() -> String {
    [
        ("rsa", include_str!("../../../crates/elephc-openssl/tests/pkey/rsa.pem")),
        ("rsa_pub", include_str!("../../../crates/elephc-openssl/tests/pkey/rsa_pub.pem")),
        ("rsa_cert", include_str!("../../../crates/elephc-openssl/tests/pkey/rsa_cert.pem")),
        ("rsa_encrypted", include_str!("../../../crates/elephc-openssl/tests/pkey/rsa_encrypted.pem")),
        ("ec", include_str!("../../../crates/elephc-openssl/tests/pkey/ec.pem")),
        ("ec_pub", include_str!("../../../crates/elephc-openssl/tests/pkey/ec_pub.pem")),
    ]
    .iter()
    .map(|(name, pem)| format!("${name} = <<<'PEM'\n{pem}PEM;\n"))
    .collect()
}

/// A program that declares the test keys, then runs `body`.
fn with_keys(body: &str) -> String {
    format!("<?php\n{}{}", key_assignments(), body)
}

/// Verifies RSA PKCS#1 v1.5 signatures byte for byte against PHP's, digest names and
/// aliases, certificate keys, the SHA-1 default, and PSS padding round trips.
#[test]
fn test_openssl_rsa_signatures_match_php_and_verify() {
    if skip_without_openssl_native("test_openssl_rsa_signatures_match_php_and_verify") {
        return;
    }
    let out = compile_and_run(&with_keys(
        r##"$message = "The quick brown fox jumps over the lazy dog";
$sha256 = $sha1 = $named = $pss = "";
$key = openssl_pkey_get_private($rsa);
var_dump(openssl_sign($message, $sha256, $key, OPENSSL_ALGO_SHA256));
echo bin2hex($sha256), "\n";
var_dump(openssl_sign($message, $sha1, $rsa));
echo bin2hex($sha1), "\n";
openssl_sign($message, $named, $key, "RSA-SHA256");
var_dump($named === $sha256);
$public = openssl_pkey_get_public($rsa_pub);
echo openssl_verify($message, $sha256, $public, OPENSSL_ALGO_SHA256), "|";
echo openssl_verify($message, $sha256, $rsa_cert, "SHA256"), "|";
echo openssl_verify("tampered", $sha256, $public, OPENSSL_ALGO_SHA256), "|";
echo openssl_verify($message, $sha1, $public), "|";
echo openssl_verify($message, $sha256, $public), "\n";
openssl_sign($message, $pss, $key, OPENSSL_ALGO_SHA256, OPENSSL_PKCS1_PSS_PADDING);
var_dump($pss !== $sha256, strlen($pss));
echo openssl_verify($message, $pss, $public, OPENSSL_ALGO_SHA256, OPENSSL_PKCS1_PSS_PADDING), "|";
echo openssl_verify($message, $pss, $public, OPENSSL_ALGO_SHA256), "\n";
"##,
    ));
    assert_eq!(
        out,
        r##"bool(true)
894c4e706dba2b23012e570eb5098002435d74ef65095e1ce12c4815b6d26f6220c28f8defebb07f84c3ec064c917c3dd494e7688b611babf3a1506b821d494ac8823d077cc271713f9729abbabe078fb803a5de07f109a59570c973b65f5fd5f7c8f394a83a8c718b229e27b7c388819ed55505f6b9f8bbdf1fafa0e0b363ea
bool(true)
5427c6a2cf256a5aaca547c3dd27e3919a846b0854d10e951cc6a32fabd1f957ae0e31e151119d531ed587fe128ceaf8c334b9fee3ace1b97fbf5b239893f0d70617d6e837433032284c2ed92977cea29817de89f203a979f77b511d285533980a25e6a382614028e6d6e6c4880ef8c423ec90372e107768c3ab026d3fabe3fd
bool(true)
1|1|0|1|0
bool(true)
int(128)
1|0
"##
    );
}

/// Verifies ECDSA P-256 sign and verify, including PHP's -1 for a malformed DER signature.
#[test]
fn test_openssl_ecdsa_signs_and_verifies_like_php() {
    if skip_without_openssl_native("test_openssl_ecdsa_signs_and_verifies_like_php") {
        return;
    }
    let out = compile_and_run(&with_keys(
        r##"$message = "The quick brown fox jumps over the lazy dog";
$private = openssl_pkey_get_private($ec);
$public = openssl_pkey_get_public($ec_pub);
$signature = "";
var_dump(openssl_sign($message, $signature, $private, OPENSSL_ALGO_SHA256));
echo bin2hex(substr($signature, 0, 1)), "|";
echo openssl_verify($message, $signature, $public, OPENSSL_ALGO_SHA256), "|";
echo openssl_verify("tampered", $signature, $public, OPENSSL_ALGO_SHA256), "|";
echo openssl_verify($message, "garbage", $public, OPENSSL_ALGO_SHA256), "\n";
"##,
    ));
    assert_eq!(
        out,
        r##"bool(true)
30|1|0|-1
"##
    );
}

/// Verifies key loading from PEM text, certificates, encrypted PEM with a passphrase,
/// `[key, passphrase]` arrays, `file://` paths and the aliases, and the empty var_dump.
#[test]
fn test_openssl_keys_load_from_every_supported_form() {
    if skip_without_openssl_native("test_openssl_keys_load_from_every_supported_form") {
        return;
    }
    let out = compile_and_run(&with_keys(
        r##"$private = openssl_pkey_get_private($rsa);
var_dump($private);
var_dump($private instanceof OpenSSLAsymmetricKey, get_class($private));
var_dump(openssl_pkey_get_private("garbage"));
var_dump(openssl_pkey_get_private($rsa_encrypted, "secret") instanceof OpenSSLAsymmetricKey);
var_dump(openssl_pkey_get_private($rsa_encrypted, "wrong"));
var_dump(openssl_pkey_get_private([$rsa_encrypted, "secret"]) instanceof OpenSSLAsymmetricKey);
var_dump(openssl_get_privatekey($rsa) instanceof OpenSSLAsymmetricKey);
var_dump(openssl_pkey_get_private($rsa_pub));
var_dump(openssl_pkey_get_public($rsa_cert) instanceof OpenSSLAsymmetricKey);
var_dump(openssl_get_publickey($ec_pub) instanceof OpenSSLAsymmetricKey);
var_dump(openssl_pkey_get_public($rsa));
$path = tempnam(sys_get_temp_dir(), "pkey");
file_put_contents($path, $rsa_pub);
var_dump(openssl_pkey_get_public("file://" . $path) instanceof OpenSSLAsymmetricKey);
unlink($path);
var_dump(openssl_pkey_get_public("file://" . $path));
$public = openssl_pkey_get_public($rsa_pub);
var_dump(openssl_pkey_get_public($public) === $public);
"##,
    ));
    assert_eq!(
        out,
        r##"object(OpenSSLAsymmetricKey)#1 (0) {
}
bool(true)
string(20) "OpenSSLAsymmetricKey"
bool(false)
bool(true)
bool(false)
bool(true)
bool(true)
bool(false)
bool(true)
bool(true)
bool(false)
bool(true)
bool(false)
bool(false)
"##
    );
}

/// Verifies PHP's warnings and results for a key of the wrong kind, an unusable key, an
/// unknown digest and a bad padding, and the array, clone and serialize refusals.
#[test]
fn test_openssl_key_refusals_warn_like_php() {
    if skip_without_openssl_native("test_openssl_key_refusals_warn_like_php") {
        return;
    }
    let out = compile_and_run(&with_keys(
        r##"set_error_handler(function (int $errno, string $errstr): bool {
    echo "[", $errstr, "]\n";
    return true;
});
$private = openssl_pkey_get_private($rsa);
$public = openssl_pkey_get_public($rsa_pub);
$ec_key = openssl_pkey_get_private($ec);
$signature = null;
var_dump(openssl_pkey_get_public($private));
var_dump(openssl_pkey_get_private($public));
var_dump(openssl_sign("m", $signature, $public));
var_dump(isset($signature));
var_dump(openssl_sign("m", $signature, "garbage"));
var_dump(openssl_sign("m", $signature, $private, "nope"));
var_dump(openssl_sign("m", $signature, $private, OPENSSL_ALGO_SHA256, 3));
var_dump(openssl_sign("m", $signature, $ec_key, OPENSSL_ALGO_SHA256, OPENSSL_PKCS1_PADDING));
var_dump(openssl_verify("m", "x", $private));
var_dump(openssl_verify("m", "x", "garbage"));
var_dump(openssl_verify("m", "x", $public, "nope"));
var_dump(openssl_verify("m", "x", $public, OPENSSL_ALGO_SHA256, 3));
try {
    openssl_pkey_get_public([$rsa_pub]);
} catch (ValueError $error) {
    echo get_class($error), ": ", $error->getMessage(), "\n";
}
try {
    $copy = clone $private;
} catch (Error $error) {
    echo get_class($error), ": ", $error->getMessage(), "\n";
}
try {
    echo serialize($private);
} catch (Exception $error) {
    echo get_class($error), ": ", $error->getMessage(), "\n";
}
"##,
    ));
    assert_eq!(
        out,
        r##"[openssl_pkey_get_public(): Don't know how to get public key from this private key]
bool(false)
[openssl_pkey_get_private(): Supplied key param is a public key]
bool(false)
[openssl_sign(): Supplied key param is a public key]
[openssl_sign(): Supplied key param cannot be coerced into a private key]
bool(false)
bool(false)
[openssl_sign(): Supplied key param cannot be coerced into a private key]
bool(false)
[openssl_sign(): Unknown digest algorithm]
bool(false)
[openssl_sign(): Unknown padding type]
bool(false)
[openssl_sign(): Padding parameter is only supported for RSA keys]
bool(false)
[openssl_verify(): Don't know how to get public key from this private key]
[openssl_verify(): Supplied key param cannot be coerced into a public key]
bool(false)
[openssl_verify(): Supplied key param cannot be coerced into a public key]
bool(false)
[openssl_verify(): Unknown digest algorithm]
bool(false)
[openssl_verify(): Unknown padding type]
int(-1)
ValueError: Key array must be of the form array(0 => key, 1 => phrase)
Error: Trying to clone an uncloneable object of class OpenSSLAsymmetricKey
Exception: Serialization of 'OpenSSLAsymmetricKey' is not allowed
"##
    );
}

/// Verifies `openssl_pkey_get_details()` keys, order, field sizes and values for RSA and EC.
#[test]
fn test_openssl_pkey_get_details_matches_php() {
    if skip_without_openssl_native("test_openssl_pkey_get_details_matches_php") {
        return;
    }
    let out = compile_and_run(&with_keys(
        r##"function describe(array|false $details): void
{
    foreach ($details as $name => $value) {
        if (is_array($value)) {
            echo $name, ":";
            foreach ($value as $field => $bytes) {
                echo " ", $field, "=", strlen($bytes);
            }
            echo "\n";
        } elseif (is_string($value)) {
            echo $name, ": ", strlen($value), " bytes\n";
        } else {
            echo $name, ": ", $value, "\n";
        }
    }
}
$private = openssl_pkey_get_details(openssl_pkey_get_private($rsa));
describe($private);
describe(openssl_pkey_get_details(openssl_pkey_get_public($rsa_cert)));
echo bin2hex($private["rsa"]["n"]), "\n";
echo bin2hex($private["rsa"]["e"]), "\n";
var_dump(rtrim($private["key"]) === $rsa_pub);
$ec_details = openssl_pkey_get_details(openssl_pkey_get_private($ec));
describe($ec_details);
echo $ec_details["ec"]["curve_name"], " ", $ec_details["ec"]["curve_oid"], " ", $ec_details["type"] === OPENSSL_KEYTYPE_EC ? "ec" : "?", "\n";
echo bin2hex(openssl_pkey_get_details(openssl_pkey_get_public($ec_pub))["ec"]["x"]), "\n";
"##,
    ));
    assert_eq!(
        out,
        r##"bits: 1024
key: 272 bytes
rsa: n=128 e=3 d=128 p=64 q=64 dmp1=64 dmq1=64 iqmp=64
type: 0
bits: 1024
key: 272 bytes
rsa: n=128 e=3
type: 0
b3e4763648426311065f048966051ec2b93ad5d82f20acdcf017d76aea9bef3848b84d0dbc9d92a16e622275f1376295fdafb165ccfa366f84293faa22b5e6e731df3cd35d84e2fcd87c4ce3cdb0e84ceae3e80455172f174c3e0d9f5c0297461fabb42419c81899eff16696b93eec1afc5be402623bfe508ff28d4cfe35174d
010001
bool(true)
bits: 256
key: 178 bytes
ec: curve_name=10 curve_oid=19 x=32 y=32 d=32
type: 3
prime256v1 1.2.840.10045.3.1.7 ec
722663094b4c40b5ed1e2d10201a5646aeb806776c76d41724466d067e007103
"##
    );
}

/// Verifies case-insensitive and namespaced calls and a namespaced class type hint.
#[test]
fn test_openssl_key_surface_resolves_from_a_namespace() {
    if skip_without_openssl_native("test_openssl_key_surface_resolves_from_a_namespace") {
        return;
    }
    let out = compile_and_run(&format!(
        "<?php\nnamespace App;\n{}{}",
        key_assignments(),
        r##"$signature = "";
$key = \OPENSSL_PKEY_GET_PRIVATE($rsa);
var_dump(Openssl_Sign("m", $signature, $key, "sha256"));
echo \openssl_verify("m", $signature, openssl_pkey_get_public($rsa_pub), "sha256"), "\n";
function check(\OpenSSLAsymmetricKey $key): string
{
    return get_class($key);
}
echo check($key), "\n";
"##,
    ));
    assert_eq!(out, r##"bool(true)
1
OpenSSLAsymmetricKey
"##);
}

/// Verifies repeated loads, signatures, verifications and details leave a clean heap.
#[test]
fn test_openssl_key_operations_leave_a_clean_heap() {
    if skip_without_openssl_native("test_openssl_key_operations_leave_a_clean_heap") {
        return;
    }
    let out = compile_and_run_with_heap_debug(&with_keys(
        r##"$signature = "";
for ($i = 0; $i < 3; $i++) {
    $private = openssl_pkey_get_private($rsa);
    $public = openssl_pkey_get_public($rsa_pub);
    openssl_sign("message " . $i, $signature, $private, OPENSSL_ALGO_SHA256);
    echo openssl_verify("message " . $i, $signature, $public, OPENSSL_ALGO_SHA256);
    echo count(openssl_pkey_get_details($private)["rsa"]);
}
echo "\n";
unset($private, $public, $signature, $i);
"##,
    ));
    assert_eq!(out.stdout, r##"181818
"##);
    assert!(out.stderr.contains("HEAP DEBUG: leak summary: clean"), "{}", out.stderr);
}
