//! Purpose:
//! Regression tests for ending a program while a thread it started is still running.
//!
//! Called from:
//! - `cargo test` through Rust's test harness.
//!
//! Key details:
//! - The C fixture starts a thread that never returns, as a database driver's worker threads do.
//! - `exit()` and an uncaught exception must end the whole process with PHP's status. An exit that
//!   ends only the calling thread (Linux `exit` rather than `exit_group`) leaves the process alive,
//!   and the runner's binary timeout fails the test.
//! - The fixture is assembled into the test program itself, so no shared library is needed.

use super::*;

/// C fixture whose one function starts a thread that waits forever.
const SLEEPER_FIXTURE: &str = r#"
#include <pthread.h>
#include <unistd.h>

static void *elephc_test_sleep_forever(void *unused) {
    (void)unused;
    for (;;) {
        pause();
    }
    return 0;
}

// Returns 0 once the thread is started, as pthread_create does.
long elephc_test_start_sleeper(void) {
    pthread_t thread;
    return pthread_create(&thread, 0, elephc_test_sleep_forever, 0);
}
"#;

/// Compiles `source` with the sleeper fixture, runs it, and returns its output and exit status
/// without asserting success.
fn run_with_sleeper(source: &str) -> ProgramOutput {
    let dir = make_cli_test_dir("elephc_test_live_threads");
    let (mut assembly, runtime, libraries) =
        compile_source_to_asm_with_options(source, &dir, 8_388_608, false, false);
    let fixture_asm = compile_c_fixture_asm(&dir, "fixture", SLEEPER_FIXTURE, &[]);
    append_c_fixture_asm(&mut assembly, &fixture_asm);
    let output = assemble_and_run_capture(
        &assembly,
        &runtime_obj_for_asm(&runtime),
        &dir,
        &libraries,
        &default_link_paths(),
        &[],
    );
    let _ = fs::remove_dir_all(&dir);
    output
}

/// Verifies `exit(3)` ends the process with status 3 while another thread is still running.
#[test]
fn test_exit_ends_a_program_with_a_live_thread() {
    let output = run_with_sleeper(
        r#"<?php
extern function elephc_test_start_sleeper(): int;
echo elephc_test_start_sleeper(), "\n";
exit(3);
"#,
    );
    assert_eq!(output.stdout, "0\n");
    assert_eq!(output.status.code(), Some(3), "{}", output.stderr);
}

/// Verifies an uncaught exception ends the process with PHP's status 255 while another thread is
/// still running, after its report.
#[test]
fn test_uncaught_exception_ends_a_program_with_a_live_thread() {
    let output = run_with_sleeper(
        r#"<?php
extern function elephc_test_start_sleeper(): int;
echo elephc_test_start_sleeper(), "\n";
throw new RuntimeException("boom");
"#,
    );
    assert_eq!(output.status.code(), Some(255), "{}\n{}", output.stdout, output.stderr);
    assert!(
        format!("{}{}", output.stdout, output.stderr).contains("Uncaught RuntimeException: boom"),
        "{}\n{}",
        output.stdout,
        output.stderr
    );
}
