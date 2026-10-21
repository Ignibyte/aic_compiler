---
title: "Pointers"
description: "Low-level memory access with ptr(), ptr_get(), ptr_set(), raw byte/string helpers, pointer arithmetic, and typed casting."
sidebar:
  order: 1
---

Pointers provide low-level memory access. A pointer is a 64-bit memory address stored in a single register.

> **Strict mode:** pointers are an elephc extension with no PHP equivalent. Compiling with [`--strict-php`](../compiling/cli-reference.md#strict-php-mode) rejects the `ptr` type, `ptr_cast<T>`, and every `ptr_*` builtin.

## Creating pointers

```php
<?php
$x = 42;
$p = ptr($x);        // take the address of a stack variable
$null = ptr_null();   // create a null pointer (0x0)
```

## Reading and writing

```php
<?php
$x = 10;
$p = ptr($x);
echo ptr_get($p);     // 10
ptr_set($p, 99);
echo $x;              // 99
```

## Pointer arithmetic

```php
<?php
$p = ptr($x);
$q = ptr_offset($p, 8);   // advance by 8 bytes
```

`ptr_offset` adds a **byte** offset.

## Typed pointers and casting

```php
<?php
$typed = ptr_cast<MyClass>($p);   // change type tag, same address
```

`ptr_cast<T>()` is compile-time only. When `T` is a packed class, fields are read and
written through the typed pointer (`$typed->field`, including nested records); see
[Packed Classes](packed-classes.md#typed-pointers). When `T` is an extern class (a C
struct), each field is read and written at its natural C offset with its declared width;
see [Extern classes](extern.md#extern-classes-c-structs).

## Querying sizes

```php
<?php
echo ptr_sizeof("int");      // 8
echo ptr_sizeof("string");   // 16
echo ptr_sizeof("Point");    // computed from class
```

## Sub-word access

```php
<?php
ptr_write8($p, 0x41);        // write 1 byte
echo ptr_read8($p);           // read 1 byte
ptr_write16($p, 0x1234);     // write 2 bytes, little-endian
echo ptr_read16($p);          // read 2 bytes, zero-extended
ptr_write32($p, 12345);      // write 4 bytes
echo ptr_read32($p);          // read 4 bytes
ptr_write_f32($p, 1.5);      // write an IEEE 754 single (4 bytes)
echo ptr_read_f32($p);        // read it back as a PHP float
ptr_write_f64($p, 0.1);      // write an IEEE 754 double (8 bytes)
echo ptr_read_f64($p);        // 0.1
```

`ptr_write_f32()` rounds the PHP float to single precision, so
`ptr_read_f32()` returns `0.10000000149011612` for a stored `0.1`; the 64-bit
pair keeps the value exactly. Both accept an `int` value and convert it.

## Null safety

Pointer dereference helpers abort on null pointer dereference. Use `ptr_is_null($p)` to check.

## Raw string copies

```php
<?php
extern function malloc(int $size): ptr;
extern function free(ptr $p): void;

$buf = malloc(64);

$written = ptr_write_string($buf, "HTTP/1.1 200 OK\r\n");
$line = ptr_read_string($buf, $written);

echo $line;
free($buf);
```

`ptr_read_string($p, $len)` copies exactly `$len` bytes into a new PHP string. It does not stop at `\0`, so binary payloads are preserved. `$len < 0` is a fatal error.

`ptr_write_string($p, $str)` copies the raw bytes of `$str` to `$p` without a trailing null terminator and returns the number of bytes written.

## Echo, string conversion and comparison

```php
<?php
echo $p;                      // prints hex: 0x16f502348
echo ptr_null();              // prints: 0x0
echo "p = " . $p;             // "p = 0x16f502348"
echo "p = $p";                // same text through interpolation
$s = (string)$p;              // "0x16f502348", also strval($p) and sprintf("%s", $p)
echo $p === $q ? "same" : "different";
echo gettype($p);             // "pointer"
```

A pointer converts to a string as `0x` followed by its address in lowercase
hexadecimal without leading zeros (`0x0` for a null pointer), on every target.
`echo`, `print`, concatenation, `.=`, interpolation in double-quoted strings and
heredocs, `(string)` casts, `strval()` and `%s` in `sprintf()`/`printf()` (with a
literal or a runtime format string) all produce that same text, for `ptr` and
`ptr<T>` values alike, including pointers returned by `extern` functions. To
hand the text to another string builtin, pass `(string)$p`.

A pointer is not a string, so it is still rejected where a `string` is required
by declaration: a `string` parameter or return type, `strlen()`, and loose
comparison with `==` (use `===`). The conversion applies to values the compiler
knows to be pointers: a pointer stored in a `mixed` or union-typed value, for
example `$v = $flag ? $p : "none"`, is held as its integer address there and
prints in decimal. `settype($p, "string")` needs that same mixed storage, so it
does not compile yet for a pointer variable; assign `(string)$p` to another
variable instead. Code running inside `eval()` also sees pointers as plain
integers, so `echo ptr_null()` there prints `0`.

## Function reference

| Function | Signature | Description |
|---|---|---|
| `ptr($var)` | `ptr($var): pointer` | Take address of a variable |
| `ptr_null()` | `ptr_null(): pointer` | Create a null pointer |
| `ptr_is_null($p)` | `ptr_is_null($p): bool` | Check if pointer is null |
| `ptr_get($p)` | `ptr_get($p): int` | Read 8 bytes at address |
| `ptr_set($p, $val)` | `ptr_set($p, $val): void` | Write 8 bytes at address |
| `ptr_offset($p, $n)` | `ptr_offset($p, $n): pointer` | Add byte offset |
| `ptr_cast<T>($p)` | `ptr_cast<T>($p): pointer` | Change type tag |
| `ptr_sizeof("type")` | `ptr_sizeof($t): int` | Get byte size of a type |
| `ptr_read8($p)` | `ptr_read8($p): int` | Read 1 byte |
| `ptr_write8($p, $v)` | `ptr_write8($p, $v): void` | Write 1 byte |
| `ptr_read16($p)` | `ptr_read16($p): int` | Read 2 little-endian bytes, zero-extended |
| `ptr_write16($p, $v)` | `ptr_write16($p, $v): void` | Write the low 16 bits as 2 little-endian bytes |
| `ptr_read32($p)` | `ptr_read32($p): int` | Read 4 bytes |
| `ptr_write32($p, $v)` | `ptr_write32($p, $v): void` | Write 4 bytes |
| `ptr_read_f32($p)` | `ptr_read_f32($p): float` | Read a 4-byte IEEE 754 single, widened to a PHP float |
| `ptr_write_f32($p, $v)` | `ptr_write_f32($p, $v): void` | Round a float (or int) to a single and write 4 bytes |
| `ptr_read_f64($p)` | `ptr_read_f64($p): float` | Read an 8-byte IEEE 754 double |
| `ptr_write_f64($p, $v)` | `ptr_write_f64($p, $v): void` | Write a float (or int) as an 8-byte double |
| `ptr_read_string($p, $len)` | `ptr_read_string($p, $len): string` | Copy exactly `$len` bytes into a new PHP string |
| `ptr_write_string($p, $str)` | `ptr_write_string($p, $str): int` | Copy string bytes into raw memory and return bytes written |

Notes:

- `ptr()` only accepts variables. `ptr(1 + 2)` is a compile-time error.
- `ptr_set()` writes a single 8-byte word (int, bool, null, or pointer); floats go through `ptr_write_f64()` or `ptr_write_f32()`.
- `ptr_cast<T>()` accepts builtin types, classes, packed classes, and extern classes.
- Use `===` and `!==` for comparison. `==`/`!=` is rejected.
