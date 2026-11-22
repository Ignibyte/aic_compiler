---
title: "openssl_pkey_get_private()"
description: "Loads a private key from PEM text or a file:// path, with an optional passphrase."
---

## openssl_pkey_get_private()

```php
function openssl_pkey_get_private(mixed $private_key, ?string $passphrase = null): mixed
```

Loads a private key from PEM text or a file:// path, with an optional passphrase.

**Parameters**:
- `$private_key` (`mixed`)
- `$passphrase` (`?string`), default `null`, optional

**Returns**: `mixed`

## Availability

| Compiled (AOT) | Magician (eval) |
|---|---|
| Available | Not available yet |

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `openssl_pkey_get_private` is implemented in the compiler, see [the internals page](../../../internals/builtins/string/openssl_pkey_get_private.md).
