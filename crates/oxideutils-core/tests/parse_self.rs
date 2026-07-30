//! Smoke: parse a known ELF if /bin/ls exists.

use oxideutils_core::format::object::OxideObject;
use oxideutils_core::utils::read_file;
use std::path::Path;

#[test]
fn parse_ls_if_present() {
    let p = Path::new("/bin/ls");
    if !p.exists() {
        return;
    }
    let data = read_file(p).expect("read ls");
    let obj = OxideObject::parse(p.display(), &data).expect("parse ls");
    assert!(obj.format_name() == "elf" || !obj.format_name().is_empty());
    let secs = obj.section_views().unwrap();
    assert!(!secs.is_empty());
}
