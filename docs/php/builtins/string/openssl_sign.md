---
title: "openssl_sign()"
description: "Signs data with an RSA or EC private key and a digest, writing the signature by reference."
---

## openssl_sign()

```php
function openssl_sign(string $data, mixed $signature, mixed $private_key, mixed $algorithm = 1, int $padding = 0): bool
```

Signs data with an RSA or EC private key and a digest, writing the signature by reference.

**Parameters**:
- `$data` (`string`)
- `$signature` (`mixed`), passed by reference
- `$private_key` (`mixed`)
- `$algorithm` (`mixed`), default `1`, optional
- `$padding` (`int`), default `0`, optional

**Returns**: `bool`

## Availability

| Compiled (AOT) | Magician (eval) |
|---|---|
| Available | Not available yet |

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `openssl_sign` is implemented in the compiler, see [the internals page](../../../internals/builtins/string/openssl_sign.md).
