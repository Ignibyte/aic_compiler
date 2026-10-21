<?php

// Packed particles integrated in a buffer<T> hot loop. With --ir-opt (the default)
// the loop resolves the buffer handle once before it starts, and every $p[$i]
// access in one iteration shares a single bounds-checked record address.

packed class Particle
{
    public float $x;
    public float $vx;
}

function step(buffer<Particle> $p, int $count, float $dt): void
{
    for ($i = 0; $i < $count; $i++) {
        $x = $p[$i]->x + $p[$i]->vx * $dt;
        if ($x < 0.0 || $x > 100.0) {
            $p[$i]->vx = -$p[$i]->vx;
            $x = $p[$i]->x;
        }
        $p[$i]->x = $x;
    }
}

$count = 8;
$p = buffer_new<Particle>($count);
for ($i = 0; $i < $count; $i++) {
    $p[$i]->x = $i * 12.5;
    $p[$i]->vx = ($i % 3 - 1) * 7.5;
}
for ($frame = 0; $frame < 100; $frame++) {
    step($p, $count, 0.5);
}

$sum = 0.0;
for ($i = 0; $i < $count; $i++) {
    $sum += $p[$i]->x;
}
echo "particles=", $count, "; sum=", $sum, "\n";
buffer_free($p);
