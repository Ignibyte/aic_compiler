---
title: "move_uploaded_file()"
description: "Moves an uploaded file to a new location."
sidebar:
  order: 986
---

## move_uploaded_file()

```php
function move_uploaded_file(string $from, string $to): bool
```

Moves an uploaded file to a new location.

**Parameters**:
- `$from` (`string`)
- `$to` (`string`)

**Returns**: `bool`

## Availability

- **Compiled (AOT)**: supported through an injected elephc-PHP prelude.
- **`eval()` (magician interpreter)**: not available inside eval'd code.

_No examples yet. Check `examples/` and `showcases/` for usage patterns._

## Internals

For how `move_uploaded_file` is implemented in the compiler, see [the internals page](../../../internals/builtins/web/move_uploaded_file.md).
