---
title: "openssl_verify()"
description: "Verifies a signature with a public key: 1 valid, 0 invalid, -1 or false on error."
---

## openssl_verify()

```php
function openssl_verify(string $data, string $signature, mixed $public_key, mixed $algorithm = 1, int $padding = 0): int|false
```

Verifies a signature with a public key: 1 valid, 0 invalid, -1 or false on error.

**Parameters**:
- `$data` (`string`)
- `$signature` (`string`)
- `$public_key` (`mixed`)
- `$algorithm` (`mixed`), default `1`, optional
- `$padding` (`int`), default `0`, optional

**Returns**: `int|false`

## Availability

| Compiled (AOT) | Magician (eval) |
|---|---|
| Available | Not available yet |

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `openssl_verify` is implemented in the compiler, see [the internals page](../../../internals/builtins/string/openssl_verify.md).
