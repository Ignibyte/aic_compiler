---
title: "openssl_pkey_get_public()"
description: "Loads a public key from a PEM key or certificate, a file:// path, or a key object."
---

## openssl_pkey_get_public()

```php
function openssl_pkey_get_public(mixed $public_key): mixed
```

Loads a public key from a PEM key or certificate, a file:// path, or a key object.

**Parameters**:
- `$public_key` (`mixed`)

**Returns**: `mixed`

## Availability

| Compiled (AOT) | Magician (eval) |
|---|---|
| Available | Not available yet |

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `openssl_pkey_get_public` is implemented in the compiler, see [the internals page](../../../internals/builtins/string/openssl_pkey_get_public.md).
