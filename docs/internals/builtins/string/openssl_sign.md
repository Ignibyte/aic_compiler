---
title: "openssl_sign() - internals"
description: "Compiler internals for openssl_sign(): lowering path, type checks, and runtime helpers."
---

## `openssl_sign()` - internals

## Where it lives

- **Signature**: [`crates/elephc-builtin-contract/src/catalog_surfaces.rs`](https://github.com/illegalstudio/elephc/blob/main/crates/elephc-builtin-contract/src/catalog_surfaces.rs)
- **Lowering**: [`src/openssl_prelude/build.rs`](https://github.com/illegalstudio/elephc/blob/main/src/openssl_prelude/build.rs) (`openssl_sign`)
- **Function symbol**: `openssl_sign()`


### Lowering notes

- Implemented by the compiler-injected openssl prelude.

## Semantic descriptor

Shared contract implemented by an injected elephc-PHP prelude.

## EIR and runtime boundary

_Implemented by an injected elephc-PHP prelude._

## Signature summary

```php
function openssl_sign(string $data, mixed $signature, mixed $private_key, mixed $algorithm = 1, int $padding = 0): bool
```

## What the type checker enforces

- **Arity**: takes 3–5 arguments (2 optional).
- **By-reference parameters**: `$signature`.

## Eval interpreter (magician)

_Not callable from eval'd code - the magician interpreter has no entry for this builtin._

## Cross-references

- [User reference for `openssl_sign()`](../../../php/builtins/string/openssl_sign.md)
