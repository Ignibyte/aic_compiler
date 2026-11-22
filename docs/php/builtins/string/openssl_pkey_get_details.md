---
title: "openssl_pkey_get_details()"
description: "Returns the bits, PEM public key, type and RSA or EC parameters of a key."
---

## openssl_pkey_get_details()

```php
function openssl_pkey_get_details(mixed $key): array|false
```

Returns the bits, PEM public key, type and RSA or EC parameters of a key.

**Parameters**:
- `$key` (`mixed`)

**Returns**: `array|false`

## Availability

| Compiled (AOT) | Magician (eval) |
|---|---|
| Available | Not available yet |

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `openssl_pkey_get_details` is implemented in the compiler, see [the internals page](../../../internals/builtins/string/openssl_pkey_get_details.md).
