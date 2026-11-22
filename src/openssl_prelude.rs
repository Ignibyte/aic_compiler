//! Purpose:
//! PHP's asymmetric `ext/openssl` key surface: the `OpenSSLAsymmetricKey` class and
//! `openssl_pkey_get_public`, `openssl_pkey_get_private`, `openssl_sign`, `openssl_verify`
//! and `openssl_pkey_get_details` (with the `openssl_get_publickey` and
//! `openssl_get_privatekey` aliases), built on the `elephc_openssl` bridge's key ABI.
//!
//! Called from:
//! - `crate::pipeline::compile()` and the codegen test harness via `inject_if_used`,
//!   before name resolution.
//!
//! Key details:
//! - WHY A PRELUDE. Declaring the `extern "elephc_openssl"` block links
//!   `libelephc_openssl.a` and, through it, the managed `openssl-crypto` package, so a
//!   program that never touches a key declares nothing and links nothing.
//! - BUILT, NOT PARSED. The declarations are `synthetic_class` builders transcribed from
//!   the PHP form in `fragments`; that text stays under `cfg(test)` as the parse-parity
//!   oracle, and the compiler never tokenizes it.
//! - STATELESS KEYS. A key object holds its normalized PEM and whether it is private;
//!   every sign, verify or details call parses that PEM again in the bridge. There is no
//!   native handle to free, so the object needs no destructor and no free path.
//! - The class contract hides the two storage properties and every helper method, so
//!   `var_dump`, reflection and `property_exists` see php-src's empty, final class.
//! - Supported: RSA and EC keys from PEM text, `file://` paths and `[key, passphrase]`
//!   arrays, every `OPENSSL_ALGO_*` digest and digest name libcrypto knows, and the
//!   PKCS#1 v1.5 and PSS paddings. Not supported: `openssl_pkey_new`, export, X.509
//!   objects, and signatures with Ed25519/Ed448 keys (which take no digest).
//! - DIVERGENCE: an encrypted key without its passphrase fails to load; php-src asks
//!   libcrypto, which prompts on the terminal.

mod build;
mod detect;
#[cfg(test)]
mod fragments;

use crate::parser::ast::Program;

/// Builds the openssl key surface declarations.
pub(crate) fn openssl_declarations() -> Program {
    build::openssl_declarations()
}

/// Parses the PHP form of the prelude; the oracle's reference side.
#[cfg(test)]
pub(crate) fn parsed_prelude() -> Program {
    let tokens = crate::lexer::tokenize(fragments::SRC).expect("openssl prelude PHP must tokenize");
    crate::parser::parse_internal(&tokens).expect("openssl prelude PHP must parse")
}

/// Whether `program` uses this prelude's surface; the same test `inject_if_used` applies.
///
/// Also run by `crate::autoload::prelude_probe` over the autoload closure, whose files
/// are only loaded after this prelude has been decided.
pub(crate) fn program_uses_surface(program: &[crate::parser::ast::Stmt]) -> bool {
    detect::program_uses_openssl_keys(program)
}

/// Injects the openssl key prelude when the program references the surface, leaving
/// every other program untouched.
///
/// `force` comes from an autoload-probe hit; otherwise the decision is
/// `detect::program_uses_openssl_keys`. The prelude carries only declarations, so
/// prepending it is order-independent: PHP hoists them.
pub fn inject_if_used(
    program: Program,
    force: bool,
    inventory: &mut crate::optimize::reachability::PreludeInventory,
) -> Program {
    crate::compiler_stack::with_compiler_stack(|| {
        if !force && !detect::program_uses_openssl_keys(&program) {
            return program;
        }
        let mut combined = openssl_declarations();
        inventory.record_program("openssl", &combined);
        combined.extend(program);
        combined
    })
}

#[cfg(test)]
mod oracle_tests {
    //! Purpose:
    //! Parse-parity oracle for the built openssl key surface: `build::openssl_declarations`
    //! must equal the parse of the PHP form, declaration by declaration.
    //!
    //! Called from:
    //! - `cargo test` through Rust's test harness.
    //!
    //! Key details:
    //! - Spans are stripped because a built node has none and a parsed one does; everything
    //!   else (order, types, nesting, name qualification) must match.

    use super::*;

    /// Removes span payloads so a built node and a parsed node compare on structure.
    fn strip_spans(rendered: &str) -> String {
        let mut cleaned = String::with_capacity(rendered.len());
        let mut rest = rendered;
        while let Some(at) = rest.find("Span {") {
            cleaned.push_str(&rest[..at]);
            cleaned.push_str("Span");
            let after = &rest[at..];
            let close = after.find('}').map(|end| end + 1).unwrap_or(after.len());
            rest = &after[close..];
        }
        cleaned.push_str(rest);
        cleaned
    }

    /// The built declarations are the parse of the PHP form, node for node.
    #[test]
    fn built_declarations_match_the_php() {
        let parsed = parsed_prelude();
        let built = openssl_declarations();
        assert_eq!(built.len(), parsed.len(), "declaration count");
        for (index, (built_stmt, parsed_stmt)) in built.iter().zip(parsed.iter()).enumerate() {
            assert_eq!(
                strip_spans(&format!("{built_stmt:?}")),
                strip_spans(&format!("{parsed_stmt:?}")),
                "declaration {index} differs from the PHP form"
            );
        }
    }

    /// The detector takes exactly the surface's names, never an `openssl_` prefix.
    #[test]
    fn detection_is_exact() {
        let uses = |source: &str| {
            let tokens = crate::lexer::tokenize(source).expect("tokenize");
            let program = crate::parser::parse(&tokens).expect("parse");
            program_uses_surface(&program)
        };
        assert!(uses("<?php openssl_sign('m', $s, 'k');"));
        assert!(uses("<?php \\OPENSSL_VERIFY('m', 's', 'k');"));
        assert!(uses("<?php function f(OpenSSLAsymmetricKey $k) {}"));
        assert!(!uses("<?php openssl_encrypt('m', 'aes-128-cbc', 'k');"));
        assert!(!uses("<?php openssl_sign_helper();"));
    }
}
