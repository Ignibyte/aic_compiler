//! Purpose:
//! The predefined integer constants of ext-filter (`FILTER_VALIDATE_*` / `FILTER_DEFAULT`)
//! and the file-upload error codes (`UPLOAD_ERR_*`), as `(name, value)` pairs.
//!
//! Called from:
//! - `crate::types::checker::driver::init` registers each name with `PhpType::Int` for the
//!   type checker.
//! - `crate::codegen_support::prescan` registers each `(name, value)` so a bare constant
//!   reference lowers to its integer literal.
//!
//! Key details:
//! - Values are the canonical PHP 8.5 constants. Only the commonly-used references (URL/email
//!   validation, `$_FILES` error handling) plus their sibling families are carried; the
//!   flag/option constants (`FILTER_FLAG_*`) are added on demand.

/// ext-filter validation filters + `FILTER_DEFAULT`, and the `UPLOAD_ERR_*` file-upload
/// error codes. All are integer constants.
pub(crate) const FILTER_INT_CONSTANTS: &[(&str, i64)] = &[
    ("FILTER_DEFAULT", 516),
    ("FILTER_VALIDATE_INT", 257),
    ("FILTER_VALIDATE_BOOL", 258),
    ("FILTER_VALIDATE_BOOLEAN", 258),
    ("FILTER_VALIDATE_FLOAT", 259),
    ("FILTER_VALIDATE_REGEXP", 272),
    ("FILTER_VALIDATE_DOMAIN", 277),
    ("FILTER_VALIDATE_URL", 273),
    ("FILTER_VALIDATE_EMAIL", 274),
    ("FILTER_VALIDATE_IP", 275),
    ("FILTER_VALIDATE_MAC", 276),
    ("UPLOAD_ERR_OK", 0),
    ("UPLOAD_ERR_INI_SIZE", 1),
    ("UPLOAD_ERR_FORM_SIZE", 2),
    ("UPLOAD_ERR_PARTIAL", 3),
    ("UPLOAD_ERR_NO_FILE", 4),
    ("UPLOAD_ERR_NO_TMP_DIR", 6),
    ("UPLOAD_ERR_CANT_WRITE", 7),
    ("UPLOAD_ERR_EXTENSION", 8),
    // parse_url() component selectors.
    ("PHP_URL_SCHEME", 0),
    ("PHP_URL_HOST", 1),
    ("PHP_URL_PORT", 2),
    ("PHP_URL_USER", 3),
    ("PHP_URL_PASS", 4),
    ("PHP_URL_PATH", 5),
    ("PHP_URL_QUERY", 6),
    ("PHP_URL_FRAGMENT", 7),
];
