//! Purpose:
//! The PHP form of the openssl key prelude: the oracle `crate::openssl_prelude::build`
//! is transcribed from and compared against, node by node.
//!
//! Called from:
//! - `crate::openssl_prelude::parsed_prelude` under `cfg(test)`.
//!
//! Key details:
//! - The compiler never tokenizes this text; the built declarations are what it injects.
//! - The helpers are static methods of `OpenSSLAsymmetricKey`, which the class catalog
//!   hides from reflection, so the only PHP-visible functions are php-src's own.
//! - `serialize()` refuses the class through `NOT_SERIALIZABLE_BUILTIN_CLASSES`, and
//!   `clone` through the class catalog's `cloneable: false`, with php-src's messages.
//! - Warnings carry php-src's text and label through `__elephc_builtin_warning`.
//! - The `elephc_openssl_pkey_*` statuses are `crate::pkey_abi`'s in the bridge crate:
//!   -10 bad key, -11 unknown digest, -12 padding needs RSA, -13 unknown padding,
//!   -14 refused by libcrypto, -18 internal.

/// The complete openssl key prelude as PHP source.
pub(crate) const SRC: &str = r#"<?php
extern "elephc_openssl" {
    function elephc_openssl_pkey_load(int $private, string $data, int $len, string $passphrase, int $passphrase_len): int;
    function elephc_openssl_pkey_sign(string $pem, int $pem_len, string $data, int $len, string $digest, int $padding): int;
    function elephc_openssl_pkey_verify(string $pem, int $pem_len, string $data, int $len, string $signature, int $signature_len, string $digest, int $padding): int;
    function elephc_openssl_pkey_details(string $pem, int $pem_len, int $private): int;
    function elephc_openssl_pkey_result(): ptr;
}

final class OpenSSLAsymmetricKey
{
    private string $__elephc_pem = '';
    private bool $__elephc_private = false;

    private function __construct()
    {
    }

    public static function __elephc_key_pem(OpenSSLAsymmetricKey $key): string
    {
        return $key->__elephc_pem;
    }

    public static function __elephc_is_private(OpenSSLAsymmetricKey $key): bool
    {
        return $key->__elephc_private;
    }

    public static function __elephc_result(int $len): string
    {
        return __elephc_ptr_read_string(elephc_openssl_pkey_result(), $len);
    }

    public static function __elephc_load(string $text, bool $private, string $passphrase): OpenSSLAsymmetricKey|false
    {
        if (str_starts_with($text, 'file://')) {
            $_path = substr($text, 7);
            if (!is_file($_path)) {
                return false;
            }
            $_read = file_get_contents($_path);
            if ($_read === false) {
                return false;
            }
            $text = $_read;
        }
        $_len = elephc_openssl_pkey_load($private ? 1 : 0, $text, strlen($text), $passphrase, strlen($passphrase));
        if ($_len <= 0) {
            return false;
        }
        $_key = new OpenSSLAsymmetricKey();
        $_key->__elephc_pem = OpenSSLAsymmetricKey::__elephc_result($_len);
        $_key->__elephc_private = $private;
        return $_key;
    }

    public static function __elephc_argument(mixed $key, bool $private, string $function, string $passphrase): OpenSSLAsymmetricKey|false
    {
        if (is_array($key)) {
            if (!array_key_exists(0, $key) || !array_key_exists(1, $key)) {
                throw new ValueError('Key array must be of the form array(0 => key, 1 => phrase)');
            }
            $passphrase = (string) $key[1];
            $key = $key[0];
        }
        if ($key instanceof OpenSSLAsymmetricKey) {
            $_held = $key->__elephc_private;
            if ($_held === $private) {
                $_copy = new OpenSSLAsymmetricKey();
                $_copy->__elephc_pem = $key->__elephc_pem;
                $_copy->__elephc_private = $_held;
                return $_copy;
            }
            if ($private) {
                __elephc_builtin_warning($function . '(): Supplied key param is a public key');
            } else {
                __elephc_builtin_warning($function . "(): Don't know how to get public key from this private key");
            }
            return false;
        }
        if (!is_string($key)) {
            return false;
        }
        return OpenSSLAsymmetricKey::__elephc_load($key, $private, $passphrase);
    }

    public static function __elephc_digest(mixed $algorithm): string
    {
        if (is_string($algorithm)) {
            return $algorithm;
        }
        return match ((int) $algorithm) {
            1 => 'sha1',
            2 => 'md5',
            3 => 'md4',
            6 => 'sha224',
            7 => 'sha256',
            8 => 'sha384',
            9 => 'sha512',
            10 => 'ripemd160',
            default => '',
        };
    }

    public static function __elephc_refusal(string $function, int $status): void
    {
        if ($status === -11) {
            __elephc_builtin_warning($function . '(): Unknown digest algorithm');
        } elseif ($status === -12) {
            __elephc_builtin_warning($function . '(): Padding parameter is only supported for RSA keys');
        } elseif ($status === -13) {
            __elephc_builtin_warning($function . '(): Unknown padding type');
        }
    }
}

function openssl_pkey_get_public(mixed $public_key): OpenSSLAsymmetricKey|false
{
    return OpenSSLAsymmetricKey::__elephc_argument($public_key, false, 'openssl_pkey_get_public', '');
}

function openssl_get_publickey(mixed $public_key): OpenSSLAsymmetricKey|false
{
    return OpenSSLAsymmetricKey::__elephc_argument($public_key, false, 'openssl_get_publickey', '');
}

function openssl_pkey_get_private(mixed $private_key, ?string $passphrase = null): OpenSSLAsymmetricKey|false
{
    return OpenSSLAsymmetricKey::__elephc_argument($private_key, true, 'openssl_pkey_get_private', (string) $passphrase);
}

function openssl_get_privatekey(mixed $private_key, ?string $passphrase = null): OpenSSLAsymmetricKey|false
{
    return OpenSSLAsymmetricKey::__elephc_argument($private_key, true, 'openssl_get_privatekey', (string) $passphrase);
}

function openssl_sign(string $data, mixed &$signature, mixed $private_key, mixed $algorithm = 1, int $padding = 0): bool
{
    $_key = OpenSSLAsymmetricKey::__elephc_argument($private_key, true, 'openssl_sign', '');
    if (!($_key instanceof OpenSSLAsymmetricKey)) {
        __elephc_builtin_warning('openssl_sign(): Supplied key param cannot be coerced into a private key');
        return false;
    }
    $_pem = OpenSSLAsymmetricKey::__elephc_key_pem($_key);
    $_digest = OpenSSLAsymmetricKey::__elephc_digest($algorithm);
    $_len = elephc_openssl_pkey_sign($_pem, strlen($_pem), $data, strlen($data), $_digest, $padding);
    if ($_len > 0) {
        $signature = OpenSSLAsymmetricKey::__elephc_result($_len);
        return true;
    }
    OpenSSLAsymmetricKey::__elephc_refusal('openssl_sign', $_len);
    return false;
}

function openssl_verify(string $data, string $signature, mixed $public_key, mixed $algorithm = 1, int $padding = 0): int|false
{
    $_key = OpenSSLAsymmetricKey::__elephc_argument($public_key, false, 'openssl_verify', '');
    if (!($_key instanceof OpenSSLAsymmetricKey)) {
        __elephc_builtin_warning('openssl_verify(): Supplied key param cannot be coerced into a public key');
        return false;
    }
    $_pem = OpenSSLAsymmetricKey::__elephc_key_pem($_key);
    $_digest = OpenSSLAsymmetricKey::__elephc_digest($algorithm);
    $_verdict = elephc_openssl_pkey_verify($_pem, strlen($_pem), $data, strlen($data), $signature, strlen($signature), $_digest, $padding);
    if ($_verdict >= -1) {
        return $_verdict;
    }
    OpenSSLAsymmetricKey::__elephc_refusal('openssl_verify', $_verdict);
    if ($_verdict === -11) {
        return false;
    }
    return -1;
}

function openssl_pkey_get_details(OpenSSLAsymmetricKey $key): array|false
{
    $_pem = OpenSSLAsymmetricKey::__elephc_key_pem($key);
    $_held = OpenSSLAsymmetricKey::__elephc_is_private($key);
    $_len = elephc_openssl_pkey_details($_pem, strlen($_pem), $_held ? 1 : 0);
    if ($_len <= 0) {
        return false;
    }
    $_details = [];
    $_group = [];
    $_group_name = '';
    $_lines = explode("\n", OpenSSLAsymmetricKey::__elephc_result($_len));
    foreach ($_lines as $_line) {
        if ($_line === '') {
            continue;
        }
        $_parts = explode('=', $_line, 2);
        $_name = $_parts[0];
        $_value = $_parts[1];
        if ($_name === 'bits') {
            $_details['bits'] = (int) $_value;
        } elseif ($_name === 'type') {
            if ($_group_name !== '') {
                $_details[$_group_name] = $_group;
            }
            $_details['type'] = (int) $_value;
        } elseif ($_name === 'key') {
            $_details['key'] = hex2bin($_value);
        } else {
            $_field = explode('.', $_name, 2);
            $_group_name = $_field[0];
            $_group[$_field[1]] = hex2bin($_value);
        }
    }
    return $_details;
}
"#;
