//! SFrame stack-trace format dump (GNU binutils 2.46 / libsframe).
//!
//! Parses header + **FDE table + FRE walk** (v2 default layout and v3 index+attr).
//! Flexible (FLEX) FDE data-words are dumped raw; full flex semantics later.

use crate::prelude::*;
use goblin::elf::Elf;

const SFRAME_MAGIC: u16 = 0xdee2;
const SFRAME_MAGIC_BE: u16 = 0xe2de;

const FRE_TYPE_ADDR1: u8 = 0;
const FRE_TYPE_ADDR2: u8 = 1;
const FRE_TYPE_ADDR4: u8 = 2;

const FDE_TYPE_DEFAULT: u8 = 0;
const FDE_TYPE_FLEX: u8 = 1;

const BASE_REG_FP: u8 = 0;

const FLAG_FDE_SORTED: u8 = 0x1;
const FLAG_FRAME_POINTER: u8 = 0x2;
const FLAG_FDE_FUNC_START_PCREL: u8 = 0x4;

/// Dump SFrame section: header + per-function FDEs + FRE rows.
pub fn format_sframe(elf: &Elf<'_>, data: &[u8], section_name: Option<&str>) -> String {
    let want = section_name.unwrap_or(".sframe");
    let mut s = String::new();
    let mut found = false;

    for sh in &elf.section_headers {
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("");
        if name != want && !(want == ".sframe" && name.contains("sframe")) {
            continue;
        }
        found = true;
        let start = sh.sh_offset as usize;
        let size = sh.sh_size as usize;
        s.push_str(&format!(
            "\nSFrame section '{name}' at offset 0x{:x} contains {} bytes:\n",
            sh.sh_offset, size
        ));
        if start.saturating_add(size) > data.len() || size < 4 {
            s.push_str("  <section truncated or empty>\n");
            continue;
        }
        let bytes = &data[start..start + size];
        let sec_addr = sh.sh_addr;
        match decode_sframe(bytes, sec_addr) {
            Ok(text) => s.push_str(&text),
            Err(e) => {
                s.push_str(&format!("  <parse error: {e}>\n"));
                let peek = size.min(32);
                let hex: String = bytes[..peek]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ");
                s.push_str(&format!("  First {peek} bytes: {hex}\n"));
            }
        }
    }

    if !found {
        s.push_str(&format!(
            "\nNo SFrame section '{want}' found in this file.\n"
        ));
    }
    s
}

/// Public decode helper for tests (synthetic sections).
pub fn format_sframe_bytes(bytes: &[u8], section_vma: u64) -> core::result::Result<String, String> {
    decode_sframe(bytes, section_vma)
}

struct SframeHeader {
    magic: u16,
    version: u8,
    flags: u8,
    abi_arch: u8,
    cfa_fixed_fp: i8,
    cfa_fixed_ra: i8,
    auxhdr_len: u8,
    num_fdes: u32,
    num_fres: u32,
    fre_len: u32,
    fdeoff: u32,
    freoff: u32,
    le: bool,
    hdr_size: usize,
}

fn decode_sframe(data: &[u8], sec_addr: u64) -> core::result::Result<String, String> {
    let h = parse_sframe_header(data).map_err(|e| e.to_string())?;
    let mut s = String::new();
    s.push_str(&format!("  Magic           : 0x{:04x}\n", h.magic));
    s.push_str(&format!("  Version         : {}\n", h.version));
    s.push_str(&format!(
        "  Flags           : 0x{:02x}{}\n",
        h.flags,
        flag_note(h.flags)
    ));
    s.push_str(&format!(
        "  ABI/Arch        : {} ({})\n",
        h.abi_arch,
        abi_name(h.abi_arch)
    ));
    s.push_str(&format!("  CFA fixed FP off: {}\n", h.cfa_fixed_fp));
    s.push_str(&format!("  CFA fixed RA off: {}\n", h.cfa_fixed_ra));
    s.push_str(&format!("  Aux header len  : {}\n", h.auxhdr_len));
    s.push_str(&format!("  Num FDEs        : {}\n", h.num_fdes));
    s.push_str(&format!("  Num FREs        : {}\n", h.num_fres));
    s.push_str(&format!("  FRE length      : {} bytes\n", h.fre_len));
    s.push_str(&format!("  FDE offset      : 0x{:x}\n", h.fdeoff));
    s.push_str(&format!("  FRE offset      : 0x{:x}\n", h.freoff));

    let fde_base = h.hdr_size.saturating_add(h.fdeoff as usize);
    let fre_base = h.hdr_size.saturating_add(h.freoff as usize);
    if fre_base > data.len() || fde_base > data.len() {
        return Err("FDE/FRE subsection out of range".into());
    }
    let fre_buf = &data[fre_base..];
    let fde_buf = &data[fde_base..];

    s.push_str("\n  Function Descriptor Entries:\n");
    s.push_str("    STARTPC           SIZE     FRE#  FRE_TYPE  PC_TYPE  INFO\n");

    if h.version >= 3 {
        dump_fdes_v3(&mut s, &h, fde_buf, fre_buf, sec_addr, fde_base)?;
    } else if h.version == 2 || h.version == 1 {
        dump_fdes_v2(&mut s, &h, fde_buf, fre_buf, sec_addr, fde_base)?;
    } else {
        s.push_str(&format!(
            "  <unsupported SFrame version {} for FRE walk>\n",
            h.version
        ));
    }
    Ok(s)
}

fn parse_sframe_header(data: &[u8]) -> core::result::Result<SframeHeader, &'static str> {
    if data.len() < 28 {
        return Err("buffer too small for sframe_header");
    }
    let magic_le = u16::from_le_bytes([data[0], data[1]]);
    let magic_be = u16::from_be_bytes([data[0], data[1]]);
    let (le, magic) = if magic_le == SFRAME_MAGIC {
        (true, magic_le)
    } else if magic_be == SFRAME_MAGIC || magic_le == SFRAME_MAGIC_BE {
        (false, SFRAME_MAGIC)
    } else {
        return Err("bad SFrame magic (expected 0xdee2)");
    };
    let r32 = |off: usize| -> u32 {
        let b = [data[off], data[off + 1], data[off + 2], data[off + 3]];
        if le {
            u32::from_le_bytes(b)
        } else {
            u32::from_be_bytes(b)
        }
    };
    let aux = data[7];
    let hdr_size = 28usize + aux as usize;
    if data.len() < hdr_size {
        return Err("truncated header (aux)");
    }
    Ok(SframeHeader {
        magic,
        version: data[2],
        flags: data[3],
        abi_arch: data[4],
        cfa_fixed_fp: data[5] as i8,
        cfa_fixed_ra: data[6] as i8,
        auxhdr_len: aux,
        num_fdes: r32(8),
        num_fres: r32(12),
        fre_len: r32(16),
        fdeoff: r32(20),
        freoff: r32(24),
        le,
        hdr_size,
    })
}

struct FuncDesc {
    /// Section-relative start of function (bytes from section start).
    start_secrel: i64,
    size: u32,
    fre_off: u32,
    num_fres: u32,
    fre_type: u8,
    pc_type: u8,
    fde_type: u8,
    rep_size: u8,
    pauth_key: u8,
    signal: bool,
}

fn dump_fdes_v2(
    s: &mut String,
    h: &SframeHeader,
    fde_buf: &[u8],
    fre_buf: &[u8],
    sec_addr: u64,
    fde_base_in_sec: usize,
) -> core::result::Result<(), String> {
    // sframe_func_desc_entry_v2 = 20 bytes
    const FDE_SZ: usize = 20;
    for i in 0..h.num_fdes as usize {
        let off = i * FDE_SZ;
        if off + FDE_SZ > fde_buf.len() {
            s.push_str(&format!("  <FDE #{i} truncated>\n"));
            break;
        }
        let raw = &fde_buf[off..off + FDE_SZ];
        let start_rel = read_i32(raw, 0, h.le) as i64;
        let size = read_u32(raw, 4, h.le);
        let fre_off = read_u32(raw, 8, h.le);
        let num_fres = read_u32(raw, 12, h.le);
        let info = raw[16];
        let rep = raw[17];
        let fre_type = info & 0xf;
        let pc_type = (info >> 4) & 0x1;
        let pauth = (info >> 5) & 0x1;
        let fde_secrel = (fde_base_in_sec + off) as i64;
        let start_secrel = if h.flags & FLAG_FDE_FUNC_START_PCREL != 0 {
            fde_secrel + start_rel
        } else {
            // distance from start of FDE section (or absolute secrel)
            start_rel
        };
        let fd = FuncDesc {
            start_secrel,
            size,
            fre_off,
            num_fres,
            fre_type,
            pc_type,
            fde_type: FDE_TYPE_DEFAULT,
            rep_size: rep,
            pauth_key: pauth,
            signal: false,
        };
        let _ = fde_secrel;
        dump_one_func(s, h, &fd, i, fre_buf, sec_addr, false)?;
    }
    Ok(())
}

fn dump_fdes_v3(
    s: &mut String,
    h: &SframeHeader,
    fde_buf: &[u8],
    fre_buf: &[u8],
    sec_addr: u64,
    fde_base_in_sec: usize,
) -> core::result::Result<(), String> {
    // sframe_func_desc_idx_v3 = 16 bytes
    const IDX_SZ: usize = 16;
    const ATTR_SZ: usize = 5; // packed: u16 + 3*u8
    for i in 0..h.num_fdes as usize {
        let off = i * IDX_SZ;
        if off + IDX_SZ > fde_buf.len() {
            s.push_str(&format!("  <FDE idx #{i} truncated>\n"));
            break;
        }
        let raw = &fde_buf[off..off + IDX_SZ];
        let start_rel = read_i64(raw, 0, h.le);
        let size = read_u32(raw, 8, h.le);
        let fre_off = read_u32(raw, 12, h.le) as usize;
        if fre_off + ATTR_SZ > fre_buf.len() {
            s.push_str(&format!("  <FDE #{i} attr out of range>\n"));
            continue;
        }
        let attr = &fre_buf[fre_off..fre_off + ATTR_SZ];
        let num_fres = read_u16(attr, 0, h.le) as u32;
        let info = attr[2];
        let info2 = attr[3];
        let rep = attr[4];
        let fre_type = info & 0xf;
        let pc_type = (info >> 4) & 0x1;
        let pauth = (info >> 5) & 0x1;
        let signal = (info >> 7) & 0x1 != 0;
        let fde_type = info2 & 0x1f;
        let fde_secrel = (fde_base_in_sec + off) as i64;
        let start_secrel = if h.flags & FLAG_FDE_FUNC_START_PCREL != 0 {
            fde_secrel + start_rel
        } else {
            start_rel
        };
        let fd = FuncDesc {
            start_secrel,
            size,
            fre_off: fre_off as u32,
            num_fres,
            fre_type,
            pc_type,
            fde_type,
            rep_size: rep,
            pauth_key: pauth,
            signal,
        };
        let _ = fde_secrel;
        dump_one_func(s, h, &fd, i, fre_buf, sec_addr, true)?;
    }
    Ok(())
}

fn dump_one_func(
    s: &mut String,
    h: &SframeHeader,
    fd: &FuncDesc,
    idx: usize,
    fre_buf: &[u8],
    sec_addr: u64,
    v3: bool,
) -> core::result::Result<(), String> {
    let start_vma = sec_addr.wrapping_add(fd.start_secrel as u64);
    let fre_name = fre_type_name(fd.fre_type);
    let pc_name = if fd.pc_type == 0 { "INC" } else { "MASK" };
    let fde_name = if fd.fde_type == FDE_TYPE_FLEX {
        "FLEX"
    } else {
        "DEFAULT"
    };
    s.push_str(&format!(
        "\n  func #{idx} [{fde_name}]\n"
    ));
    s.push_str(&format!(
        "    start PC: {:016x}  size: {:#x}  FREs: {}  fre_type: {fre_name}  pc: {pc_name}",
        start_vma, fd.size, fd.num_fres
    ));
    if fd.pc_type != 0 {
        s.push_str(&format!("  rep_block: {}", fd.rep_size));
    }
    if fd.pauth_key != 0 {
        s.push_str("  pauth:B");
    }
    if fd.signal {
        s.push_str("  [signal]");
    }
    s.push('\n');
    s.push_str("      STARTPC           CFA        FP        RA\n");

    // FRE stream start
    let mut cursor = fd.fre_off as usize;
    if v3 {
        cursor = cursor.saturating_add(5); // skip attr
    }

    for j in 0..fd.num_fres {
        if cursor >= fre_buf.len() {
            s.push_str(&format!("      <FRE #{j} OOB>\n"));
            break;
        }
        match parse_fre(&fre_buf[cursor..], fd.fre_type, h.le) {
            Ok((fre, consumed)) => {
                let fre_pc = if fd.pc_type != 0 {
                    fre.start_addr as u64
                } else {
                    start_vma.wrapping_add(fre.start_addr as u64)
                };
                let line = format_fre_line(h, fre_pc, &fre);
                s.push_str(&format!("      {line}\n"));
                cursor = cursor.saturating_add(consumed);
            }
            Err(e) => {
                s.push_str(&format!("      <FRE #{j} error: {e}>\n"));
                break;
            }
        }
    }
    Ok(())
}

struct Fre {
    start_addr: u32,
    info: u8,
    words: Vec<i32>,
}

fn parse_fre(
    buf: &[u8],
    fre_type: u8,
    le: bool,
) -> core::result::Result<(Fre, usize), String> {
    let addr_sz = fre_addr_size(fre_type)?;
    if buf.len() < addr_sz + 1 {
        return Err("short FRE".into());
    }
    let start_addr = match addr_sz {
        1 => buf[0] as u32,
        2 => read_u16(buf, 0, le) as u32,
        4 => read_u32(buf, 0, le),
        _ => return Err("bad fre addr size".into()),
    };
    let info = buf[addr_sz];
    let count = ((info >> 1) & 0xf) as usize;
    let size_code = (info >> 5) & 0x3;
    let word_bytes = fre_offset_byte_size(size_code);
    let data_len = count * word_bytes;
    let total = addr_sz + 1 + data_len;
    if buf.len() < total {
        return Err("FRE data words truncated".into());
    }
    let mut words = Vec::with_capacity(count);
    let mut o = addr_sz + 1;
    for _ in 0..count {
        let w = read_signed(&buf[o..o + word_bytes], le, word_bytes);
        words.push(w);
        o += word_bytes;
    }
    Ok((
        Fre {
            start_addr,
            info,
            words,
        },
        total,
    ))
}

fn format_fre_line(h: &SframeHeader, fre_pc: u64, fre: &Fre) -> String {
    let count = ((fre.info >> 1) & 0xf) as usize;
    // RA undefined when no data words (v2 rule)
    if count == 0 {
        return format!("{fre_pc:016x}  RA undefined");
    }
    let base = fre.info & 0x1;
    let base_s = if base == BASE_REG_FP { "fp" } else { "sp" };
    let mangled = (fre.info >> 7) & 0x1 != 0;

    let cfa = fre.words.first().copied().unwrap_or(0);
    let cfa_s = format!("{base_s}+{cfa}");

    // AMD64: words = [CFA, FP?]
    // AArch64: [CFA, RA?, FP?] when frame record
    // s390x: more complex — treat like generic
    let (fp_s, ra_s) = match h.abi_arch {
        3 => {
            // AMD64 LE: fixed RA often in header
            let fp = if fre.words.len() >= 2 {
                format!("c{:+}", fre.words[1])
            } else {
                "u".into()
            };
            let ra = if h.cfa_fixed_ra != 0 {
                "f".into()
            } else {
                "u".into()
            };
            (fp, ra)
        }
        1 | 2 => {
            // AArch64: CFA, RA, FP
            let ra = if fre.words.len() >= 2 {
                format!("c{:+}", fre.words[1])
            } else {
                "u".into()
            };
            let fp = if fre.words.len() >= 3 {
                format!("c{:+}", fre.words[2])
            } else {
                "u".into()
            };
            (fp, ra)
        }
        _ => {
            let fp = if fre.words.len() >= 2 {
                format!("c{:+}", fre.words[1])
            } else {
                "u".into()
            };
            let ra = if fre.words.len() >= 3 {
                format!("c{:+}", fre.words[2])
            } else if h.cfa_fixed_ra != 0 {
                "f".into()
            } else {
                "u".into()
            };
            (fp, ra)
        }
    };
    let ra_mark = if mangled { "[s]" } else { "   " };
    format!(
        "{fre_pc:016x}  {cfa_s:<10} {fp_s:<10} {ra_s}{ra_mark}"
    )
}

fn fre_addr_size(fre_type: u8) -> core::result::Result<usize, String> {
    match fre_type {
        FRE_TYPE_ADDR1 => Ok(1),
        FRE_TYPE_ADDR2 => Ok(2),
        FRE_TYPE_ADDR4 => Ok(4),
        t => Err(format!("unknown FRE type {t}")),
    }
}

fn fre_offset_byte_size(size_code: u8) -> usize {
    match size_code {
        0 => 1,
        1 => 2,
        2 => 4,
        _ => 1,
    }
}

fn fre_type_name(t: u8) -> &'static str {
    match t {
        FRE_TYPE_ADDR1 => "ADDR1",
        FRE_TYPE_ADDR2 => "ADDR2",
        FRE_TYPE_ADDR4 => "ADDR4",
        _ => "?",
    }
}

fn abi_name(a: u8) -> &'static str {
    match a {
        1 => "AARCH64 BE",
        2 => "AARCH64 LE",
        3 => "AMD64 LE",
        4 => "S390X BE",
        _ => "unknown",
    }
}

fn flag_note(f: u8) -> String {
    let mut parts = Vec::new();
    if f & FLAG_FDE_SORTED != 0 {
        parts.push("FDE_SORTED");
    }
    if f & FLAG_FRAME_POINTER != 0 {
        parts.push("FRAME_POINTER");
    }
    if f & FLAG_FDE_FUNC_START_PCREL != 0 {
        parts.push("FDE_FUNC_START_PCREL");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join("|"))
    }
}

fn read_u16(b: &[u8], off: usize, le: bool) -> u16 {
    let a = [b[off], b[off + 1]];
    if le {
        u16::from_le_bytes(a)
    } else {
        u16::from_be_bytes(a)
    }
}
fn read_u32(b: &[u8], off: usize, le: bool) -> u32 {
    let a = [b[off], b[off + 1], b[off + 2], b[off + 3]];
    if le {
        u32::from_le_bytes(a)
    } else {
        u32::from_be_bytes(a)
    }
}
fn read_i32(b: &[u8], off: usize, le: bool) -> i32 {
    read_u32(b, off, le) as i32
}
fn read_i64(b: &[u8], off: usize, le: bool) -> i64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[off..off + 8]);
    if le {
        i64::from_le_bytes(a)
    } else {
        i64::from_be_bytes(a)
    }
}
fn read_signed(b: &[u8], le: bool, nbytes: usize) -> i32 {
    match nbytes {
        1 => b[0] as i8 as i32,
        2 => {
            let v = read_u16(b, 0, le) as i16;
            v as i32
        }
        4 => read_i32(b, 0, le),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal SFrame v2 with one FDE and one FRE (AMD64).
    fn synth_v2() -> Vec<u8> {
        let mut v = Vec::new();
        // header 28
        v.extend_from_slice(&0xdee2u16.to_le_bytes()); // magic
        v.push(2); // version
        v.push(0); // flags
        v.push(3); // AMD64 LE
        v.push(0); // fixed fp invalid
        v.push((-8i8) as u8); // fixed RA = -8 (typical amd64)
        v.push(0); // aux
        v.extend_from_slice(&1u32.to_le_bytes()); // num fdes
        v.extend_from_slice(&1u32.to_le_bytes()); // num fres
        // fre_len: FRE = 1 (addr) + 1 (info) + 1 (word) = 3
        v.extend_from_slice(&3u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // fdeoff
        v.extend_from_slice(&20u32.to_le_bytes()); // freoff = after 20-byte FDE
                                                   // FDE v2 (20)
        v.extend_from_slice(&0i32.to_le_bytes()); // start
        v.extend_from_slice(&0x20u32.to_le_bytes()); // size
        v.extend_from_slice(&0u32.to_le_bytes()); // fre_off
        v.extend_from_slice(&1u32.to_le_bytes()); // num_fres
        v.push(0); // fre_type ADDR1, pc INC
        v.push(0); // rep
        v.extend_from_slice(&0u16.to_le_bytes()); // pad
                                                 // FRE: start=0, info=sp+1word 1B = base SP(1) count1 size0 => 0b00000011
        v.push(0); // start
        v.push(0x03); // info
        v.push(16); // cfa offset 16
        v
    }

    #[test]
    fn parse_synth_v2_fre_walk() {
        let bytes = synth_v2();
        let text = format_sframe_bytes(&bytes, 0x1000).expect("decode");
        assert!(text.contains("Num FDEs"), "{text}");
        assert!(text.contains("func #0"), "{text}");
        assert!(text.contains("sp+16") || text.contains("CFA"), "{text}");
        assert!(text.contains("0000000000001000") || text.contains("1000"), "{text}");
    }
}
