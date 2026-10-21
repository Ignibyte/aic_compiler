---
title: "Buffers"
description: "Fixed-size contiguous arrays with buffer<T> for hot-path data and game loops."
sidebar:
  order: 2
---

`buffer<T>` is a fixed-size contiguous array of POD values or packed records. Designed for game loops, renderers, and performance-critical code where hash table overhead is unacceptable.

> **Strict mode:** buffers are an elephc extension with no PHP equivalent. Compiling with [`--strict-php`](../compiling/cli-reference.md#strict-php-mode) rejects the `buffer<T>` type, `buffer_new<T>`, `buffer_len()`, and `buffer_free()`.

## Why not PHP arrays?

PHP arrays are hash tables. Every access goes through hashing, probing, and entry
comparison. A `buffer<T>` access instead resolves an opaque handle, checks the handle
generation and bounds, then performs direct address arithmetic over a contiguous
payload: `payload + index * stride`. Repeated immutable integer index arithmetic can
be shared or moved out of loops by the EIR optimizer.

## Creating buffers

```php
<?php
buffer<int> $ids = buffer_new<int>(1000);
buffer<float> $speeds = buffer_new<float>(1000);
buffer<Enemy> $enemies = buffer_new<Enemy>(256);
```

Only POD scalar, pointer, or packed-record element types are accepted. No union types (`buffer<int|string>`) or nullable (`buffer<?int>`).

## Buffer builtins

| Function | Signature | Description |
|---|---|---|
| `buffer_new<T>()` | `buffer_new<T>($length): buffer<T>` | Allocate a fixed-size buffer with `$length` elements of type `T` |
| `buffer_len()` | `buffer_len($buffer): int` | Return the logical element count stored in the buffer descriptor |
| `buffer_free()` | `buffer_free($buffer): void` | Release a local buffer variable and nullify it |

## Reading and writing

```php
<?php
$buf[3] = 42;          // direct store
echo $buf[3];           // direct load

$enemies[0]->x = 100;  // packed class field access
echo $enemies[0]->x;   // 100
```

Buffer indices may be statically typed as `int` or `mixed`. A `mixed` index is converted to `int` at runtime before bounds checking; a statically known non-integer index is rejected at compile time.

## Buffers in class members

Class members can name `buffer<T>` like any other type, including buffers of
packed classes: properties, method parameters and return types, promoted
constructor parameters, and generic type arguments.

```php
<?php
packed class Vec2 { public float $x; public float $y; }

final class Points {
    public buffer<Vec2> $data;

    public function __construct(int $n) {
        $this->data = buffer_new<Vec2>($n);
    }

    public function moveX(int $i, float $dx): void {
        $this->data[$i]->x = $this->data[$i]->x + $dx;
    }

    public function all(): buffer<Vec2> {
        return $this->data;
    }
}

final class Store<T> {
    public function __construct(public buffer<T> $items) {}
}

$store = new Store<Vec2>(buffer_new<Vec2>(16));
```

Packed element fields can be written through a buffer-typed property, as in
`moveX()` above. Writing a whole element through a property
(`$this->ids[$i] = 42` on a `buffer<int>` property) is not supported yet, and
a static property cannot hold a buffer yet.

## Buffer length

```php
<?php
echo buffer_len($data);   // 512
```

## Freeing buffers

```php
<?php
buffer_free($buf);   // release heap memory, nullify variable
```

Use-after-free produces: `Fatal error: use of buffer after buffer_free()`

Restrictions:

- Only accepts plain local variables
- Freeing a local nullifies that local; freeing the same local again is a no-op
- Existing aliases become stale and fail with the use-after-free fatal error, even if the descriptor slot and heap payload are later reused

## Bounds checking

Always enabled. Out-of-bounds aborts: `Fatal error: buffer index out of bounds`

## Length validation

`buffer_new<T>()` validates the requested length before allocating. A negative length, or a length
whose `length * stride` payload size does not fit in a machine word, aborts with:

```
Fatal error: buffer_new() length is negative or exceeds the maximum buffer size
```

This keeps the length recorded in the buffer header consistent with the memory the buffer actually
owns, so the bounds check above can never approve an index outside the allocation. A length that is
representable but larger than the configured heap still reports `Fatal error: heap memory exhausted`.

## Memory layout

The PHP-visible value is an opaque 64-bit handle, not a heap pointer:

```
Bits 63..32: generation (non-zero u32)
Bits 31..0:  descriptor index (1..=4096)
```

The handle resolves through a static descriptor registry. Each 48-byte descriptor contains:

```
Offset 0:   [payload pointer: 8 bytes]
Offset 8:   [length: 8 bytes]
Offset 16:  [stride: 8 bytes]
Offset 24:  [generation: 8-byte slot, low u32 used]
Offset 32:  [active marker: 8 bytes]
Offset 40:  [free-list successor: 8 bytes]
```

The payload is a separate heap allocation containing exactly `length * stride` zero-initialized bytes. Reusing a descriptor increments its generation, so a stale alias cannot resolve to a newer buffer lifetime.

## SoA vs AoS patterns

**Structure of Arrays (SoA)** — better cache locality for single-field iteration:

```php
<?php
buffer<float> $x = buffer_new<float>(1000);
buffer<float> $y = buffer_new<float>(1000);
for ($i = 0; $i < 1000; $i++) {
    $x[$i] = $x[$i] + $speed * $dt;
}
```

**Array of Structures (AoS)** — better when accessing all fields together:

```php
<?php
packed class Particle { float $x; float $y; float $vx; float $vy; }
buffer<Particle> $particles = buffer_new<Particle>(10000);
for ($i = 0; $i < buffer_len($particles); $i++) {
    $particles[$i]->x = $particles[$i]->x + $particles[$i]->vx;
}
```

## Limitations

- Fixed size — no push, pop, or dynamic resize
- No automatic cleanup — use `buffer_free()` explicitly
- No conversion to/from PHP arrays
- No copy-on-write semantics
- No `foreach` iteration
- No mixed element types
- Payload is zero-initialized by `buffer_new`
- At most 4096 buffers can be live simultaneously; descriptor slots are recycled after `buffer_free()`
