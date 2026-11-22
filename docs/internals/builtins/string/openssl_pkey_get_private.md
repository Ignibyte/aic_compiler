---
title: "openssl_pkey_get_private() - internals"
description: "Compiler internals for openssl_pkey_get_private(): lowering path, type checks, and runtime helpers."
---

## `openssl_pkey_get_private()` - internals

## Where it lives

- **Signature**: [`crates/elephc-builtin-contract/src/catalog_surfaces.rs`](https://github.com/illegalstudio/elephc/blob/main/crates/elephc-builtin-contract/src/catalog_surfaces.rs)
- **Lowering**: [`src/openssl_prelude/build.rs`](https://github.com/illegalstudio/elephc/blob/main/src/openssl_prelude/build.rs) (`openssl_pkey_get_private`)
- **Function symbol**: `openssl_pkey_get_private()`


### Lowering notes

- Implemented by the compiler-injected openssl prelude.

## Semantic descriptor

Shared contract implemented by an injected elephc-PHP prelude.

## EIR and runtime boundary

_Implemented by an injected elephc-PHP prelude._

## Signature summary

```php
function openssl_pkey_get_private(mixed $private_key, ?string $passphrase = null): mixed
```

## What the type checker enforces

- **Arity**: takes 1–2 arguments (1 optional).

## Eval interpreter (magician)

_Not callable from eval'd code - the magician interpreter has no entry for this builtin._

## Cross-references

- [User reference for `openssl_pkey_get_private()`](../../../php/builtins/string/openssl_pkey_get_private.md)
