<?php

// The value-set prelude: `array_diff()`, `array_intersect()`, `array_unique()`,
// `array_diff_assoc()` and `array_intersect_assoc()` for operands the native helpers cannot
// compare as PHP does (strings, boxed values, objects, nested arrays, a declared `array`). PHP
// compares these values as `(string) $a === (string) $b` and keeps every survivor under its
// original key, so each helper renders a value once and looks the rendering up in a hash. The
// early returns follow php-src's fast paths, so the same values are rendered, in the same order.
// A rendering is held in a local before an `isset()` reads it: a cast written inside the
// subscript leaks its temporary string.

function __elephc_array_diff(array $array, array $other): array
{
    if (count($array) === 0) {
        return [];
    }
    if (count($array) === 1) {
        $search = '';
        foreach ($array as $value) {
            $search = (string) $value;
        }
        foreach ($other as $value) {
            if ((string) $value === $search) {
                return [];
            }
        }
        return $array;
    }
    if (count($other) === 0) {
        return $array;
    }
    $exclude = [];
    foreach ($other as $value) {
        $exclude[(string) $value] = true;
    }
    $result = [];
    foreach ($array as $key => $value) {
        $rendering = (string) $value;
        if (!isset($exclude[$rendering])) {
            $result[$key] = $value;
        }
    }
    return $result;
}

function __elephc_array_intersect(array $array, array $other): array
{
    if (count($array) === 0 || count($other) === 0) {
        return [];
    }
    $include = [];
    foreach ($other as $value) {
        $include[(string) $value] = true;
    }
    $result = [];
    foreach ($array as $key => $value) {
        $rendering = (string) $value;
        if (isset($include[$rendering])) {
            $result[$key] = $value;
        }
    }
    return $result;
}

function __elephc_array_unique(array $array): array
{
    if (count($array) <= 1) {
        return $array;
    }
    $seen = [];
    $result = [];
    foreach ($array as $key => $value) {
        $rendering = (string) $value;
        if (!isset($seen[$rendering])) {
            $seen[$rendering] = true;
            $result[$key] = $value;
        }
    }
    return $result;
}

function __elephc_array_diff_assoc(array $array, array $other): array
{
    $result = [];
    foreach ($array as $key => $value) {
        if (!array_key_exists($key, $other) || (string) $value !== (string) $other[$key]) {
            $result[$key] = $value;
        }
    }
    return $result;
}

function __elephc_array_intersect_assoc(array $array, array $other): array
{
    $result = [];
    foreach ($array as $key => $value) {
        if (array_key_exists($key, $other) && (string) $value === (string) $other[$key]) {
            $result[$key] = $value;
        }
    }
    return $result;
}
