---
title: "move_uploaded_file() - internals"
description: "Compiler internals for move_uploaded_file(): lowering path, type checks, and runtime helpers."
sidebar:
  order: 982
---

## `move_uploaded_file()` - internals

## Where it lives

- **Signature**: [`crates/elephc-builtin-contract/src/catalog_data.rs`](https://github.com/illegalstudio/elephc/blob/main/crates/elephc-builtin-contract/src/catalog_data.rs)
- **Lowering**: [`src/web_prelude/build.rs`:3097](https://github.com/illegalstudio/elephc/blob/main/src/web_prelude/build.rs#L3097) (`move_uploaded_file`)
- **Function symbol**: `move_uploaded_file()`


### Lowering notes

- Implemented by the compiler-injected web prelude.

## Semantic descriptor

Shared contract implemented by an injected elephc-PHP prelude.

## EIR and runtime boundary

_Implemented by an injected elephc-PHP prelude._

## Signature summary

```php
function move_uploaded_file(string $from, string $to): bool
```

## What the type checker enforces

- **Arity**: takes exactly 2 arguments.

## Eval interpreter (magician)

_Not callable from eval'd code - the magician interpreter has no entry for this builtin._

## Cross-references

- [User reference for `move_uploaded_file()`](../../../php/builtins/web/move_uploaded_file.md)
