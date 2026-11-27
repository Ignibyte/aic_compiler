<?php
function __elephc_get_debug_type_object(object $value): string {
    $class = get_class($value);
    return str_contains($class, '@anonymous') ? 'class@anonymous' : $class;
}
function get_debug_type(mixed $value): string {
    if ($value === null) { return 'null'; }
    if (is_bool($value)) { return 'bool'; }
    if (is_int($value)) { return 'int'; }
    if (is_float($value)) { return 'float'; }
    if (is_string($value)) { return 'string'; }
    if (is_array($value)) { return 'array'; }
    if ($value instanceof Closure) { return 'Closure'; }
    if (is_object($value)) { return __elephc_get_debug_type_object($value); }
    $type = get_resource_type($value);
    if ($type === 'Unknown') { return 'resource (closed)'; }
    return 'resource (' . $type . ')';
}
