use std::process::Command;

fn run(fixture: &str) -> (String, String, bool) {
    let path = format!("tests/fixtures/{fixture}");
    let output = Command::new(env!("CARGO_BIN_EXE_txs-eng"))
        .arg(&path)
        .env("RUST_LOG", "warn")
        .output()
        .expect("failed to run binary");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (stdout, stderr, output.status.success())
}

#[test]
fn valid_transactions() {
    let (stdout, stderr, success) = run("valid.csv");

    assert!(success);
    assert!(stderr.is_empty());

    let mut lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "client,available,held,total,locked");
    lines.remove(0);
    lines.sort();
    assert_eq!(lines[0], "1,75,0,75,false");
    assert_eq!(lines[1], "2,50,0,50,false");
}

#[test]
fn errors_warn_but_do_not_block() {
    let (stdout, stderr, success) = run("with_errors.csv");

    assert!(success);
    assert!(stderr.contains("unrecognized transaction type"));
    assert!(stderr.contains("missing amount"));

    let lines: Vec<&str> = stdout.lines().collect();
    assert_eq!(lines[0], "client,available,held,total,locked");
    assert_eq!(lines[1], "1,75,0,75,false");
}
