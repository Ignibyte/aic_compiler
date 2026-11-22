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
//! - `_eof_flags` has more slots (`EOF_FLAG_SLOTS`, 1 << 20) than the other tables
//!   (`FD_TABLE_SLOTS`, 65536); one fixture reads a stream past 65536 to its end,
//!   and a runtime-assembly check pins the table size and the feof bound.

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

/// Verifies `_eof_flags` is sized for 1 << 20 descriptors (Linux's default
/// `fs.nr_open`) and that the host runtime's `__rt_feof` bounds the lookup by that
/// count rather than by the 65536 slots of the other per-fd tables.
#[test]
fn test_fd_tables_eof_flags_cover_every_default_openable_descriptor() {
    let dir = std::env::temp_dir().join(format!(
        "elephc_fd_tables_eof_asm_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let (_, runtime_asm, _) = compile_source_to_asm_with_options(
        "<?php\n$f = fopen(\"php://memory\", \"r\");\necho feof($f) ? \"eof\" : \"open\";\n",
        &dir,
        8_388_608,
        false,
        false,
    );
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        runtime_asm.contains(".comm _eof_flags, 1048576,"),
        "_eof_flags is not sized for 1 << 20 descriptors"
    );
    let start = runtime_asm.find("rt_feof:").expect("__rt_feof in the runtime");
    let feof = &runtime_asm[start..];
    let feof = &feof[..feof.find("rt_feof_no_slot").expect("feof no-slot path")];
    let bound = if cfg!(target_arch = "x86_64") {
        "cmp rdi, 1048576"
    } else {
        "cmp x0, #256, lsl #12"
    };
    assert!(feof.contains(bound), "__rt_feof does not compare against 1 << 20:\n{feof}");
}

/// Verifies `feof()` turns true on a stream whose descriptor is past 65536, the
/// slot count of the other per-fd tables, once the stream is read to its end.
///
/// The test process places a file at descriptor 65540 with `dup2` (no
/// `FD_CLOEXEC`, so the compiled program inherits it) and the fixture opens it as
/// `php://fd/65540`. Skips when the hard `RLIMIT_NOFILE` does not allow 70000.
#[test]
fn test_fd_tables_feof_past_the_fd_table_slots() {
    const HIGH_FD: libc::c_int = 65540;
    if !raise_open_file_limit(70_000) {
        eprintln!("skipping: RLIMIT_NOFILE hard limit is below 70000");
        return;
    }
    let dir = std::env::temp_dir().join(format!(
        "elephc_fd_tables_high_fd_{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("high.txt");
    std::fs::write(&path, "high").unwrap();
    let file = std::fs::File::open(&path).unwrap();
    // SAFETY: dup2 duplicates a descriptor this test owns onto an unused number.
    let placed = unsafe {
        libc::dup2(std::os::unix::io::AsRawFd::as_raw_fd(&file), HIGH_FD)
    };
    assert_eq!(placed, HIGH_FD, "dup2 onto fd {HIGH_FD} failed");
    let out = compile_and_run(
        r#"<?php
$f = fopen("php://fd/65540", "r");
echo feof($f) ? "eof" : "open", "|";
$read = fread($f, 10);
$read .= fread($f, 10);
echo $read, "|", feof($f) ? "eof" : "open", "\n";
fclose($f);
"#,
    );
    // SAFETY: closes only the descriptor this test placed.
    unsafe {
        libc::close(HIGH_FD);
    }
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(out, "open|high|eof\n");
}
