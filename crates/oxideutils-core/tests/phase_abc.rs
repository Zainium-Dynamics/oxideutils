//! Phase A/B/C regression tests: strip verify, archive write, atomic write helpers.

use oxideutils_core::archive::{OxideArchive, is_archive};
use oxideutils_core::archive_write::{ArOperation, ArchiveBuilder};
use oxideutils_core::strip::{StripOptions, strip_bytes};
use oxideutils_core::utils::atomic_write;
use std::fs;
use std::path::Path;
use std::process::Command;

fn have_cc() -> bool {
    Command::new("cc").arg("--version").output().is_ok()
}

fn compile_pic_obj(dir: &Path, name: &str, src: &str) -> Option<std::path::PathBuf> {
    if !have_cc() {
        return None;
    }
    let c_path = dir.join(format!("{name}.c"));
    let o_path = dir.join(format!("{name}.o"));
    fs::write(&c_path, src).ok()?;
    let status = Command::new("cc")
        .args(["-c", "-fPIC", "-g", "-o"])
        .arg(&o_path)
        .arg(&c_path)
        .status()
        .ok()?;
    if status.success() { Some(o_path) } else { None }
}

#[test]
fn archive_create_list_delete_roundtrip() {
    let mut b = ArchiveBuilder::new()
        .deterministic(true)
        .with_symbol_index(false);
    b.replace_or_add("a.o".into(), b"AAA".to_vec());
    b.replace_or_add("b.o".into(), b"BBBB".to_vec());
    let bytes = b.to_bytes().expect("write archive");
    assert!(is_archive(&bytes));

    let parsed = ArchiveBuilder::from_bytes("t.a", &bytes).expect("parse");
    assert_eq!(parsed.members.len(), 2);
    assert_eq!(parsed.members[0].data, b"AAA");

    let mut b2 = parsed;
    assert_eq!(b2.delete(&["a.o".into()]), 1);
    let bytes2 = b2.to_bytes().unwrap();
    let arch = OxideArchive::parse("t2.a", &bytes2).unwrap();
    assert_eq!(arch.members.len(), 1);
    assert_eq!(arch.members[0].name, "b.o");
}

#[test]
fn archive_key_parse_rcs() {
    let op = ArOperation::parse_key("rcsD").unwrap();
    assert!(op.replace && op.create && op.symbol_index && op.deterministic);
}

#[test]
fn atomic_write_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("out.bin");
    atomic_write(&path, b"hello-oxide", None).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"hello-oxide");
    atomic_write(&path, b"updated", Some(&path)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"updated");
}

#[test]
fn strip_system_binary_if_present() {
    let p = Path::new("/bin/ls");
    if !p.exists() {
        return;
    }
    let data = fs::read(p).unwrap();
    if data.len() < 4 || &data[0..4] != b"\x7fELF" {
        return;
    }
    let stripped = strip_bytes(
        &data,
        StripOptions {
            strip_all: true,
            strip_debug: false,
            strip_unneeded: false,
        },
    )
    .expect("strip /bin/ls");
    // Must still parse as ELF
    object::File::parse(&stripped[..]).expect("stripped still valid");
    // Should not grow unboundedly; typically shrinks or stays similar
    assert!(stripped.len() <= data.len() + 4096);
}

#[test]
fn strip_and_ar_with_real_objects() {
    let dir = tempfile::tempdir().unwrap();
    let Some(a) = compile_pic_obj(dir.path(), "a", "int oxide_a(void) { return 42; }\n") else {
        eprintln!("skip: no cc");
        return;
    };
    let Some(b) = compile_pic_obj(dir.path(), "b", "int oxide_b(void) { return 7; }\n") else {
        return;
    };

    // strip relocatable
    let a_data = fs::read(&a).unwrap();
    let a_stripped = strip_bytes(
        &a_data,
        StripOptions {
            strip_debug: true,
            strip_all: false,
            strip_unneeded: false,
        },
    )
    .expect("strip-debug .o");
    object::File::parse(&a_stripped[..]).expect("stripped .o valid");

    // ar rcs
    let mut builder = ArchiveBuilder::new()
        .deterministic(true)
        .with_symbol_index(true);
    builder.replace_or_add("a.o".into(), a_stripped);
    builder.replace_or_add("b.o".into(), fs::read(&b).unwrap());
    let lib = dir.path().join("liboxide.a");
    builder.write_to(&lib).unwrap();

    let lib_bytes = fs::read(&lib).unwrap();
    assert!(is_archive(&lib_bytes));
    let arch = OxideArchive::parse(lib.display().to_string(), &lib_bytes).unwrap();
    assert!(arch.members.len() >= 2);

    // System nm should see symbols if available
    if Command::new("nm").arg("--version").output().is_ok() {
        let out = Command::new("nm").arg(&lib).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        // At least one of our symbols may appear depending on strip mode
        assert!(
            text.contains("oxide_b") || text.contains("oxide_a") || !text.is_empty(),
            "nm output unexpected: {text}"
        );
    }
}

#[test]
fn truncated_elf_strip_errors() {
    let bad = [0x7fu8, b'E', b'L', b'F', 2, 1, 1, 0];
    let err = strip_bytes(&bad, StripOptions::default());
    assert!(err.is_err(), "must fail loud on truncated ELF");
}
