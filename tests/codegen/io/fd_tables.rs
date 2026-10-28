//! Purpose:
//! Regression tests for the runtime's per-descriptor tables (`_eof_flags`,
//! `_zstream_handles`, `_tls_sessions`, `_stream_write_filters`, ...) once a
//! program holds descriptors past the old 256-slot bound.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - Each fixture opens 300 handles to one file first, so the streams it then
//!   exercises sit on descriptors at or above 256, where the tables used to be
//!   indexed past their end into neighbouring BSS symbols.
//! - Expected output is PHP's own. The test process raises its soft
//!   `RLIMIT_NOFILE` (inherited by the compiled program) and skips when the hard
//!   limit is too low to open the handles.

use crate::support::*;

/// Raises the soft open-file limit to `wanted` descriptors, which the compiled
/// fixture inherits; returns false when the hard limit does not allow it.
fn raise_open_file_limit(wanted: libc::rlim_t) -> bool {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit/setrlimit only read and write the local `limit` struct.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) != 0 {
            return false;
        }
        if limit.rlim_cur >= wanted {
            return true;
        }
        if limit.rlim_max != libc::RLIM_INFINITY && limit.rlim_max < wanted {
            return false;
        }
        limit.rlim_cur = wanted;
        libc::setrlimit(libc::RLIMIT_NOFILE, &limit) == 0
    }
}

/// Compiles and runs `body` after a prologue that holds 300 open handles, and
/// closes them after it; returns `None` when the descriptor limit is too low.
fn run_with_high_descriptors(body: &str) -> Option<String> {
    if !raise_open_file_limit(1024) {
        eprintln!("skipping: RLIMIT_NOFILE hard limit is below 1024");
        return None;
    }
    let source = format!(
        r#"<?php
file_put_contents("fd_tables_data.txt", "high");
$handles = [];
for ($i = 0; $i < 300; $i++) {{
    $handles[] = fopen("fd_tables_data.txt", "r");
}}
{body}
foreach ($handles as $h) {{
    fclose($h);
}}
"#
    );
    Some(compile_and_run(&source))
}

/// Verifies that closing descriptors past 256 after a zlib.deflate filter ran on a
/// low descriptor neither reads a stale zlib slot nor calls the close helper.
///
/// The original failure: fclose read `_zstream_handles[fd]` past the table, found the
/// next BSS global non-zero, and jumped through the zlib close helper (SIGSEGV).
#[test]
fn test_fd_tables_high_descriptor_close_after_zlib_filter() {
    let Some(out) = run_with_high_descriptors(
        r#"$z = fopen("fd_tables_low.bin", "w");
stream_filter_append($z, "zlib.deflate", STREAM_FILTER_WRITE);
fwrite($z, "low descriptor payload");
fclose($z);
$count = 0;
foreach ($handles as $h) {
    $s = fread($h, 10);
    $s .= fread($h, 10);
    if ($s === "high" && feof($h)) {
        $count++;
    }
}
echo $count, "\n";"#,
    ) else {
        return;
    };
    assert_eq!(out, "300\n");
}

/// Verifies fwrite, rewind, fgets, fread, feof, stream_get_meta_data, fseek,
/// stream_set_chunk_size and fclose on a descriptor past 256 behave as in PHP.
#[test]
fn test_fd_tables_high_descriptor_read_write_eof_and_chunk_size() {
    let Some(out) = run_with_high_descriptors(
        r#"$f = fopen("fd_tables_rw.txt", "w+");
echo fwrite($f, "line one\nline two\n"), "|";
rewind($f);
echo fgets($f), "|";
echo fread($f, 100), "|";
echo fread($f, 100) === "" ? "empty" : "more", "|";
echo feof($f) ? "eof" : "open", "|";
echo stream_get_meta_data($f)["eof"] ? "meta-eof" : "meta-open", "|";
fseek($f, 0);
echo feof($f) ? "eof" : "open", "|";
echo stream_set_chunk_size($f, 4096), "|", stream_set_chunk_size($f, 512), "|";
echo fclose($f) ? "closed" : "fail", "\n";"#,
    ) else {
        return;
    };
    assert_eq!(
        out,
        "18|line one\n|line two\n|empty|eof|meta-eof|open|8192|4096|closed\n"
    );
}

/// Verifies stream filters, glob:// directory streams and process pipes attach and
/// detach on descriptors past 256 the way PHP does.
#[test]
fn test_fd_tables_high_descriptor_filters_glob_and_popen() {
    let Some(out) = run_with_high_descriptors(
        r#"$u = fopen("fd_tables_upper.txt", "w");
$filter = stream_filter_append($u, "string.toupper", STREAM_FILTER_WRITE);
fwrite($u, "shout ");
stream_filter_remove($filter);
fwrite($u, "quiet");
fclose($u);
echo file_get_contents("fd_tables_upper.txt"), "|";
$z = fopen("fd_tables_deflate.bin", "w");
stream_filter_append($z, "zlib.deflate", STREAM_FILTER_WRITE);
fwrite($z, "high descriptor deflate payload");
fclose($z);
$r = fopen("fd_tables_deflate.bin", "r");
stream_filter_append($r, "zlib.inflate", STREAM_FILTER_READ);
echo stream_get_contents($r), "|";
fclose($r);
file_put_contents("fd_tables_g_a.txt", "a");
file_put_contents("fd_tables_g_b.txt", "b");
$d = opendir("glob://fd_tables_g_*.txt");
$names = [];
while (($e = readdir($d)) !== false) {
    $names[] = $e;
}
closedir($d);
sort($names);
echo implode(",", $names), "|";
$p = popen("echo piped", "r");
echo trim(fread($p, 100)), "|", pclose($p), "\n";"#,
    ) else {
        return;
    };
    assert_eq!(
        out,
        "SHOUT quiet|high descriptor deflate payload|fd_tables_g_a.txt,fd_tables_g_b.txt|piped|0\n"
    );
}
