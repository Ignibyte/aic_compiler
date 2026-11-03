<?php

$message = "hello preg";
if (preg_match("/^hello preg$/", $message)) {
    echo "Hello, preg!\n";
}

preg_match_all('/\b/u', $message, $boundaries, PREG_OFFSET_CAPTURE);
echo 'Word boundaries:';
foreach ($boundaries[0] as $boundary) {
    echo ' ', $boundary[1];
}
echo "\n";

// A /u pattern refuses a subject that is not valid UTF-8, as PHP does.
if (preg_match('/preg/u', "hello \xff preg") === false) {
    echo 'Rejected: ', preg_last_error_msg(), "\n";
}
