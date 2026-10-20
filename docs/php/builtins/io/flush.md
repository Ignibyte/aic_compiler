---
title: "flush()"
description: "Flushes the system write buffers of PHP and whatever backend PHP is using."
sidebar:
  order: 321
---

## flush()

```php
function flush(): void
```

Flushes the system write buffers of PHP and whatever backend PHP is using.

**Parameters**: none.

**Returns**: `void`

## Availability

- **Compiled (AOT)**: supported by the Elephc code generator.
- **`eval()` (magician interpreter)**: supported through a declarative interpreter builtin ([`crates/elephc-magician/src/interpreter/builtins/core/flush.rs`](https://github.com/illegalstudio/elephc/blob/main/crates/elephc-magician/src/interpreter/builtins/core/flush.rs)).

**Examples**:

```php
// Full example: examples/web-flush/main.php
header('X-Report: building');
flush(); // under --web the status and headers leave now
sleep(1);
echo "done\n";
```

## Internals

For how `flush` is implemented in the compiler, see [the internals page](../../../internals/builtins/io/flush.md).
