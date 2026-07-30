//! oxide-readelf Phase D CLI smoke.

use std::path::PathBuf;
use std::process::Command;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_oxide-readelf"))
}

#[test]
fn version_info_and_got() {
    let p = PathBuf::from("/bin/ls");
    if !p.exists() {
        return;
    }
    let out = Command::new(bin())
        .args(["-V", "--got-contents", "-u", p.to_str().unwrap()])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("Version")
            || text.contains("version")
            || text.contains("GOT")
            || text.contains("eh_frame"),
        "unexpected output length {}",
        text.len()
    );
}

#[test]
fn all_includes_phase_d() {
    let p = PathBuf::from("/bin/ls");
    if !p.exists() {
        return;
    }
    let out = Command::new(bin())
        .args(["-a", p.to_str().unwrap()])
        .output()
        .expect("run -a");
    assert!(out.status.success());
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.len() > 200);
}
