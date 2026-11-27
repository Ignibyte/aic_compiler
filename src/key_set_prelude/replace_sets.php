<?php

// The replace prelude: `array_replace()`, `array_replace_recursive()` and
// `array_merge_recursive()` for operands the native helpers cannot convert to a hash (a
// declared `array`, an empty literal or constant, a list of strings, arrays or objects). Each
// helper follows php-src's loop over the second array: `array_replace()` overwrites by key,
// `array_replace_recursive()` descends only where both sides hold an array, and
// `array_merge_recursive()` appends integer keys and folds a repeated string key into a list.

function __elephc_array_replace(array $array, array $replacements): array
{
    foreach ($replacements as $key => $value) {
        $array[$key] = $value;
    }
    return $array;
}

function __elephc_array_replace_recursive(array $array, array $replacements): array
{
    foreach ($replacements as $key => $value) {
        if (is_array($value) && array_key_exists($key, $array) && is_array($array[$key])) {
            $array[$key] = __elephc_array_replace_recursive($array[$key], $value);
            continue;
        }
        $array[$key] = $value;
    }
    return $array;
}

function __elephc_array_merge_recursive_into(array $merged, array $source): array
{
    foreach ($source as $key => $value) {
        if (is_int($key)) {
            $merged[] = $value;
            continue;
        }
        if (!array_key_exists($key, $merged)) {
            $merged[$key] = $value;
            continue;
        }
        $current = $merged[$key];
        if ($current === null) {
            $current = [null];
        } elseif (is_object($current)) {
            $current = (array) $current;
        } elseif (!is_array($current)) {
            $current = [$current];
        }
        if (is_object($value)) {
            $value = (array) $value;
        }
        if (is_array($value)) {
            $current = __elephc_array_merge_recursive_into($current, $value);
        } else {
            $current[] = $value;
        }
        $merged[$key] = $current;
    }
    return $merged;
}

function __elephc_array_merge_recursive(array $array, array $other): array
{
    return __elephc_array_merge_recursive_into(__elephc_array_merge_recursive_into([], $array), $other);
}
