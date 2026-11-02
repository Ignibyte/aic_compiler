<?php

// The key-set prelude: `array_intersect_key()` and `array_diff_key()` for operands the native
// helpers cannot take (a declared `array`, a boxed value, a list). The builtin checks retarget
// such a call here; a call on two concrete hashes keeps the native helper.

function __elephc_array_intersect_key(array $array, array $other): array
{
    $result = [];
    foreach ($array as $key => $value) {
        if (array_key_exists($key, $other)) {
            $result[$key] = $value;
        }
    }
    return $result;
}

function __elephc_array_diff_key(array $array, array $other): array
{
    $result = [];
    foreach ($array as $key => $value) {
        if (!array_key_exists($key, $other)) {
            $result[$key] = $value;
        }
    }
    return $result;
}
