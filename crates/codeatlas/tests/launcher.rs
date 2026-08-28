//! The launcher exists only for a human at a terminal. Through pipes —
//! which is every script, every CI leg, and this test — a bare
//! `codeatlas` must keep printing clap's usage and exiting 2, exactly as
//! it did before the launcher existed: a script that ran the binary bare
//! to read its usage must never find an interview waiting on stdin.

#[test]
fn a_bare_invocation_through_pipes_still_prints_usage_and_exits_2() {
    let output = assert_cmd::Command::cargo_bin("codeatlas")
        .unwrap()
        .assert()
        .code(2);
    let stderr = String::from_utf8_lossy(&output.get_output().stderr).to_string();
    assert!(
        stderr.contains("Usage:"),
        "expected clap usage on a piped bare invocation, got: {stderr:?}"
    );
    assert!(
        !stderr.contains("repository path"),
        "the launcher's interview leaked into a pipe: {stderr:?}"
    );
}
