use std::process::Command;

fn srchr() -> Command {
    Command::new(env!("CARGO_BIN_EXE_srchr"))
}

#[test]
fn help_does_not_require_tty() {
    let output = srchr().arg("--help").output().expect("run srchr --help");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("--query"));
}

#[test]
fn version_does_not_require_tty() {
    let output = srchr()
        .arg("--version")
        .output()
        .expect("run srchr --version");

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout is utf-8");
    assert!(stdout.contains("srchr"));
}
