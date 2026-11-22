<?php

// `preg_match($pattern, $subject, $matches)` for a literal pattern with named groups. The
// compiled regex functions number every group; php-src also stores each named group under its
// name, just before its number. The rewritten call is
// `__elephc_preg_named_result($matches = __elephc_preg_named_matches($pattern, $subject, $names))`:
// the first helper returns the named array, and the second answers what preg_match() returned.

final class __ElephcPregNamedGroups
{
    /** The last result: 1, 0, or -1 for false. */
    public static int $last = 0;
}

function __elephc_preg_named_matches(string $pattern, string $subject, array $names): array
{
    $raw = [];
    $result = preg_match($pattern, $subject, $raw);
    __ElephcPregNamedGroups::$last = $result === false ? -1 : $result;
    $named = [];
    foreach ($raw as $index => $value) {
        if (isset($names[$index])) {
            $named[$names[$index]] = $value;
        }
        $named[$index] = $value;
    }
    return $named;
}

function __elephc_preg_named_result(array $matches): int|false
{
    $last = __ElephcPregNamedGroups::$last;
    return $last === -1 ? false : $last;
}
