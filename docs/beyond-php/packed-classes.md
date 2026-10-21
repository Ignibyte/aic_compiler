---
title: "Packed Classes"
description: "Flat POD record types with compile-time field offsets for hot-path storage."
sidebar:
  order: 3
---

A `packed class` is a flat record with compile-time field offsets. Designed for hot-path data that needs predictable layouts and zero hash overhead.

> **Strict mode:** `packed class` is an elephc extension with no PHP equivalent. Compiling with [`--strict-php`](../compiling/cli-reference.md#strict-php-mode) rejects it.

## Declaration
```php
<?php
packed class Enemy {
    int $x;
    int $y;
    int $hp;
    int $state;
}
```

## Constraints
- Fields must be `int`, `float`, `bool`, `ptr`, or another `packed class`
- No union or nullable types
- No strings, arrays, objects, or mixed values
- No methods, constructors, inheritance, traits, or interfaces
- Only `public` visibility (optional and redundant)
- No default values
- Layout is sequential: each `int`, `float`, `bool`, or `ptr` field takes 8 bytes, and a nested packed field takes the full size of its record, so in `Body` below `$pos` is at offset 0, `$vel` at 16 and `$id` at 32 (40 bytes in total)

## Usage with buffers
```php
<?php
packed class Vertex {
    float $x;
    float $y;
    float $u;
    float $v;
    int $color;
}

buffer<Vertex> $verts = buffer_new<Vertex>(1024);
$verts[0]->x = 10.0;
echo $verts[0]->x;  // 10
```

A packed class name is a type wherever a type is accepted: a `buffer<Vertex>`
or `ptr<Vertex>` can be a class property, a method parameter or return type, a
promoted constructor parameter, or a generic type argument. See
[buffers in class members](buffers.md#buffers-in-class-members).

## Nested records

A field whose type is another packed class embeds that record in place. Chained access
resolves the inner field from the summed compile-time offsets, for reads, writes, and
compound assignments, at any depth:

```php
<?php
packed class Vec2 { public float $x; public float $y; }
packed class Body { public Vec2 $pos; public Vec2 $vel; public int $id; }

$bodies = buffer_new<Body>(2);
$bodies[0]->pos->x = 1.25;
$bodies[0]->vel->y += 0.5;
$bodies[1]->id++;
echo $bodies[0]->pos->x;   // 1.25
echo ptr_sizeof("Body");   // 40
```

The embedded class may be declared before or after the class that embeds it. A packed class
cannot embed itself, directly or through other packed classes, because the record would have
no finite size.

Reading a nested record (`$bodies[0]->pos`) yields the record's address, like reading a
buffer element: a local holding it writes through to the buffer. Assigning a whole record
to a nested field copies its bytes, and the source must be a record of the same packed
class: a buffer element, another nested field, or a `ptr<T>` typed pointer. Like C struct
assignment, the copy assumes the source and the destination do not partially overlap.

```php
<?php
$bodies[1]->pos = $bodies[0]->pos;   // copies 16 bytes
$bodies[1]->pos = 5;                 // compile error: Body::pos takes a Vec2 record
```

A whole buffer element is still assigned field by field (`$bodies[0] = ...` is rejected).

## Typed pointers

A `ptr<T>` whose target is a packed class reaches the same fields, which maps a packed
record onto memory a C library owns:

```php
<?php
extern function calloc(u64 $count, u64 $size): ptr;

$body = ptr_cast<Body>(calloc(1, ptr_sizeof("Body")));
$body->pos->y = 4.5;
$body->id = 7;
```

Field access through a null `ptr<T>` aborts with `Fatal error: null pointer dereference`,
like the other pointer dereference helpers.

## Best practices
- Use packed classes for structured hot-path data instead of associative arrays
- Prefer SoA (parallel buffers) for large datasets where you iterate one field at a time
- Use AoS (buffer of packed class) when you always access all fields together
