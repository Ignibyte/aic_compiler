---
title: "is_uploaded_file()"
description: "Tells whether the file was uploaded via HTTP POST."
sidebar:
  order: 985
---

## is_uploaded_file()

```php
function is_uploaded_file(string $filename): bool
```

Tells whether the file was uploaded via HTTP POST.

**Parameters**:
- `$filename` (`string`)

**Returns**: `bool`

## Availability

- **Compiled (AOT)**: supported through an injected elephc-PHP prelude.
- **`eval()` (magician interpreter)**: not available inside eval'd code.

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `is_uploaded_file` is implemented in the compiler, see [the internals page](../../../internals/builtins/web/is_uploaded_file.md).
