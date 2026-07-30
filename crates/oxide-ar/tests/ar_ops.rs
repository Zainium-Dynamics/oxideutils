//! Archive create / table / delete via oxide-ar binary.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oxide-ar"))
}

fn ranlib_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oxide-ranlib"))
}

#[test]
fn rcs_create_and_table() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.o");
    let b = dir.path().join("b.o");
    // Fake object-ish payloads (not real ELF — ar should still store them)
    fs::write(&a, b"object-a-payload").unwrap();
    fs::write(&b, b"object-b-payload-xx").unwrap();
    let lib = dir.path().join("libt.a");

    let status = Command::new(bin())
        .args([
            "rcs",
            lib.to_str().unwrap(),
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let out = Command::new(bin())
        .args(["t", lib.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("a.o"), "{text}");
    assert!(text.contains("b.o"), "{text}");

    // delete
    let status = Command::new(bin())
        .args(["d", lib.to_str().unwrap(), "a.o"])
        .status()
        .unwrap();
    assert!(status.success());
    let out = Command::new(bin())
        .args(["t", lib.to_str().unwrap()])
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!text.contains("a.o"), "{text}");
    assert!(text.contains("b.o"), "{text}");
}

/// oxide-ranlib is a separate bin entry (like binutils is-ranlib.c) that
/// shares lib logic with oxide-ar. This exercises the real installed binary.
#[test]
fn ranlib_rebuilds_symbol_index() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.o");
    fs::write(&a, b"object-a-payload").unwrap();
    let lib = dir.path().join("libt.a");

    let status = Command::new(bin())
        .args(["qc", lib.to_str().unwrap(), a.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    let status = Command::new(ranlib_bin())
        .args(["-D", lib.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());

    // Archive must still be well-formed and list its member after ranlib ran.
    let out = Command::new(bin())
        .args(["t", lib.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("a.o"));
}

#[test]
fn ranlib_errors_on_missing_archive() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("nope.a");
    let status = Command::new(ranlib_bin())
        .arg(missing.to_str().unwrap())
        .status()
        .unwrap();
    assert!(!status.success());
}
