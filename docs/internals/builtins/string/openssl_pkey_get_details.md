---
title: "openssl_pkey_get_details() - internals"
description: "Compiler internals for openssl_pkey_get_details(): lowering path, type checks, and runtime helpers."
---

## `openssl_pkey_get_details()` - internals

## Where it lives

- **Signature**: [`crates/elephc-builtin-contract/src/catalog_surfaces.rs`](https://github.com/illegalstudio/elephc/blob/main/crates/elephc-builtin-contract/src/catalog_surfaces.rs)
- **Lowering**: [`src/openssl_prelude/build.rs`](https://github.com/illegalstudio/elephc/blob/main/src/openssl_prelude/build.rs) (`openssl_pkey_get_details`)
- **Function symbol**: `openssl_pkey_get_details()`


### Lowering notes

- Implemented by the compiler-injected openssl prelude.

## Semantic descriptor

Shared contract implemented by an injected elephc-PHP prelude.

## EIR and runtime boundary

_Implemented by an injected elephc-PHP prelude._

## Signature summary

```php
function openssl_pkey_get_details(mixed $key): array|false
```

## What the type checker enforces

- **Arity**: takes exactly 1 argument.

## Eval interpreter (magician)

_Not callable from eval'd code - the magician interpreter has no entry for this builtin._

## Cross-references

- [User reference for `openssl_pkey_get_details()`](../../../php/builtins/string/openssl_pkey_get_details.md)
