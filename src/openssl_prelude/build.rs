//! Purpose:
//! Builds the openssl key prelude's declarations as AST: the `elephc_openssl` key ABI,
//! the `OpenSSLAsymmetricKey` class and php-src's key, sign and verify functions.
//!
//! Called from:
//! - `crate::openssl_prelude::openssl_declarations`.
//!
//! Key details:
//! - Each helper was transcribed from the PHP form in `super::fragments` and is held to it
//!   node for node by `super::oracle_tests`; the behaviour is documented there and in the
//!   parent module, so this file is only the shape.

use crate::parser::ast::{BinOp, CType, CastType, Program, Stmt, TypeExpr};
use crate::synthetic_class::*;

/// `elephc_openssl_pkey_load` — transcribed from the PHP form.
fn decl_extern_elephc_openssl_pkey_load() -> Stmt {
    extern_fn("elephc_openssl_pkey_load", "elephc_openssl")
        .param("private", CType::Int)
        .param("data", CType::Str)
        .param("len", CType::Int)
        .param("passphrase", CType::Str)
        .param("passphrase_len", CType::Int)
        .returns(CType::Int)
        .build()
}

/// `elephc_openssl_pkey_sign` — transcribed from the PHP form.
fn decl_extern_elephc_openssl_pkey_sign() -> Stmt {
    extern_fn("elephc_openssl_pkey_sign", "elephc_openssl")
        .param("pem", CType::Str)
        .param("pem_len", CType::Int)
        .param("data", CType::Str)
        .param("len", CType::Int)
        .param("digest", CType::Str)
        .param("padding", CType::Int)
        .returns(CType::Int)
        .build()
}

/// `elephc_openssl_pkey_verify` — transcribed from the PHP form.
fn decl_extern_elephc_openssl_pkey_verify() -> Stmt {
    extern_fn("elephc_openssl_pkey_verify", "elephc_openssl")
        .param("pem", CType::Str)
        .param("pem_len", CType::Int)
        .param("data", CType::Str)
        .param("len", CType::Int)
        .param("signature", CType::Str)
        .param("signature_len", CType::Int)
        .param("digest", CType::Str)
        .param("padding", CType::Int)
        .returns(CType::Int)
        .build()
}

/// `elephc_openssl_pkey_details` — transcribed from the PHP form.
fn decl_extern_elephc_openssl_pkey_details() -> Stmt {
    extern_fn("elephc_openssl_pkey_details", "elephc_openssl")
        .param("pem", CType::Str)
        .param("pem_len", CType::Int)
        .param("private", CType::Int)
        .returns(CType::Int)
        .build()
}

/// `elephc_openssl_pkey_result` — transcribed from the PHP form.
fn decl_extern_elephc_openssl_pkey_result() -> Stmt {
    extern_fn("elephc_openssl_pkey_result", "elephc_openssl")
        .returns(CType::Ptr)
        .build()
}

/// `OpenSSLAsymmetricKey` — transcribed from the PHP form.
fn decl_class_opensslasymmetrickey() -> Stmt {
    class("OpenSSLAsymmetricKey")
        .final_()
        .private_prop("__elephc_pem", TypeExpr::Str, Some(e_str("")))
        .private_prop("__elephc_private", TypeExpr::Bool, Some(e_bool(false)))
        .method(
            method("__construct")
                .private(),
        )
        .method(
            method("__elephc_key_pem")
                .static_()
                .param("key", t_class("OpenSSLAsymmetricKey"))
                .returns(TypeExpr::Str)
                .body(vec![
                    s_return(e_prop(e_var("key"), "__elephc_pem")),
                ]),
        )
        .method(
            method("__elephc_is_private")
                .static_()
                .param("key", t_class("OpenSSLAsymmetricKey"))
                .returns(TypeExpr::Bool)
                .body(vec![
                    s_return(e_prop(e_var("key"), "__elephc_private")),
                ]),
        )
        .method(
            method("__elephc_result")
                .static_()
                .param("len", TypeExpr::Int)
                .returns(TypeExpr::Str)
                .body(vec![
                    s_return(e_call("__elephc_ptr_read_string", vec![e_call("elephc_openssl_pkey_result", vec![]), e_var("len")])),
                ]),
        )
        .method(
            method("__elephc_load")
                .static_()
                .param("text", TypeExpr::Str)
                .param("private", TypeExpr::Bool)
                .param("passphrase", TypeExpr::Str)
                .returns(t_union(vec![t_class("OpenSSLAsymmetricKey"), TypeExpr::False]))
                .body(vec![
                    s_if(
                        e_call("str_starts_with", vec![e_var("text"), e_str("file://")]),
                        vec![
                            s_assign("_path", e_call("substr", vec![e_var("text"), e_int(7)])),
                            s_if(
                                e_not(e_call("is_file", vec![e_var("_path")])),
                                vec![
                                    s_return(e_bool(false)),
                                ],
                                vec![],
                                None,
                            ),
                            s_assign("_read", e_call("file_get_contents", vec![e_var("_path")])),
                            s_if(
                                e_binop(e_var("_read"), BinOp::StrictEq, e_bool(false)),
                                vec![
                                    s_return(e_bool(false)),
                                ],
                                vec![],
                                None,
                            ),
                            s_assign("text", e_var("_read")),
                        ],
                        vec![],
                        None,
                    ),
                    s_assign("_len", e_call("elephc_openssl_pkey_load", vec![e_ternary(e_var("private"), e_int(1), e_int(0)), e_var("text"), e_call("strlen", vec![e_var("text")]), e_var("passphrase"), e_call("strlen", vec![e_var("passphrase")])])),
                    s_if(
                        e_binop(e_var("_len"), BinOp::LtEq, e_int(0)),
                        vec![
                            s_return(e_bool(false)),
                        ],
                        vec![],
                        None,
                    ),
                    s_assign("_key", e_new("OpenSSLAsymmetricKey", vec![])),
                    s_prop_assign(e_var("_key"), "__elephc_pem", e_static_call("OpenSSLAsymmetricKey", "__elephc_result", vec![e_var("_len")])),
                    s_prop_assign(e_var("_key"), "__elephc_private", e_var("private")),
                    s_return(e_var("_key")),
                ]),
        )
        .method(
            method("__elephc_argument")
                .static_()
                .param("key", t_mixed())
                .param("private", TypeExpr::Bool)
                .param("function", TypeExpr::Str)
                .param("passphrase", TypeExpr::Str)
                .returns(t_union(vec![t_class("OpenSSLAsymmetricKey"), TypeExpr::False]))
                .body(vec![
                    s_if(
                        e_call("is_array", vec![e_var("key")]),
                        vec![
                            s_if(
                                e_binop(e_not(e_call("array_key_exists", vec![e_int(0), e_var("key")])), BinOp::Or, e_not(e_call("array_key_exists", vec![e_int(1), e_var("key")]))),
                                vec![
                                    s_throw(e_new("ValueError", vec![e_str("Key array must be of the form array(0 => key, 1 => phrase)")])),
                                ],
                                vec![],
                                None,
                            ),
                            s_assign("passphrase", e_cast(CastType::String, e_index(e_var("key"), e_int(1)))),
                            s_assign("key", e_index(e_var("key"), e_int(0))),
                        ],
                        vec![],
                        None,
                    ),
                    s_if(
                        e_instance_of(e_var("key"), "OpenSSLAsymmetricKey"),
                        vec![
                            s_assign("_held", e_prop(e_var("key"), "__elephc_private")),
                            s_if(
                                e_binop(e_var("_held"), BinOp::StrictEq, e_var("private")),
                                vec![
                                    s_assign("_copy", e_new("OpenSSLAsymmetricKey", vec![])),
                                    s_prop_assign(e_var("_copy"), "__elephc_pem", e_prop(e_var("key"), "__elephc_pem")),
                                    s_prop_assign(e_var("_copy"), "__elephc_private", e_var("_held")),
                                    s_return(e_var("_copy")),
                                ],
                                vec![],
                                None,
                            ),
                            s_if(
                                e_var("private"),
                                vec![
                                    s_expr(e_call("__elephc_builtin_warning", vec![e_binop(e_var("function"), BinOp::Concat, e_str("(): Supplied key param is a public key"))])),
                                ],
                                vec![],
                                Some(vec![
                                s_expr(e_call("__elephc_builtin_warning", vec![e_binop(e_var("function"), BinOp::Concat, e_str("(): Don't know how to get public key from this private key"))])),
                            ]),
                            ),
                            s_return(e_bool(false)),
                        ],
                        vec![],
                        None,
                    ),
                    s_if(
                        e_not(e_call("is_string", vec![e_var("key")])),
                        vec![
                            s_return(e_bool(false)),
                        ],
                        vec![],
                        None,
                    ),
                    s_return(e_static_call("OpenSSLAsymmetricKey", "__elephc_load", vec![e_var("key"), e_var("private"), e_var("passphrase")])),
                ]),
        )
        .method(
            method("__elephc_digest")
                .static_()
                .param("algorithm", t_mixed())
                .returns(TypeExpr::Str)
                .body(vec![
                    s_if(
                        e_call("is_string", vec![e_var("algorithm")]),
                        vec![
                            s_return(e_var("algorithm")),
                        ],
                        vec![],
                        None,
                    ),
                    s_return(e_match(e_cast(CastType::Int, e_var("algorithm")), vec![(vec![e_int(1)], e_str("sha1")), (vec![e_int(2)], e_str("md5")), (vec![e_int(3)], e_str("md4")), (vec![e_int(6)], e_str("sha224")), (vec![e_int(7)], e_str("sha256")), (vec![e_int(8)], e_str("sha384")), (vec![e_int(9)], e_str("sha512")), (vec![e_int(10)], e_str("ripemd160"))], Some(e_str("")))),
                ]),
        )
        .method(
            method("__elephc_refusal")
                .static_()
                .param("function", TypeExpr::Str)
                .param("status", TypeExpr::Int)
                .returns(TypeExpr::Void)
                .body(vec![
                    s_if(
                        e_binop(e_var("status"), BinOp::StrictEq, e_neg(e_int(11))),
                        vec![
                            s_expr(e_call("__elephc_builtin_warning", vec![e_binop(e_var("function"), BinOp::Concat, e_str("(): Unknown digest algorithm"))])),
                        ],
                        vec![
                        (e_binop(e_var("status"), BinOp::StrictEq, e_neg(e_int(12))), vec![
                            s_expr(e_call("__elephc_builtin_warning", vec![e_binop(e_var("function"), BinOp::Concat, e_str("(): Padding parameter is only supported for RSA keys"))])),
                        ]),
                        (e_binop(e_var("status"), BinOp::StrictEq, e_neg(e_int(13))), vec![
                            s_expr(e_call("__elephc_builtin_warning", vec![e_binop(e_var("function"), BinOp::Concat, e_str("(): Unknown padding type"))])),
                        ]),
                    ],
                        None,
                    ),
                ]),
        )
        .build()
}

/// `openssl_pkey_get_public` — transcribed from the PHP form.
fn decl_fn_openssl_pkey_get_public() -> Stmt {
    function("openssl_pkey_get_public")
        .param("public_key", t_mixed())
        .returns(t_union(vec![t_class("OpenSSLAsymmetricKey"), TypeExpr::False]))
        .body(vec![
            s_return(e_static_call("OpenSSLAsymmetricKey", "__elephc_argument", vec![e_var("public_key"), e_bool(false), e_str("openssl_pkey_get_public"), e_str("")])),
        ])
        .build()
}

/// `openssl_get_publickey` — transcribed from the PHP form.
fn decl_fn_openssl_get_publickey() -> Stmt {
    function("openssl_get_publickey")
        .param("public_key", t_mixed())
        .returns(t_union(vec![t_class("OpenSSLAsymmetricKey"), TypeExpr::False]))
        .body(vec![
            s_return(e_static_call("OpenSSLAsymmetricKey", "__elephc_argument", vec![e_var("public_key"), e_bool(false), e_str("openssl_get_publickey"), e_str("")])),
        ])
        .build()
}

/// `openssl_pkey_get_private` — transcribed from the PHP form.
fn decl_fn_openssl_pkey_get_private() -> Stmt {
    function("openssl_pkey_get_private")
        .param("private_key", t_mixed())
        .param_default("passphrase", t_nullable(TypeExpr::Str), e_null())
        .returns(t_union(vec![t_class("OpenSSLAsymmetricKey"), TypeExpr::False]))
        .body(vec![
            s_return(e_static_call("OpenSSLAsymmetricKey", "__elephc_argument", vec![e_var("private_key"), e_bool(true), e_str("openssl_pkey_get_private"), e_cast(CastType::String, e_var("passphrase"))])),
        ])
        .build()
}

/// `openssl_get_privatekey` — transcribed from the PHP form.
fn decl_fn_openssl_get_privatekey() -> Stmt {
    function("openssl_get_privatekey")
        .param("private_key", t_mixed())
        .param_default("passphrase", t_nullable(TypeExpr::Str), e_null())
        .returns(t_union(vec![t_class("OpenSSLAsymmetricKey"), TypeExpr::False]))
        .body(vec![
            s_return(e_static_call("OpenSSLAsymmetricKey", "__elephc_argument", vec![e_var("private_key"), e_bool(true), e_str("openssl_get_privatekey"), e_cast(CastType::String, e_var("passphrase"))])),
        ])
        .build()
}

/// `openssl_sign` — transcribed from the PHP form.
fn decl_fn_openssl_sign() -> Stmt {
    function("openssl_sign")
        .param("data", TypeExpr::Str)
        .param_by_ref("signature", Some(t_mixed()))
        .param("private_key", t_mixed())
        .param_default("algorithm", t_mixed(), e_int(1))
        .param_default("padding", TypeExpr::Int, e_int(0))
        .returns(TypeExpr::Bool)
        .body(vec![
            s_assign("_key", e_static_call("OpenSSLAsymmetricKey", "__elephc_argument", vec![e_var("private_key"), e_bool(true), e_str("openssl_sign"), e_str("")])),
            s_if(
                e_not(e_instance_of(e_var("_key"), "OpenSSLAsymmetricKey")),
                vec![
                    s_expr(e_call("__elephc_builtin_warning", vec![e_str("openssl_sign(): Supplied key param cannot be coerced into a private key")])),
                    s_return(e_bool(false)),
                ],
                vec![],
                None,
            ),
            s_assign("_pem", e_static_call("OpenSSLAsymmetricKey", "__elephc_key_pem", vec![e_var("_key")])),
            s_assign("_digest", e_static_call("OpenSSLAsymmetricKey", "__elephc_digest", vec![e_var("algorithm")])),
            s_assign("_len", e_call("elephc_openssl_pkey_sign", vec![e_var("_pem"), e_call("strlen", vec![e_var("_pem")]), e_var("data"), e_call("strlen", vec![e_var("data")]), e_var("_digest"), e_var("padding")])),
            s_if(
                e_binop(e_var("_len"), BinOp::Gt, e_int(0)),
                vec![
                    s_assign("signature", e_static_call("OpenSSLAsymmetricKey", "__elephc_result", vec![e_var("_len")])),
                    s_return(e_bool(true)),
                ],
                vec![],
                None,
            ),
            s_expr(e_static_call("OpenSSLAsymmetricKey", "__elephc_refusal", vec![e_str("openssl_sign"), e_var("_len")])),
            s_return(e_bool(false)),
        ])
        .build()
}

/// `openssl_verify` — transcribed from the PHP form.
fn decl_fn_openssl_verify() -> Stmt {
    function("openssl_verify")
        .param("data", TypeExpr::Str)
        .param("signature", TypeExpr::Str)
        .param("public_key", t_mixed())
        .param_default("algorithm", t_mixed(), e_int(1))
        .param_default("padding", TypeExpr::Int, e_int(0))
        .returns(t_union(vec![TypeExpr::Int, TypeExpr::False]))
        .body(vec![
            s_assign("_key", e_static_call("OpenSSLAsymmetricKey", "__elephc_argument", vec![e_var("public_key"), e_bool(false), e_str("openssl_verify"), e_str("")])),
            s_if(
                e_not(e_instance_of(e_var("_key"), "OpenSSLAsymmetricKey")),
                vec![
                    s_expr(e_call("__elephc_builtin_warning", vec![e_str("openssl_verify(): Supplied key param cannot be coerced into a public key")])),
                    s_return(e_bool(false)),
                ],
                vec![],
                None,
            ),
            s_assign("_pem", e_static_call("OpenSSLAsymmetricKey", "__elephc_key_pem", vec![e_var("_key")])),
            s_assign("_digest", e_static_call("OpenSSLAsymmetricKey", "__elephc_digest", vec![e_var("algorithm")])),
            s_assign("_verdict", e_call("elephc_openssl_pkey_verify", vec![e_var("_pem"), e_call("strlen", vec![e_var("_pem")]), e_var("data"), e_call("strlen", vec![e_var("data")]), e_var("signature"), e_call("strlen", vec![e_var("signature")]), e_var("_digest"), e_var("padding")])),
            s_if(
                e_binop(e_var("_verdict"), BinOp::GtEq, e_neg(e_int(1))),
                vec![
                    s_return(e_var("_verdict")),
                ],
                vec![],
                None,
            ),
            s_expr(e_static_call("OpenSSLAsymmetricKey", "__elephc_refusal", vec![e_str("openssl_verify"), e_var("_verdict")])),
            s_if(
                e_binop(e_var("_verdict"), BinOp::StrictEq, e_neg(e_int(11))),
                vec![
                    s_return(e_bool(false)),
                ],
                vec![],
                None,
            ),
            s_return(e_neg(e_int(1))),
        ])
        .build()
}

/// `openssl_pkey_get_details` — transcribed from the PHP form.
fn decl_fn_openssl_pkey_get_details() -> Stmt {
    function("openssl_pkey_get_details")
        .param("key", t_class("OpenSSLAsymmetricKey"))
        .returns(t_union(vec![t_array(), TypeExpr::False]))
        .body(vec![
            s_assign("_pem", e_static_call("OpenSSLAsymmetricKey", "__elephc_key_pem", vec![e_var("key")])),
            s_assign("_held", e_static_call("OpenSSLAsymmetricKey", "__elephc_is_private", vec![e_var("key")])),
            s_assign("_len", e_call("elephc_openssl_pkey_details", vec![e_var("_pem"), e_call("strlen", vec![e_var("_pem")]), e_ternary(e_var("_held"), e_int(1), e_int(0))])),
            s_if(
                e_binop(e_var("_len"), BinOp::LtEq, e_int(0)),
                vec![
                    s_return(e_bool(false)),
                ],
                vec![],
                None,
            ),
            s_assign("_details", e_array(vec![])),
            s_assign("_group", e_array(vec![])),
            s_assign("_group_name", e_str("")),
            s_assign("_lines", e_call("explode", vec![e_str("\n"), e_static_call("OpenSSLAsymmetricKey", "__elephc_result", vec![e_var("_len")])])),
            s_foreach(e_var("_lines"), None, "_line", vec![
                s_if(
                    e_binop(e_var("_line"), BinOp::StrictEq, e_str("")),
                    vec![
                        s_continue(1),
                    ],
                    vec![],
                    None,
                ),
                s_assign("_parts", e_call("explode", vec![e_str("="), e_var("_line"), e_int(2)])),
                s_assign("_name", e_index(e_var("_parts"), e_int(0))),
                s_assign("_value", e_index(e_var("_parts"), e_int(1))),
                s_if(
                    e_binop(e_var("_name"), BinOp::StrictEq, e_str("bits")),
                    vec![
                        s_array_assign("_details", e_str("bits"), e_cast(CastType::Int, e_var("_value"))),
                    ],
                    vec![
                    (e_binop(e_var("_name"), BinOp::StrictEq, e_str("type")), vec![
                        s_if(
                            e_binop(e_var("_group_name"), BinOp::StrictNotEq, e_str("")),
                            vec![
                                s_array_assign("_details", e_var("_group_name"), e_var("_group")),
                            ],
                            vec![],
                            None,
                        ),
                        s_array_assign("_details", e_str("type"), e_cast(CastType::Int, e_var("_value"))),
                    ]),
                    (e_binop(e_var("_name"), BinOp::StrictEq, e_str("key")), vec![
                        s_array_assign("_details", e_str("key"), e_call("hex2bin", vec![e_var("_value")])),
                    ]),
                ],
                    Some(vec![
                    s_assign("_field", e_call("explode", vec![e_str("."), e_var("_name"), e_int(2)])),
                    s_assign("_group_name", e_index(e_var("_field"), e_int(0))),
                    s_array_assign("_group", e_index(e_var("_field"), e_int(1)), e_call("hex2bin", vec![e_var("_value")])),
                ]),
                ),
            ]),
            s_return(e_var("_details")),
        ])
        .build()
}

/// Builds the whole surface, one declaration per helper above.
pub(super) fn openssl_declarations() -> Program {
    internal_declarations(|| {
        vec![
            decl_extern_elephc_openssl_pkey_load(),
            decl_extern_elephc_openssl_pkey_sign(),
            decl_extern_elephc_openssl_pkey_verify(),
            decl_extern_elephc_openssl_pkey_details(),
            decl_extern_elephc_openssl_pkey_result(),
            decl_class_opensslasymmetrickey(),
            decl_fn_openssl_pkey_get_public(),
            decl_fn_openssl_get_publickey(),
            decl_fn_openssl_pkey_get_private(),
            decl_fn_openssl_get_privatekey(),
            decl_fn_openssl_sign(),
            decl_fn_openssl_verify(),
            decl_fn_openssl_pkey_get_details(),
        ]
    })
}
