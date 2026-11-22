<?php
// flush(): send the response head before the slow part of a request.
// pool and request isolation stream the response; the default worker model buffers it whole.
// Compile: cargo run -- --web --web-isolation=pool examples/web-flush/main.php
// Run:     ./examples/web-flush/main --listen 127.0.0.1:8080
// Try:     curl -N -i http://127.0.0.1:8080/
//          The status and X-Report header arrive at once; each line follows a second apart.

header('Content-Type: text/plain; charset=utf-8');
header('X-Report: building');
flush();

for ($step = 1; $step <= 3; $step++) {
    sleep(1);
    echo "step ", $step, " done\n";
}
