use std::collections::HashMap;
use std::process::Command;

fn run_engine(fixture: &str) -> String {
    let output = Command::new("cargo")
        .args(["run", "--", &format!("tests/fixtures/{}", fixture)])
        .output()
        .expect("failed to execute process");

    assert!(
        output.status.success(),
        "process failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("invalid utf8")
}

/// Parse CSV output into a map of client_id -> (available, held, total, locked)
fn parse_output(output: &str) -> HashMap<u16, (String, String, String, bool)> {
    let mut map = HashMap::new();
    for line in output.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(',').collect();
        assert_eq!(parts.len(), 5, "expected 5 columns, got: {}", line);
        let client: u16 = parts[0].trim().parse().unwrap();
        let available = parts[1].trim().to_string();
        let held = parts[2].trim().to_string();
        let total = parts[3].trim().to_string();
        let locked: bool = parts[4].trim().parse().unwrap();
        map.insert(client, (available, held, total, locked));
    }
    map
}

#[test]
fn test_pdf_example() {
    let output = run_engine("sample.csv");
    let accounts = parse_output(&output);

    assert_eq!(accounts.len(), 2);

    let (avail, held, total, locked) = &accounts[&1];
    assert_eq!(avail, "1.5000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "1.5000");
    assert!(!locked);

    let (avail, held, total, locked) = &accounts[&2];
    assert_eq!(avail, "2.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "2.0000");
    assert!(!locked);
}

#[test]
fn test_dispute_resolve_lifecycle() {
    let output = run_engine("dispute_lifecycle.csv");
    let accounts = parse_output(&output);

    // deposit 100 + deposit 50 - withdrawal 30 = 120
    // dispute on tx1 (100) then resolve → funds restored
    let (avail, held, total, locked) = &accounts[&1];
    assert_eq!(avail, "120.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "120.0000");
    assert!(!locked);
}

#[test]
fn test_chargeback_freezes_account() {
    let output = run_engine("chargeback_lifecycle.csv");
    let accounts = parse_output(&output);

    // deposit 50 + deposit 25 = 75 available
    // dispute tx1 (50) → available=25, held=50
    // chargeback tx1 → held=0, total=25, locked=true
    // deposit 10 → rejected (locked)
    // withdrawal 5 → rejected (locked)
    let (avail, held, total, locked) = &accounts[&1];
    assert_eq!(avail, "25.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "25.0000");
    assert!(locked);
}

#[test]
fn test_multiple_clients() {
    let output = run_engine("multiple_clients.csv");
    let accounts = parse_output(&output);

    assert_eq!(accounts.len(), 3);

    // Client 1: 100 - 50 = 50
    let (avail, _, total, locked) = &accounts[&1];
    assert_eq!(avail, "50.0000");
    assert_eq!(total, "50.0000");
    assert!(!locked);

    // Client 2: 200 - 100 + 50 = 150
    let (avail, _, total, locked) = &accounts[&2];
    assert_eq!(avail, "150.0000");
    assert_eq!(total, "150.0000");
    assert!(!locked);

    // Client 3: 300, dispute+resolve → back to 300
    let (avail, held, total, locked) = &accounts[&3];
    assert_eq!(avail, "300.0000");
    assert_eq!(held, "0.0000");
    assert_eq!(total, "300.0000");
    assert!(!locked);
}

#[test]
fn test_empty_file() {
    let output = run_engine("empty.csv");
    let accounts = parse_output(&output);
    assert_eq!(accounts.len(), 0);
}

#[test]
fn test_whitespace_handling() {
    let output = run_engine("whitespace.csv");
    let accounts = parse_output(&output);

    // 10.5678 + 5.0000 - 2.1234 = 13.4444
    let (avail, _, total, _) = &accounts[&1];
    assert_eq!(avail, "13.4444");
    assert_eq!(total, "13.4444");
}
