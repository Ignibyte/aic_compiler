---
title: "openssl_get_privatekey()"
description: "Alias of openssl_pkey_get_private."
---

## openssl_get_privatekey()

```php
function openssl_get_privatekey(mixed $private_key, ?string $passphrase = null): mixed
```

Alias of openssl_pkey_get_private.

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

For how `openssl_get_privatekey` is implemented in the compiler, see [the internals page](../../../internals/builtins/string/openssl_get_privatekey.md).
