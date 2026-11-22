//! Purpose:
//! Decides whether a parsed program references PHP's asymmetric openssl key surface.
//!
//! Called from:
//! - `crate::openssl_prelude::inject_if_used` and `program_uses_surface`.
//!
//! Key details:
//! - Runs before name resolution, so names are raw source text and PHP function/class
//!   lookup is case-insensitive. Only the unqualified last segment is compared, which
//!   matches `openssl_sign`, `\openssl_sign` and `\App\openssl_sign`.
//! - The function set is exact, never an `openssl_` prefix: the cipher functions are
//!   native builtins and must not inject this prelude.
//! - Soundness beats precision: a missed reference would remove declarations a valid
//!   program needs, while a false positive only adds declarations.

use crate::parser::ast::Stmt;

/// The PHP functions the prelude declares.
pub(crate) const OPENSSL_KEY_FUNCTIONS: &[&str] = &[
    "openssl_pkey_get_public",
    "openssl_get_publickey",
    "openssl_pkey_get_private",
    "openssl_get_privatekey",
    "openssl_sign",
    "openssl_verify",
    "openssl_pkey_get_details",
];

/// The class the prelude declares.
const OPENSSL_KEY_CLASS: &str = "OpenSSLAsymmetricKey";

/// Whether `name` is one of the surface functions, ignoring case.
fn is_key_function(name: &str) -> bool {
    OPENSSL_KEY_FUNCTIONS.iter().any(|candidate| name.eq_ignore_ascii_case(candidate))
}

/// Returns whether any top-level statement references the openssl key surface.
pub(super) fn program_uses_openssl_keys(program: &[Stmt]) -> bool {
    let usage = crate::prelude_prune::usage::collect(program);
    usage.functions.iter().any(|name| name.rsplit('\\').next().is_some_and(is_key_function))
        || usage.classes.iter().any(|name| {
            name.rsplit('\\')
                .next()
                .is_some_and(|segment| segment.eq_ignore_ascii_case(OPENSSL_KEY_CLASS))
        })
}
