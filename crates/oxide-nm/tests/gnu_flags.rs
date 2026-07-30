//! nm flag smoke (GNU -S = print-size).

use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oxide-nm"))
}

#[test]
fn print_size_flag_accepted() {
    let p = PathBuf::from("/bin/ls");
    if !p.exists() {
        return;
    }
    let out = Command::new(bin())
        .args(["-S", p.to_str().unwrap()])
        .output()
        .expect("run oxide-nm");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout);
    // With -S, defined symbols show a size column (hex) between addr and type
    // Format: ADDR SIZE TYPE NAME — look for lines with multiple fields
    let sample = text.lines().find(|l| l.len() > 20 && !l.contains('U'));
    if let Some(line) = sample {
        let parts: Vec<_> = line.split_whitespace().collect();
        assert!(
            parts.len() >= 4,
            "expected addr size type name, got: {line}"
        );
    }
}

#[test]
fn size_sort_is_long_only() {
    // --size-sort should not conflict with -S
    let help = Command::new(bin())
        .arg("--help")
        .output()
        .expect("help");
    let h = String::from_utf8_lossy(&help.stdout);
    assert!(h.contains("print-size") || h.contains("-S"));
    assert!(h.contains("size-sort"));
}
