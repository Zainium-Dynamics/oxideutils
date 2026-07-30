//! x86_64 AT&T encoder for oxide-as (gas/tc-i386 subset).
//!
//! Emits machine code plus pending ELF relocations for symbol operands,
//! matching gas behaviour for:
//!   call/jmp/jcc → R_X86_64_PLT32 / R_X86_64_PC32 (PC-relative rel32)
//!   movabs $sym, %reg → R_X86_64_64
//!   lea sym(%rip), %reg → R_X86_64_PC32
//!   mov $sym, %reg (32-bit) → R_X86_64_32

use anyhow::{Result, bail};

/// Pending relocation relative to the start of the encoded instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingReloc {
    /// Byte offset within `EncodedInsn::bytes` where the reloc field starts.
    pub offset: u8,
    /// Symbol name (may be defined later in the same unit or left undefined).
    pub symbol: String,
    pub kind: RelocKind,
    /// Explicit addend written at the place (gas often uses -4 for PC32).
    pub addend: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocKind {
    /// R_X86_64_PC32 — S + A - P
    Pc32,
    /// R_X86_64_PLT32 — L + A - P (treat like PC32 for static links)
    Plt32,
    /// R_X86_64_32 — S + A (32-bit absolute)
    Abs32,
    /// R_X86_64_64 — S + A (64-bit absolute)
    Abs64,
    /// R_X86_64_32S — signed 32-bit absolute. Not yet emitted by any
    /// encoder path (no AT&T operand form here needs the signed variant
    /// over plain `Abs32`), kept for API completeness against the real
    /// ELF reloc type and matched exhaustively in `main.rs`.
    #[allow(dead_code)]
    Abs32S,
    /// R_X86_64_TPOFF32 — local-exec TLS: tpoff(S) + A, a plain 32-bit
    /// absolute write (no GOT, no PC-relative component).
    TpOff32,
    /// R_X86_64_GOTTPOFF — initial-exec TLS: G + GOTPLT + A - P (same
    /// PC-relative math as `Pc32`, just pointing at a GOT slot that holds
    /// the precomputed tpoff value rather than at the symbol itself).
    GotTpOff,
}

/// Split a known `@SUFFIX` TLS relocation marker off a symbol name, if
/// present (gas `tc-i386.c`'s `operand_type_check (..., @tpoff/@gottpoff)`
/// at a reduced scale — just the two forms Phase 1 needs).
fn split_tls_suffix(name: &str) -> (&str, Option<RelocKind>) {
    if let Some(base) = name.strip_suffix("@gottpoff") {
        (base, Some(RelocKind::GotTpOff))
    } else if let Some(base) = name.strip_suffix("@tpoff") {
        (base, Some(RelocKind::TpOff32))
    } else {
        (name, None)
    }
}

#[derive(Debug, Clone)]
pub struct EncodedInsn {
    pub bytes: Vec<u8>,
    pub relocs: Vec<PendingReloc>,
}

impl EncodedInsn {
    fn raw(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            relocs: Vec::new(),
        }
    }
    fn with_reloc(bytes: Vec<u8>, reloc: PendingReloc) -> Self {
        Self {
            bytes,
            relocs: vec![reloc],
        }
    }
}

/// Encode one AT&T instruction.
pub fn encode_insn(mnemonic: &str, operands: &[String]) -> Result<EncodedInsn> {
    let m = strip_size_suffix(mnemonic);
    let size = insn_size(mnemonic);

    match mnemonic {
        "ret" | "retq" | "retl" => return Ok(EncodedInsn::raw(vec![0xc3])),
        "nop" => return Ok(EncodedInsn::raw(vec![0x90])),
        "leave" | "leaveq" => return Ok(EncodedInsn::raw(vec![0xc9])),
        "syscall" => return Ok(EncodedInsn::raw(vec![0x0f, 0x05])),
        "sysenter" => return Ok(EncodedInsn::raw(vec![0x0f, 0x34])),
        "sysexit" => return Ok(EncodedInsn::raw(vec![0x0f, 0x35])),
        "hlt" => return Ok(EncodedInsn::raw(vec![0xf4])),
        "ud2" => return Ok(EncodedInsn::raw(vec![0x0f, 0x0b])),
        "cpuid" => return Ok(EncodedInsn::raw(vec![0x0f, 0xa2])),
        "rdtsc" => return Ok(EncodedInsn::raw(vec![0x0f, 0x31])),
        "rdtscp" => return Ok(EncodedInsn::raw(vec![0x0f, 0x01, 0xf9])),
        "pause" => return Ok(EncodedInsn::raw(vec![0xf3, 0x90])),
        "cdq" | "cltd" => return Ok(EncodedInsn::raw(vec![0x99])),
        "cqo" | "cqto" => return Ok(EncodedInsn::raw(vec![0x48, 0x99])),
        "cwde" | "cwtl" => return Ok(EncodedInsn::raw(vec![0x98])),
        "cdqe" | "cltq" => return Ok(EncodedInsn::raw(vec![0x48, 0x98])),
        "clc" => return Ok(EncodedInsn::raw(vec![0xf8])),
        "stc" => return Ok(EncodedInsn::raw(vec![0xf9])),
        "cmc" => return Ok(EncodedInsn::raw(vec![0xf5])),
        "cli" => return Ok(EncodedInsn::raw(vec![0xfa])),
        "sti" => return Ok(EncodedInsn::raw(vec![0xfb])),
        "cld" => return Ok(EncodedInsn::raw(vec![0xfc])),
        "std" => return Ok(EncodedInsn::raw(vec![0xfd])),
        "lahf" => return Ok(EncodedInsn::raw(vec![0x9f])),
        "sahf" => return Ok(EncodedInsn::raw(vec![0x9e])),
        "pushf" | "pushfq" => return Ok(EncodedInsn::raw(vec![0x9c])),
        "popf" | "popfq" => return Ok(EncodedInsn::raw(vec![0x9d])),
        "endbr64" => return Ok(EncodedInsn::raw(vec![0xf3, 0x0f, 0x1e, 0xfa])),
        "endbr32" => return Ok(EncodedInsn::raw(vec![0xf3, 0x0f, 0x1e, 0xfb])),
        "lfence" => return Ok(EncodedInsn::raw(vec![0x0f, 0xae, 0xe8])),
        "sfence" => return Ok(EncodedInsn::raw(vec![0x0f, 0xae, 0xf8])),
        "mfence" => return Ok(EncodedInsn::raw(vec![0x0f, 0xae, 0xf0])),
        _ => {}
    }

    // Conditional jumps: je/jz, jne/jnz, ja, jb, ...
    if let Some(cc) = jcc_opcode(mnemonic) {
        return encode_jcc(cc, operands);
    }

    // SSE/SSE2 scalar float + a few packed ops (movaps/movapd/xorps/xorpd
    // for the zeroing/reg-move idioms). These are matched on the *raw*
    // mnemonic, not `m` (strip_size_suffix output): most of them are
    // fixed-width by name already (movss/addsd/... never take a b/w/l/q
    // suffix), and the four that do allow one (cvtsi2sd/cvtsi2ss/
    // cvttsd2si/cvttss2si) need to tell "no suffix" apart from "explicit
    // q suffix" to get REX.W right — strip_size_suffix would collapse
    // "cvtsi2sdq" down to "cvtsi2sd" and lose exactly that distinction.
    match mnemonic {
        "movss" => return encode_sse_movlike(operands, Some(0xf3), 0x10, 0x11),
        "movsd" => return encode_sse_movlike(operands, Some(0xf2), 0x10, 0x11),
        "movaps" => return encode_sse_movlike(operands, None, 0x28, 0x29),
        "movapd" => return encode_sse_movlike(operands, Some(0x66), 0x28, 0x29),
        "movups" => return encode_sse_movlike(operands, None, 0x10, 0x11),
        "movupd" => return encode_sse_movlike(operands, Some(0x66), 0x10, 0x11),
        "addss" => return encode_sse_alu(operands, Some(0xf3), 0x58),
        "addsd" => return encode_sse_alu(operands, Some(0xf2), 0x58),
        "subss" => return encode_sse_alu(operands, Some(0xf3), 0x5c),
        "subsd" => return encode_sse_alu(operands, Some(0xf2), 0x5c),
        "mulss" => return encode_sse_alu(operands, Some(0xf3), 0x59),
        "mulsd" => return encode_sse_alu(operands, Some(0xf2), 0x59),
        "divss" => return encode_sse_alu(operands, Some(0xf3), 0x5e),
        "divsd" => return encode_sse_alu(operands, Some(0xf2), 0x5e),
        "xorps" => return encode_sse_alu(operands, None, 0x57),
        "xorpd" => return encode_sse_alu(operands, Some(0x66), 0x57),
        "ucomiss" => return encode_sse_alu(operands, None, 0x2e),
        "ucomisd" => return encode_sse_alu(operands, Some(0x66), 0x2e),
        "cvtsi2sd" => return encode_cvtsi2s(operands, 0xf2, None),
        "cvtsi2sdl" => return encode_cvtsi2s(operands, 0xf2, Some(false)),
        "cvtsi2sdq" => return encode_cvtsi2s(operands, 0xf2, Some(true)),
        "cvtsi2ss" => return encode_cvtsi2s(operands, 0xf3, None),
        "cvtsi2ssl" => return encode_cvtsi2s(operands, 0xf3, Some(false)),
        "cvtsi2ssq" => return encode_cvtsi2s(operands, 0xf3, Some(true)),
        "cvttsd2si" => return encode_cvttx2si(operands, 0xf2, None),
        "cvttsd2sil" => return encode_cvttx2si(operands, 0xf2, Some(false)),
        "cvttsd2siq" => return encode_cvttx2si(operands, 0xf2, Some(true)),
        "cvttss2si" => return encode_cvttx2si(operands, 0xf3, None),
        "cvttss2sil" => return encode_cvttx2si(operands, 0xf3, Some(false)),
        "cvttss2siq" => return encode_cvttx2si(operands, 0xf3, Some(true)),
        _ => {}
    }

    match m {
        "push" => encode_push(operands, size),
        "pop" => encode_pop(operands, size),
        "mov" => encode_mov(operands, size),
        "lea" => encode_lea(operands, size),
        "xor" => encode_alu(operands, size, 0x31, 0x81, 6),
        "add" => encode_alu(operands, size, 0x01, 0x81, 0),
        "sub" => encode_alu(operands, size, 0x29, 0x81, 5),
        "and" => encode_alu(operands, size, 0x21, 0x81, 4),
        "or" => encode_alu(operands, size, 0x09, 0x81, 1),
        "cmp" => encode_alu(operands, size, 0x39, 0x81, 7),
        "test" => encode_test(operands, size),
        "adc" => encode_alu(operands, size, 0x11, 0x81, 2),
        "sbb" => encode_alu(operands, size, 0x19, 0x81, 3),
        "xchg" => encode_xchg(operands, size),
        "imul" => encode_imul(operands, size),
        "mul" => encode_unary_group(operands, size, 4, 0xf7),
        "div" => encode_unary_group(operands, size, 6, 0xf7),
        "idiv" => encode_unary_group(operands, size, 7, 0xf7),
        "inc" => encode_unary_group(operands, size, 0, 0xff),
        "dec" => encode_unary_group(operands, size, 1, 0xff),
        "not" => encode_unary_group(operands, size, 2, 0xf7),
        "neg" => encode_unary_group(operands, size, 3, 0xf7),
        "shl" | "sal" => encode_shift(operands, size, 4),
        "shr" => encode_shift(operands, size, 5),
        "sar" => encode_shift(operands, size, 7),
        "rol" => encode_shift(operands, size, 0),
        "ror" => encode_shift(operands, size, 1),
        "call" => encode_call_jmp(operands, true),
        "jmp" => encode_call_jmp(operands, false),
        "int" => {
            if let Some(imm) = operands.first().and_then(|o| parse_imm(o)) {
                Ok(EncodedInsn::raw(vec![0xcd, imm as u8]))
            } else {
                Ok(EncodedInsn::raw(vec![0xcd, 0x80]))
            }
        }
        "seta" | "setnbe" => encode_setcc(0x97, operands),
        "setae" | "setnb" | "setnc" => encode_setcc(0x93, operands),
        "setb" | "setc" | "setnae" => encode_setcc(0x92, operands),
        "setbe" | "setna" => encode_setcc(0x96, operands),
        "sete" | "setz" => encode_setcc(0x94, operands),
        "setg" | "setnle" => encode_setcc(0x9f, operands),
        "setge" | "setnl" => encode_setcc(0x9d, operands),
        "setl" | "setnge" => encode_setcc(0x9c, operands),
        "setle" | "setng" => encode_setcc(0x9e, operands),
        "setne" | "setnz" => encode_setcc(0x95, operands),
        "setno" => encode_setcc(0x91, operands),
        "setnp" | "setpo" => encode_setcc(0x9b, operands),
        "setns" => encode_setcc(0x99, operands),
        "seto" => encode_setcc(0x90, operands),
        "setp" | "setpe" => encode_setcc(0x9a, operands),
        "sets" => encode_setcc(0x98, operands),
        _ => {
            bail!("oxide-as: unsupported instruction `{mnemonic} {operands:?}`")
        }
    }
}

// Base mnemonics whose own trailing letter happens to collide with a size
// suffix (b/w/l/q) — e.g. "call", "sub", "mul" — so they must never be
// stripped when unsuffixed, or they'd wrongly resolve to a nonsense mnemonic
// ("cal", "su", "mu") and fall through to the unsupported-instruction error.
const NO_STRIP_MNEMONICS: &[&str] = &["call", "sub", "sbb", "mul", "imul", "shl", "sal", "rol"];

fn strip_size_suffix(mnemonic: &str) -> &str {
    // Don't strip from set*/j* that end in letter coincidentally.
    if mnemonic.starts_with("set") || mnemonic.starts_with('j') && mnemonic != "jmp" {
        return mnemonic;
    }
    if NO_STRIP_MNEMONICS.contains(&mnemonic) {
        return mnemonic;
    }
    match mnemonic.chars().last() {
        Some('b' | 'w' | 'l' | 'q') if mnemonic.len() > 2 => &mnemonic[..mnemonic.len() - 1],
        _ => mnemonic,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OpSize {
    B,
    W,
    L,
    Q,
}

fn insn_size(mnemonic: &str) -> OpSize {
    if mnemonic.starts_with("set") {
        return OpSize::B;
    }
    if NO_STRIP_MNEMONICS.contains(&mnemonic) {
        // Unsuffixed mnemonic whose last letter coincidentally looks like a
        // size suffix (e.g. "sub", "mul") — no size info here; default Q.
        return OpSize::Q;
    }
    match mnemonic.chars().last() {
        Some('b') => OpSize::B,
        Some('w') => OpSize::W,
        Some('l') => OpSize::L,
        Some('q') => OpSize::Q,
        _ => OpSize::Q,
    }
}

#[derive(Clone, Copy)]
struct Reg {
    num: u8,
}

fn parse_reg(s: &str) -> Option<Reg> {
    let s = s.trim().trim_start_matches('%');
    let num = match s {
        "al" | "ax" | "eax" | "rax" => 0u8,
        "cl" | "cx" | "ecx" | "rcx" => 1,
        "dl" | "dx" | "edx" | "rdx" => 2,
        "bl" | "bx" | "ebx" | "rbx" => 3,
        "spl" | "sp" | "esp" | "rsp" | "ah" => 4,
        "bpl" | "bp" | "ebp" | "rbp" | "ch" => 5,
        "sil" | "si" | "esi" | "rsi" | "dh" => 6,
        "dil" | "di" | "edi" | "rdi" | "bh" => 7,
        "r8" | "r8d" | "r8w" | "r8b" => 8,
        "r9" | "r9d" | "r9w" | "r9b" => 9,
        "r10" | "r10d" | "r10w" | "r10b" => 10,
        "r11" | "r11d" | "r11w" | "r11b" => 11,
        "r12" | "r12d" | "r12w" | "r12b" => 12,
        "r13" | "r13d" | "r13w" | "r13b" => 13,
        "r14" | "r14d" | "r14w" | "r14b" => 14,
        "r15" | "r15d" | "r15w" | "r15b" => 15,
        _ => return None,
    };
    Some(Reg { num })
}

/// Parse `%xmm0`..`%xmm15` to the same 0-15 numbering scheme `Reg` uses for
/// GPRs, so REX.R/X/B extension math via `rex()` works unchanged for
/// xmm8-15. Deliberately a separate function from `parse_reg` (rather than
/// folding xmm names into it) so `%xmm0` and `%eax` can never be confused
/// for each other in either direction.
fn parse_xmm_reg(s: &str) -> Option<Reg> {
    let s = s.trim().trim_start_matches('%');
    let n = s.strip_prefix("xmm")?;
    let num: u8 = n.parse().ok()?;
    (num <= 15).then_some(Reg { num })
}

/// True if `s` names a 64-bit GPR (`%rax`, `%r8`, ...). Used by
/// cvtsi2sd/cvtsi2ss/cvttsd2si/cvttss2si, which — unlike every other
/// instruction in this file — infer their REX.W bit from the actual GPR
/// operand chosen when no explicit l/q suffix is given (verified against
/// real `as`/`objdump` output: `cvttsd2si %xmm0,%rax` gets REX.W purely
/// because the destination register is %rax, not because of any suffix).
fn gpr_is_64(s: &str) -> bool {
    let s = s.trim().trim_start_matches('%');
    matches!(
        s,
        "rax"
            | "rbx"
            | "rcx"
            | "rdx"
            | "rsi"
            | "rdi"
            | "rbp"
            | "rsp"
            | "r8"
            | "r9"
            | "r10"
            | "r11"
            | "r12"
            | "r13"
            | "r14"
            | "r15"
    )
}

fn parse_imm(s: &str) -> Option<i64> {
    let s = s.trim().trim_start_matches('$');
    if s.is_empty() {
        return None;
    }
    // A numeric literal always starts with a digit or a sign; anything else
    // (bare symbol without $ handled elsewhere; $name is symbol imm) is not
    // an immediate — let the caller fall back to symbol handling.
    let first = s.chars().next().unwrap();
    if first != '-' && !first.is_ascii_digit() {
        return None;
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok().map(|v| v as i64);
    }
    s.parse::<i64>().ok()
}

fn is_symbol_name(s: &str) -> bool {
    let s = s.trim().trim_start_matches('$').trim_start_matches('*');
    if s.is_empty() || s.starts_with('%') {
        return false;
    }
    let first = s.chars().next().unwrap();
    // Numeric-looking operands (immediates) are never symbol names. Checked
    // directly (rather than via parse_imm) to avoid mutual recursion.
    if first.is_ascii_digit() || first == '-' {
        return false;
    }
    (first.is_ascii_alphabetic() || first == '_' || first == '.')
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '@')
}

/// Split a trailing `+N`/`-N` integer offset off a symbol-ish token
/// (`sym+4` -> `("sym", 4)`) — the practically-useful slice of gas's general
/// expression grammar (`gas/expr.c`): plain symbol-plus-constant, which an
/// ELF relocation's addend field already represents exactly, so no general
/// expression evaluator is needed for this common case. Full arithmetic
/// expressions remain unsupported (documented gap).
fn split_symbol_addend(text: &str) -> (&str, i64) {
    for (i, c) in text.char_indices().rev() {
        if i == 0 {
            break;
        }
        if c == '+' || c == '-' {
            let (sym, off) = text.split_at(i);
            if let Some(v) = parse_imm(off) {
                return (sym, v);
            }
        }
    }
    (text, 0)
}

/// Returns `(base_symbol_name, addend)` — `addend` is 0 when the operand is
/// a bare symbol.
fn symbol_from_operand(s: &str) -> Option<(String, i64)> {
    let s = s.trim().trim_start_matches('*');
    // $sym(+N)
    if let Some(rest) = s.strip_prefix('$') {
        let (base, addend) = split_symbol_addend(rest);
        if is_symbol_name(base) && parse_imm(s).is_none() {
            return Some((base.to_string(), addend));
        }
        return None;
    }
    // sym(+N)(%rip) or disp(base)
    if let Some(paren) = s.find('(') {
        let disp = s[..paren].trim();
        let (base, addend) = split_symbol_addend(disp);
        if !base.is_empty() && is_symbol_name(base) {
            return Some((base.to_string(), addend));
        }
        return None;
    }
    // bare symbol(+N) (call foo / jmp bar)
    let (base, addend) = split_symbol_addend(s);
    if is_symbol_name(base) && parse_reg(base).is_none() {
        return Some((base.to_string(), addend));
    }
    None
}

fn rex(w: bool, r: u8, x: u8, b: u8) -> Option<u8> {
    if !w && r < 8 && x < 8 && b < 8 {
        return None;
    }
    let mut v = 0x40u8;
    if w {
        v |= 0x08;
    }
    if r >= 8 {
        v |= 0x04;
    }
    if x >= 8 {
        v |= 0x02;
    }
    if b >= 8 {
        v |= 0x01;
    }
    Some(v)
}

fn modrm(mod_: u8, reg: u8, rm: u8) -> u8 {
    ((mod_ & 3) << 6) | ((reg & 7) << 3) | (rm & 7)
}

/// AT&T memory operand: `disp(base,index,scale)`. Any of disp/base/index may
/// be absent (`-4(%rbp)` has no index; `(%rax,%rcx,8)` has no disp; etc).
/// Pure `sym(%rip)` operands are handled separately by the reloc-based
/// RIP-relative path — this struct is only for real base/index registers.
#[derive(Clone, Copy)]
struct MemOperand {
    disp: i32,
    base: Option<Reg>,
    index: Option<Reg>,
    scale: u8,
}

/// Parse `disp(base,index,scale)` / `(base)` / `(base,index)` /
/// `(,index,scale)` etc. Displacement must be a plain integer literal (no
/// symbols) — symbolic addresses go through the RIP-relative reloc path.
/// Returns None if `s` isn't a register-based memory operand at all.
fn parse_mem_operand(s: &str) -> Option<MemOperand> {
    let s = s.trim();
    let paren = s.find('(')?;
    if !s.ends_with(')') {
        return None;
    }
    let disp_s = s[..paren].trim();
    let inside = &s[paren + 1..s.len() - 1];
    let disp = if disp_s.is_empty() {
        0i32
    } else {
        parse_imm(&format!("${disp_s}"))? as i32
    };
    let parts: Vec<&str> = inside.split(',').map(|p| p.trim()).collect();
    let base = parts
        .first()
        .filter(|p| !p.is_empty())
        .and_then(|p| parse_reg(p));
    let index = parts
        .get(1)
        .filter(|p| !p.is_empty())
        .and_then(|p| parse_reg(p));
    let scale = match parts.get(2).map(|p| p.trim()) {
        None | Some("") => 1u8,
        Some(p) => {
            let v: u8 = p.parse().ok()?;
            if !matches!(v, 1 | 2 | 4 | 8) {
                return None;
            }
            v
        }
    };
    if base.is_none() && index.is_none() {
        return None;
    }
    Some(MemOperand {
        disp,
        base,
        index,
        scale,
    })
}

/// Emit the ModRM (+ SIB + displacement) bytes for `mem`, using `reg` as the
/// ModRM.reg field (the other operand — a register, or an opcode-extension
/// group number for unary/immediate forms). Does not emit REX or the opcode
/// byte; callers build those themselves via `rex()` using `mem.index`/`mem.base`.
fn encode_modrm_mem(reg: u8, mem: &MemOperand) -> Vec<u8> {
    let mut out = Vec::new();
    // rsp/r12 (encoding 100 in the base field) always require a SIB byte —
    // rm=100 in a plain ModRM means "SIB follows", it can never address
    // those registers directly. Likewise any index, or no base at all.
    let need_sib =
        mem.index.is_some() || mem.base.is_none() || mem.base.is_some_and(|b| b.num & 7 == 4);
    if need_sib {
        let (base_field, no_base) = match mem.base {
            Some(b) => (b.num & 7, false),
            None => (0b101, true),
        };
        let index_field = match mem.index {
            Some(idx) => idx.num & 7,
            None => 0b100, // no index
        };
        let scale_bits = match mem.scale {
            2 => 0b01,
            4 => 0b10,
            8 => 0b11,
            _ => 0b00,
        };
        let sib = (scale_bits << 6) | (index_field << 3) | base_field;
        if no_base {
            // mod=00, base field=101 with no base register is a dedicated
            // "disp32, no base" escape — disp32 is always emitted, even if 0.
            out.push(modrm(0b00, reg, 0b100));
            out.push(sib);
            out.extend_from_slice(&mem.disp.to_le_bytes());
        } else if base_field == 0b101 && mem.disp == 0 {
            // base=%rbp/%r13 with disp==0 collides with the no-base escape
            // above at mod=00, so force a disp8=0 instead (mod=01).
            out.push(modrm(0b01, reg, 0b100));
            out.push(sib);
            out.push(0);
        } else if mem.disp == 0 {
            out.push(modrm(0b00, reg, 0b100));
            out.push(sib);
        } else if (-128..128).contains(&mem.disp) {
            out.push(modrm(0b01, reg, 0b100));
            out.push(sib);
            out.push(mem.disp as u8);
        } else {
            out.push(modrm(0b10, reg, 0b100));
            out.push(sib);
            out.extend_from_slice(&mem.disp.to_le_bytes());
        }
    } else {
        let base = mem.base.expect("need_sib is false only when base is set");
        let rm = base.num & 7;
        if rm == 0b101 && mem.disp == 0 {
            // base=%rbp/%r13 with disp==0 collides with RIP-relative (mod=00,
            // rm=101), so force a disp8=0 instead (mod=01).
            out.push(modrm(0b01, reg, rm));
            out.push(0);
        } else if mem.disp == 0 {
            out.push(modrm(0b00, reg, rm));
        } else if (-128..128).contains(&mem.disp) {
            out.push(modrm(0b01, reg, rm));
            out.push(mem.disp as u8);
        } else {
            out.push(modrm(0b10, reg, rm));
            out.extend_from_slice(&mem.disp.to_le_bytes());
        }
    }
    out
}

/// REX.X/R.B inputs derived from a memory operand's index/base registers
/// (0 when absent, which never sets a REX bit since `rex()` only reacts to
/// register numbers >= 8).
fn mem_rex_xb(mem: &MemOperand) -> (u8, u8) {
    (
        mem.index.map(|r| r.num).unwrap_or(0),
        mem.base.map(|r| r.num).unwrap_or(0),
    )
}

fn encode_push(operands: &[String], _size: OpSize) -> Result<EncodedInsn> {
    if operands.is_empty() {
        return Ok(EncodedInsn::raw(vec![0x55]));
    }
    let op = &operands[0];
    if let Some(r) = parse_reg(op) {
        let mut out = Vec::new();
        if let Some(rx) = rex(false, 0, 0, r.num) {
            out.push(rx);
        }
        out.push(0x50 + (r.num & 7));
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(imm) = parse_imm(op) {
        if (-128..128).contains(&imm) {
            return Ok(EncodedInsn::raw(vec![0x6a, imm as u8]));
        }
        let mut out = vec![0x68];
        out.extend_from_slice(&(imm as i32).to_le_bytes());
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(mem) = parse_mem_operand(op) {
        // push m64 → FF /6 (always 64-bit in long mode, no REX.W needed).
        let mut out = Vec::new();
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(false, 0, x, b) {
            out.push(rx);
        }
        out.push(0xff);
        out.extend(encode_modrm_mem(6, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction `push {operands:?}`")
}

fn encode_pop(operands: &[String], _size: OpSize) -> Result<EncodedInsn> {
    if operands.is_empty() {
        return Ok(EncodedInsn::raw(vec![0x5d]));
    }
    if let Some(r) = parse_reg(&operands[0]) {
        let mut out = Vec::new();
        if let Some(rx) = rex(false, 0, 0, r.num) {
            out.push(rx);
        }
        out.push(0x58 + (r.num & 7));
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(mem) = parse_mem_operand(&operands[0]) {
        // pop m64 → 8F /0 (always 64-bit in long mode, no REX.W needed).
        let mut out = Vec::new();
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(false, 0, x, b) {
            out.push(rx);
        }
        out.push(0x8f);
        out.extend(encode_modrm_mem(0, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction `pop {operands:?}`")
}

fn encode_mov(operands: &[String], size: OpSize) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction `mov {operands:?}` (needs 2 operands)");
    }
    let src = &operands[0];
    let dst = &operands[1];

    if let Some(rd) = parse_reg(dst) {
        // mov sym(%rip), %reg — load from rip-relative. Checked before the
        // plain symbol/imm case below since symbol_from_operand() also
        // matches "sym(%rip)" (it just extracts the "sym" part), which would
        // otherwise wrongly be treated as `movabs $sym, %reg`.
        if src.contains("(%rip)")
            && let Some((sym, addend)) = symbol_from_operand(src)
        {
            return encode_load_rip(rd, &sym, size, false, addend);
        }
        // mov $imm/sym, %reg
        if let Some((sym, addend)) = symbol_from_operand(src) {
            return encode_mov_sym_imm(rd, &sym, size, addend);
        }
        if let Some(imm) = parse_imm(src) {
            return Ok(EncodedInsn::raw(encode_mov_imm(rd, imm, size)));
        }
        // mov disp(base,index,scale), %reg — general memory load.
        if let Some(mem) = parse_mem_operand(src) {
            return Ok(EncodedInsn::raw(encode_mem_reg_insn(&mem, rd, size, false)));
        }
    }
    if let Some(rs) = parse_reg(src) {
        // mov %reg, sym(%rip)
        if dst.contains("(%rip)")
            && let Some((sym, addend)) = symbol_from_operand(dst)
        {
            return encode_load_rip(rs, &sym, size, true, addend);
        }
        if let Some(rd) = parse_reg(dst) {
            return Ok(EncodedInsn::raw(encode_mov_rr(rs, rd, size)));
        }
        // mov %reg, disp(base,index,scale) — general memory store.
        if let Some(mem) = parse_mem_operand(dst) {
            return Ok(EncodedInsn::raw(encode_mem_reg_insn(&mem, rs, size, true)));
        }
    }
    bail!("oxide-as: unsupported instruction `mov {operands:?}`")
}

/// Shared mov-family mem<->reg encoder (0x88/0x8a byte, 0x89/0x8b word+).
fn encode_mem_reg_insn(mem: &MemOperand, reg: Reg, size: OpSize, is_store: bool) -> Vec<u8> {
    let mut out = Vec::new();
    let w = size == OpSize::Q;
    if size == OpSize::W {
        out.push(0x66);
    }
    let (x, b) = mem_rex_xb(mem);
    if let Some(rx) = rex(w, reg.num, x, b) {
        out.push(rx);
    } else if w {
        out.push(0x48);
    }
    let opcode = match (size, is_store) {
        (OpSize::B, true) => 0x88,
        (OpSize::B, false) => 0x8a,
        (_, true) => 0x89,
        (_, false) => 0x8b,
    };
    out.push(opcode);
    out.extend(encode_modrm_mem(reg.num, mem));
    out
}

fn encode_mov_sym_imm(rd: Reg, sym: &str, size: OpSize, addend: i64) -> Result<EncodedInsn> {
    match size {
        OpSize::Q => {
            // movabs r64, imm64 → R_X86_64_64
            let mut out = vec![rex(true, 0, 0, rd.num).unwrap_or(0x48)];
            out.push(0xb8 + (rd.num & 7));
            let off = out.len() as u8;
            out.extend_from_slice(&0u64.to_le_bytes());
            Ok(EncodedInsn::with_reloc(
                out,
                PendingReloc {
                    offset: off,
                    symbol: sym.to_string(),
                    kind: RelocKind::Abs64,
                    addend,
                },
            ))
        }
        OpSize::L | OpSize::W | OpSize::B => {
            let (sym, tls_kind) = split_tls_suffix(sym);
            let mut out = Vec::new();
            if size == OpSize::W {
                out.push(0x66);
            }
            if let Some(rx) = rex(false, 0, 0, rd.num) {
                out.push(rx);
            }
            out.push(0xb8 + (rd.num & 7));
            let off = out.len() as u8;
            out.extend_from_slice(&0u32.to_le_bytes());
            Ok(EncodedInsn::with_reloc(
                out,
                PendingReloc {
                    offset: off,
                    symbol: sym.to_string(),
                    kind: tls_kind.unwrap_or(RelocKind::Abs32),
                    addend,
                },
            ))
        }
    }
}

fn encode_load_rip(
    reg: Reg,
    sym: &str,
    size: OpSize,
    is_store: bool,
    extra_addend: i64,
) -> Result<EncodedInsn> {
    let (sym, tls_kind) = split_tls_suffix(sym);
    // mov reg/mem with ModRM = [rip+disp32]
    let mut out = Vec::new();
    let w = size == OpSize::Q;
    if size == OpSize::W {
        out.push(0x66);
    }
    if let Some(rx) = rex(w, reg.num, 0, 0) {
        out.push(rx);
    } else if w {
        out.push(0x48);
    }
    // store: 0x89, load: 0x8b (32/64); byte variants 0x88/0x8a
    let opcode = match (size, is_store) {
        (OpSize::B, true) => 0x88,
        (OpSize::B, false) => 0x8a,
        (_, true) => 0x89,
        (_, false) => 0x8b,
    };
    out.push(opcode);
    // mod=00, rm=101 → rip-relative
    out.push(modrm(0b00, reg.num, 0b101));
    let off = out.len() as u8;
    out.extend_from_slice(&0i32.to_le_bytes());
    Ok(EncodedInsn::with_reloc(
        out,
        PendingReloc {
            offset: off,
            symbol: sym.to_string(),
            kind: tls_kind.unwrap_or(RelocKind::Pc32),
            addend: -4 + extra_addend,
        },
    ))
}

fn encode_mov_imm(rd: Reg, imm: i64, size: OpSize) -> Vec<u8> {
    let mut out = Vec::new();
    match size {
        OpSize::B => {
            if let Some(rx) = rex(false, 0, 0, rd.num) {
                out.push(rx);
            }
            out.push(0xb0 + (rd.num & 7));
            out.push(imm as u8);
        }
        OpSize::W => {
            out.push(0x66);
            if let Some(rx) = rex(false, 0, 0, rd.num) {
                out.push(rx);
            }
            out.push(0xb8 + (rd.num & 7));
            out.extend_from_slice(&(imm as u16).to_le_bytes());
        }
        OpSize::L => {
            if let Some(rx) = rex(false, 0, 0, rd.num) {
                out.push(rx);
            }
            out.push(0xb8 + (rd.num & 7));
            out.extend_from_slice(&(imm as u32).to_le_bytes());
        }
        OpSize::Q => {
            if imm >= 0 && imm <= u32::MAX as i64 {
                if let Some(rx) = rex(false, 0, 0, rd.num) {
                    out.push(rx);
                }
                out.push(0xb8 + (rd.num & 7));
                out.extend_from_slice(&(imm as u32).to_le_bytes());
            } else {
                out.push(rex(true, 0, 0, rd.num).unwrap_or(0x48));
                out.push(0xb8 + (rd.num & 7));
                out.extend_from_slice(&(imm as u64).to_le_bytes());
            }
        }
    }
    out
}

fn encode_mov_rr(rs: Reg, rd: Reg, size: OpSize) -> Vec<u8> {
    let mut out = Vec::new();
    let w = size == OpSize::Q;
    if size == OpSize::W {
        out.push(0x66);
    }
    if let Some(rx) = rex(w, rs.num, 0, rd.num) {
        out.push(rx);
    } else if w {
        out.push(0x48);
    }
    out.push(if size == OpSize::B { 0x88 } else { 0x89 });
    out.push(modrm(0b11, rs.num, rd.num));
    out
}

fn encode_lea(operands: &[String], size: OpSize) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction `lea {operands:?}` (needs 2 operands)");
    }
    let mem = &operands[0];
    let Some(rd) = parse_reg(&operands[1]) else {
        bail!("oxide-as: unsupported instruction `lea {operands:?}` (dst must be a register)");
    };
    if mem.contains("(%rip)")
        && let Some((sym, addend)) = symbol_from_operand(mem)
    {
        let mut out = Vec::new();
        let w = size != OpSize::L; // lea usually 64-bit
        if let Some(rx) = rex(w || size == OpSize::Q, rd.num, 0, 0) {
            out.push(rx);
        } else if size == OpSize::Q {
            out.push(0x48);
        }
        out.push(0x8d);
        out.push(modrm(0b00, rd.num, 0b101));
        let off = out.len() as u8;
        out.extend_from_slice(&0i32.to_le_bytes());
        return Ok(EncodedInsn::with_reloc(
            out,
            PendingReloc {
                offset: off,
                symbol: sym,
                kind: RelocKind::Pc32,
                addend: -4 + addend,
            },
        ));
    }
    // lea disp(base,index,scale), %dst — general SIB/base+disp addressing.
    if let Some(mem_op) = parse_mem_operand(mem) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        let (x, b) = mem_rex_xb(&mem_op);
        if let Some(rx) = rex(w, rd.num, x, b) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(0x8d);
        out.extend(encode_modrm_mem(rd.num, &mem_op));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction `lea {operands:?}`")
}

fn encode_alu(
    operands: &[String],
    size: OpSize,
    opcode_rr: u8,
    opcode_imm: u8,
    group: u8,
) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction (alu op) `{operands:?}` (needs 2 operands)");
    }
    let src = &operands[0];
    let dst = &operands[1];

    // op $imm, %reg
    if let (Some(imm), Some(rd)) = (parse_imm(src), parse_reg(dst)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        if let Some(rx) = rex(w, 0, 0, rd.num) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        if (-128..128).contains(&imm) && size != OpSize::B {
            out.push(0x83);
            out.push(modrm(0b11, group, rd.num));
            out.push(imm as u8);
        } else if size == OpSize::B {
            out.push(0x80);
            out.push(modrm(0b11, group, rd.num));
            out.push(imm as u8);
        } else {
            out.push(opcode_imm);
            out.push(modrm(0b11, group, rd.num));
            out.extend_from_slice(&(imm as i32).to_le_bytes());
        }
        return Ok(EncodedInsn::raw(out));
    }

    // op $imm, mem
    if let (Some(imm), Some(mem)) = (parse_imm(src), parse_mem_operand(dst)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, 0, x, b) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        if (-128..128).contains(&imm) && size != OpSize::B {
            out.push(0x83);
            out.extend(encode_modrm_mem(group, &mem));
            out.push(imm as u8);
        } else if size == OpSize::B {
            out.push(0x80);
            out.extend(encode_modrm_mem(group, &mem));
            out.push(imm as u8);
        } else {
            out.push(opcode_imm);
            out.extend(encode_modrm_mem(group, &mem));
            out.extend_from_slice(&(imm as i32).to_le_bytes());
        }
        return Ok(EncodedInsn::raw(out));
    }

    // op %reg, %reg
    if let (Some(rs), Some(rd)) = (parse_reg(src), parse_reg(dst)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        if let Some(rx) = rex(w, rs.num, 0, rd.num) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B {
            opcode_rr.wrapping_sub(1)
        } else {
            opcode_rr
        });
        out.push(modrm(0b11, rs.num, rd.num));
        return Ok(EncodedInsn::raw(out));
    }

    // op %reg, mem — store direction: r/m (mem) is the destination, reg is
    // the source. Opcode family base is "Eb,Gb"/"Ev,Gv" (opcode_rr-1/opcode_rr).
    if let (Some(rs), Some(mem)) = (parse_reg(src), parse_mem_operand(dst)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, rs.num, x, b) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B {
            opcode_rr.wrapping_sub(1)
        } else {
            opcode_rr
        });
        out.extend(encode_modrm_mem(rs.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }

    // mem, %reg — load direction: reg is the destination, r/m (mem) is the
    // source. Opcode family "Gb,Eb"/"Gv,Ev" (opcode_rr+1/opcode_rr+2).
    if let (Some(mem), Some(rd)) = (parse_mem_operand(src), parse_reg(dst)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, rd.num, x, b) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B {
            opcode_rr.wrapping_add(1)
        } else {
            opcode_rr.wrapping_add(2)
        });
        out.extend(encode_modrm_mem(rd.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }

    bail!("oxide-as: unsupported instruction (alu op) `{operands:?}`")
}

fn encode_test(operands: &[String], size: OpSize) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction `test {operands:?}` (needs 2 operands)");
    }
    let a = &operands[0];
    let b = &operands[1];
    // test %r, %r → 0x85
    if let (Some(rs), Some(rd)) = (parse_reg(a), parse_reg(b)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        if let Some(rx) = rex(w, rs.num, 0, rd.num) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B { 0x84 } else { 0x85 });
        out.push(modrm(0b11, rs.num, rd.num));
        return Ok(EncodedInsn::raw(out));
    }
    // test $imm, %reg
    if let (Some(imm), Some(rd)) = (parse_imm(a), parse_reg(b)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if let Some(rx) = rex(w, 0, 0, rd.num) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B { 0xf6 } else { 0xf7 });
        out.push(modrm(0b11, 0, rd.num));
        if size == OpSize::B {
            out.push(imm as u8);
        } else {
            out.extend_from_slice(&(imm as i32).to_le_bytes());
        }
        return Ok(EncodedInsn::raw(out));
    }
    // test $imm, mem
    if let (Some(imm), Some(mem)) = (parse_imm(a), parse_mem_operand(b)) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        let (x, bb) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, 0, x, bb) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B { 0xf6 } else { 0xf7 });
        out.extend(encode_modrm_mem(0, &mem));
        if size == OpSize::B {
            out.push(imm as u8);
        } else {
            out.extend_from_slice(&(imm as i32).to_le_bytes());
        }
        return Ok(EncodedInsn::raw(out));
    }
    // test %reg, mem  /  test mem, %reg — test has no reversed opcode (it
    // never writes back), so both syntactic orders encode identically:
    // r/m=mem, reg=the register, opcode 0x84/0x85.
    let mem_reg = match (parse_reg(a), parse_mem_operand(b)) {
        (Some(r), Some(m)) => Some((r, m)),
        _ => parse_mem_operand(a).and_then(|m| parse_reg(b).map(|r| (r, m))),
    };
    if let Some((reg, mem)) = mem_reg {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        let (x, bb) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, reg.num, x, bb) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B { 0x84 } else { 0x85 });
        out.extend(encode_modrm_mem(reg.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction `test {operands:?}`")
}

fn encode_xchg(operands: &[String], size: OpSize) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction `xchg {operands:?}` (needs 2 operands)");
    }
    if let (Some(a), Some(b)) = (parse_reg(&operands[0]), parse_reg(&operands[1])) {
        // xchg rax, r64 has short form
        if size == OpSize::Q && (a.num == 0 || b.num == 0) {
            let other = if a.num == 0 { b.num } else { a.num };
            let mut out = Vec::new();
            if let Some(rx) = rex(true, 0, 0, other) {
                out.push(rx);
            } else {
                out.push(0x48);
            }
            out.push(0x90 + (other & 7));
            return Ok(EncodedInsn::raw(out));
        }
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if let Some(rx) = rex(w, a.num, 0, b.num) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(if size == OpSize::B { 0x86 } else { 0x87 });
        out.push(modrm(0b11, a.num, b.num));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction `xchg {operands:?}`")
}

fn encode_imul(operands: &[String], size: OpSize) -> Result<EncodedInsn> {
    match operands.len() {
        1 => encode_unary_group(operands, size, 5, 0xf7),
        2 => {
            // imul %src, %dst → 0F AF
            if let (Some(rs), Some(rd)) = (parse_reg(&operands[0]), parse_reg(&operands[1])) {
                let mut out = Vec::new();
                let w = size == OpSize::Q;
                if let Some(rx) = rex(w, rd.num, 0, rs.num) {
                    out.push(rx);
                } else if w {
                    out.push(0x48);
                }
                out.extend_from_slice(&[0x0f, 0xaf]);
                out.push(modrm(0b11, rd.num, rs.num));
                return Ok(EncodedInsn::raw(out));
            }
            bail!("oxide-as: unsupported instruction `imul {operands:?}`")
        }
        3 => {
            // imul $imm, %src, %dst
            if let (Some(imm), Some(rs), Some(rd)) = (
                parse_imm(&operands[0]),
                parse_reg(&operands[1]),
                parse_reg(&operands[2]),
            ) {
                let mut out = Vec::new();
                let w = size == OpSize::Q;
                if let Some(rx) = rex(w, rd.num, 0, rs.num) {
                    out.push(rx);
                } else if w {
                    out.push(0x48);
                }
                if (-128..128).contains(&imm) {
                    out.push(0x6b);
                    out.push(modrm(0b11, rd.num, rs.num));
                    out.push(imm as u8);
                } else {
                    out.push(0x69);
                    out.push(modrm(0b11, rd.num, rs.num));
                    out.extend_from_slice(&(imm as i32).to_le_bytes());
                }
                return Ok(EncodedInsn::raw(out));
            }
            bail!("oxide-as: unsupported instruction `imul {operands:?}`")
        }
        _ => bail!("oxide-as: unsupported instruction `imul {operands:?}` (bad operand count)"),
    }
}

fn encode_unary_group(
    operands: &[String],
    size: OpSize,
    group: u8,
    opcode: u8,
) -> Result<EncodedInsn> {
    let Some(op) = operands.first() else {
        bail!("oxide-as: unsupported instruction (unary op) `{operands:?}` (needs 1 operand)");
    };
    let byte_op = if size == OpSize::B && opcode == 0xff {
        0xfe
    } else if size == OpSize::B && opcode == 0xf7 {
        0xf6
    } else {
        opcode
    };
    if let Some(rd) = parse_reg(op) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        if let Some(rx) = rex(w, 0, 0, rd.num) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(byte_op);
        out.push(modrm(0b11, group, rd.num));
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(mem) = parse_mem_operand(op) {
        let mut out = Vec::new();
        let w = size == OpSize::Q;
        if size == OpSize::W {
            out.push(0x66);
        }
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, 0, x, b) {
            out.push(rx);
        } else if w {
            out.push(0x48);
        }
        out.push(byte_op);
        out.extend(encode_modrm_mem(group, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction (unary op) `{operands:?}`")
}

fn encode_shift(operands: &[String], size: OpSize, group: u8) -> Result<EncodedInsn> {
    // shl $n, %reg  or  shl %cl, %reg
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction (shift op) `{operands:?}` (needs 2 operands)");
    }
    let count = &operands[0];
    let Some(rd) = parse_reg(&operands[1]) else {
        bail!(
            "oxide-as: unsupported instruction (shift op) `{operands:?}` (dst must be a register)"
        );
    };
    let mut out = Vec::new();
    let w = size == OpSize::Q;
    if size == OpSize::W {
        out.push(0x66);
    }
    if let Some(rx) = rex(w, 0, 0, rd.num) {
        out.push(rx);
    } else if w {
        out.push(0x48);
    }
    if count == "%cl" || count == "%Cl" {
        out.push(if size == OpSize::B { 0xd2 } else { 0xd3 });
        out.push(modrm(0b11, group, rd.num));
    } else if let Some(imm) = parse_imm(count) {
        if imm == 1 {
            out.push(if size == OpSize::B { 0xd0 } else { 0xd1 });
            out.push(modrm(0b11, group, rd.num));
        } else {
            out.push(if size == OpSize::B { 0xc0 } else { 0xc1 });
            out.push(modrm(0b11, group, rd.num));
            out.push(imm as u8);
        }
    } else {
        bail!("oxide-as: unsupported instruction (shift op) `{operands:?}` (bad count operand)");
    }
    Ok(EncodedInsn::raw(out))
}

fn encode_call_jmp(operands: &[String], is_call: bool) -> Result<EncodedInsn> {
    if let Some(op) = operands.first() {
        let cleaned = op.trim().trim_start_matches('*');
        if let Some(r) = parse_reg(cleaned) {
            let mut out = Vec::new();
            if let Some(rx) = rex(false, 0, 0, r.num) {
                out.push(rx);
            }
            out.push(0xff);
            out.push(modrm(0b11, if is_call { 2 } else { 4 }, r.num));
            return Ok(EncodedInsn::raw(out));
        }
        if let Some((sym, addend)) = symbol_from_operand(op) {
            // call/jmp rel32 with PLT32/PC32 reloc (gas uses PLT32 for globals)
            let mut out = if is_call { vec![0xe8] } else { vec![0xe9] };
            let off = out.len() as u8;
            out.extend_from_slice(&0i32.to_le_bytes());
            return Ok(EncodedInsn::with_reloc(
                out,
                PendingReloc {
                    offset: off,
                    symbol: sym,
                    kind: if is_call {
                        RelocKind::Plt32
                    } else {
                        RelocKind::Pc32
                    },
                    addend: -4 + addend,
                },
            ));
        }
    }
    bail!(
        "oxide-as: unsupported instruction `{} {operands:?}`",
        if is_call { "call" } else { "jmp" }
    )
}

fn jcc_opcode(mnemonic: &str) -> Option<u8> {
    // Secondary opcode byte for 0F 8x near jcc
    Some(match mnemonic {
        "jo" => 0x80,
        "jno" => 0x81,
        "jb" | "jc" | "jnae" => 0x82,
        "jae" | "jnb" | "jnc" => 0x83,
        "je" | "jz" => 0x84,
        "jne" | "jnz" => 0x85,
        "jbe" | "jna" => 0x86,
        "ja" | "jnbe" => 0x87,
        "js" => 0x88,
        "jns" => 0x89,
        "jp" | "jpe" => 0x8a,
        "jnp" | "jpo" => 0x8b,
        "jl" | "jnge" => 0x8c,
        "jge" | "jnl" => 0x8d,
        "jle" | "jng" => 0x8e,
        "jg" | "jnle" => 0x8f,
        _ => return None,
    })
}

fn encode_jcc(sec_opcode: u8, operands: &[String]) -> Result<EncodedInsn> {
    let (sym, addend) = operands
        .first()
        .and_then(|o| symbol_from_operand(o))
        .unwrap_or_else(|| ("0".into(), 0));
    if sym.chars().all(|c| c.is_ascii_digit() || c == '-') {
        // numeric relative — rare
        return Ok(EncodedInsn::raw(vec![0x0f, sec_opcode, 0, 0, 0, 0]));
    }
    let mut out = vec![0x0f, sec_opcode];
    let off = out.len() as u8;
    out.extend_from_slice(&0i32.to_le_bytes());
    Ok(EncodedInsn::with_reloc(
        out,
        PendingReloc {
            offset: off,
            symbol: sym,
            kind: RelocKind::Pc32,
            addend: -4 + addend,
        },
    ))
}

fn encode_setcc(sec: u8, operands: &[String]) -> Result<EncodedInsn> {
    let rd = operands
        .first()
        .and_then(|o| parse_reg(o))
        .unwrap_or(Reg { num: 0 });
    let mut out = Vec::new();
    if let Some(rx) = rex(false, 0, 0, rd.num) {
        out.push(rx);
    }
    out.extend_from_slice(&[0x0f, sec]);
    out.push(modrm(0b11, 0, rd.num));
    Ok(EncodedInsn::raw(out))
}

/// Shared encoder for movss/movsd/movaps/movapd/movups/movupd: a "mandatory
/// prefix" (F3/F2/66/none) SSE move with a load opcode (mem/xmm → xmm) and a
/// distinct store opcode (xmm → mem). For the xmm,xmm form gas always
/// prefers the load-opcode encoding (verified against `objdump`: `movsd
/// %xmm1, %xmm0` disassembles back as `f2 0f 10 c1`, the *load* opcode, with
/// ModRM.reg = dst and ModRM.rm = src) — there is no reg-reg-only "store"
/// path to pick. REX.W is never used here; the opcode itself fixes the
/// operand width.
fn encode_sse_movlike(
    operands: &[String],
    prefix: Option<u8>,
    load_op: u8,
    store_op: u8,
) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction (sse mov) `{operands:?}` (needs 2 operands)");
    }
    let src = &operands[0];
    let dst = &operands[1];

    if let Some(rd) = parse_xmm_reg(dst) {
        let mut out = Vec::new();
        if let Some(p) = prefix {
            out.push(p);
        }
        if let Some(rs) = parse_xmm_reg(src) {
            // xmm, xmm — load-opcode form, reg=dst, rm=src.
            if let Some(rx) = rex(false, rd.num, 0, rs.num) {
                out.push(rx);
            }
            out.extend_from_slice(&[0x0f, load_op]);
            out.push(modrm(0b11, rd.num, rs.num));
            return Ok(EncodedInsn::raw(out));
        }
        if let Some(mem) = parse_mem_operand(src) {
            // mem, xmm — load.
            let (x, b) = mem_rex_xb(&mem);
            if let Some(rx) = rex(false, rd.num, x, b) {
                out.push(rx);
            }
            out.extend_from_slice(&[0x0f, load_op]);
            out.extend(encode_modrm_mem(rd.num, &mem));
            return Ok(EncodedInsn::raw(out));
        }
        bail!("oxide-as: unsupported instruction (sse mov) `{operands:?}`");
    }
    if let Some(mem) = parse_mem_operand(dst) {
        // xmm, mem — store.
        let Some(rs) = parse_xmm_reg(src) else {
            bail!("oxide-as: unsupported instruction (sse mov) `{operands:?}`");
        };
        let mut out = Vec::new();
        if let Some(p) = prefix {
            out.push(p);
        }
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(false, rs.num, x, b) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, store_op]);
        out.extend(encode_modrm_mem(rs.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction (sse mov) `{operands:?}`")
}

/// Shared encoder for the SSE scalar/packed ALU-shaped ops that only have a
/// single opcode (addss/subss/mulss/divss + sd variants, xorps/xorpd,
/// ucomiss/ucomisd): ModRM.reg is always the xmm destination, ModRM.rm is
/// the xmm-or-memory source (gas's 2-operand AT&T form `op src, dst`). No
/// REX.W — operand width comes from the opcode/prefix, not from REX.
fn encode_sse_alu(operands: &[String], prefix: Option<u8>, opcode: u8) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction (sse alu) `{operands:?}` (needs 2 operands)");
    }
    let src = &operands[0];
    let dst = &operands[1];
    let Some(rd) = parse_xmm_reg(dst) else {
        bail!("oxide-as: unsupported instruction (sse alu) `{operands:?}` (dst must be xmm)");
    };
    let mut out = Vec::new();
    if let Some(p) = prefix {
        out.push(p);
    }
    if let Some(rs) = parse_xmm_reg(src) {
        if let Some(rx) = rex(false, rd.num, 0, rs.num) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, opcode]);
        out.push(modrm(0b11, rd.num, rs.num));
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(mem) = parse_mem_operand(src) {
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(false, rd.num, x, b) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, opcode]);
        out.extend(encode_modrm_mem(rd.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction (sse alu) `{operands:?}`")
}

/// cvtsi2sd/cvtsi2ss (0F 2A) — src is a GPR (32 or 64-bit) or memory, dst is
/// xmm. ModRM.reg = dst xmm, ModRM.rm = src GPR/mem — the reverse of the
/// usual "reg field is the register operand not being addressed via r/m"
/// intuition, since here *both* reg and rm can be plain registers but from
/// different register files. `forced_w`: `Some` when the mnemonic carried an
/// explicit l/q suffix (cvtsi2sdl/cvtsi2sdq); `None` means infer REX.W from
/// the actual GPR operand's width when it's a register, or default to
/// 32-bit for an unsuffixed memory operand (matches observed `as` behaviour:
/// `cvtsi2sd (%rax), %xmm0` with no suffix assembles as the 32-bit form).
fn encode_cvtsi2s(operands: &[String], prefix: u8, forced_w: Option<bool>) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction (cvtsi2s) `{operands:?}` (needs 2 operands)");
    }
    let src = &operands[0];
    let dst = &operands[1];
    let Some(rd) = parse_xmm_reg(dst) else {
        bail!("oxide-as: unsupported instruction (cvtsi2s) `{operands:?}` (dst must be xmm)");
    };
    let mut out = vec![prefix];
    if let Some(rs) = parse_reg(src) {
        let w = forced_w.unwrap_or_else(|| gpr_is_64(src));
        if let Some(rx) = rex(w, rd.num, 0, rs.num) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, 0x2a]);
        out.push(modrm(0b11, rd.num, rs.num));
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(mem) = parse_mem_operand(src) {
        let w = forced_w.unwrap_or(false);
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, rd.num, x, b) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, 0x2a]);
        out.extend(encode_modrm_mem(rd.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction (cvtsi2s) `{operands:?}` (src must be GPR or memory)")
}

/// cvttsd2si/cvttss2si (0F 2C) — src is xmm or memory, dst is a GPR (32 or
/// 64-bit). ModRM.reg = dst GPR, ModRM.rm = src xmm/mem. `forced_w` as in
/// `encode_cvtsi2s`; dst here is always a register (never memory), so when
/// no suffix is given, REX.W is inferred straight from the chosen
/// destination register's width (verified: `cvttsd2si %xmm0, %rax` gets
/// REX.W with no suffix at all, purely from `%rax` being 64-bit).
fn encode_cvttx2si(operands: &[String], prefix: u8, forced_w: Option<bool>) -> Result<EncodedInsn> {
    if operands.len() < 2 {
        bail!("oxide-as: unsupported instruction (cvttx2si) `{operands:?}` (needs 2 operands)");
    }
    let src = &operands[0];
    let dst = &operands[1];
    let Some(rd) = parse_reg(dst) else {
        bail!("oxide-as: unsupported instruction (cvttx2si) `{operands:?}` (dst must be a GPR)");
    };
    let w = forced_w.unwrap_or_else(|| gpr_is_64(dst));
    let mut out = vec![prefix];
    if let Some(rs) = parse_xmm_reg(src) {
        if let Some(rx) = rex(w, rd.num, 0, rs.num) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, 0x2c]);
        out.push(modrm(0b11, rd.num, rs.num));
        return Ok(EncodedInsn::raw(out));
    }
    if let Some(mem) = parse_mem_operand(src) {
        let (x, b) = mem_rex_xb(&mem);
        if let Some(rx) = rex(w, rd.num, x, b) {
            out.push(rx);
        }
        out.extend_from_slice(&[0x0f, 0x2c]);
        out.extend(encode_modrm_mem(rd.num, &mem));
        return Ok(EncodedInsn::raw(out));
    }
    bail!("oxide-as: unsupported instruction (cvttx2si) `{operands:?}` (src must be xmm or memory)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_basics() {
        assert_eq!(encode_insn("ret", &[]).unwrap().bytes, vec![0xc3]);
        assert_eq!(
            encode_insn("pushq", &["%rbp".into()]).unwrap().bytes,
            vec![0x55]
        );
        assert_eq!(
            encode_insn("movq", &["%rsp".into(), "%rbp".into()])
                .unwrap()
                .bytes,
            vec![0x48, 0x89, 0xe5]
        );
        assert_eq!(
            encode_insn("xorl", &["%eax".into(), "%eax".into()])
                .unwrap()
                .bytes,
            vec![0x31, 0xc0]
        );
    }

    #[test]
    fn encode_call_emits_reloc() {
        let e = encode_insn("call", &["puts".into()]).unwrap();
        assert_eq!(e.bytes[0], 0xe8);
        assert_eq!(e.relocs.len(), 1);
        assert_eq!(e.relocs[0].symbol, "puts");
        assert_eq!(e.relocs[0].kind, RelocKind::Plt32);
    }

    #[test]
    fn encode_lea_rip() {
        let e = encode_insn("leaq", &["msg(%rip)".into(), "%rdi".into()]).unwrap();
        assert_eq!(e.bytes[0], 0x48);
        assert_eq!(e.bytes[1], 0x8d);
        assert_eq!(e.relocs[0].symbol, "msg");
        assert_eq!(e.relocs[0].kind, RelocKind::Pc32);
    }

    #[test]
    fn encode_jcc() {
        let e = encode_insn("je", &[".L1".into()]).unwrap();
        assert_eq!(&e.bytes[..2], &[0x0f, 0x84]);
        assert_eq!(e.relocs[0].symbol, ".L1");
    }

    #[test]
    fn unknown_mnemonic_errors_instead_of_nop() {
        let err = encode_insn("frobnicate", &["%eax".into()]).unwrap_err();
        assert!(err.to_string().contains("frobnicate"));
    }

    #[test]
    fn unsupported_mov_shape_errors() {
        // mem, mem is not a valid operand pair for mov.
        let err = encode_insn("movl", &["(%rax)".into(), "(%rbx)".into()]).unwrap_err();
        assert!(err.to_string().contains("mov"));
    }

    #[test]
    fn encode_mov_base_index_scale() {
        // movl (%rax,%rcx,8), %edx  →  8b 14 c8
        // modrm = mod00 reg=010(edx) rm=100(SIB); sib = scale11(8) index001(rcx) base000(rax)
        let e = encode_insn("movl", &["(%rax,%rcx,8)".into(), "%edx".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x8b, 0x14, 0xc8]);
    }

    #[test]
    fn encode_lea_disp_base_index() {
        // leaq -8(%rbp,%rax,4), %rdi  →  48 8d 7c 85 f8
        // REX.W; modrm = mod01 reg=111(rdi) rm=100(SIB); sib = scale10(4) index000(rax) base101(rbp); disp8=-8
        let e = encode_insn("leaq", &["-8(%rbp,%rax,4)".into(), "%rdi".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x48, 0x8d, 0x7c, 0x85, 0xf8]);
    }

    #[test]
    fn encode_mov_disp_base_no_sib() {
        // movq -4(%rbp), %rax  →  48 8b 45 fc  (plain ModRM, no SIB needed)
        let e = encode_insn("movq", &["-4(%rbp)".into(), "%rax".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x48, 0x8b, 0x45, 0xfc]);
    }

    #[test]
    fn encode_mov_rsp_base_forces_sib() {
        // movq (%rsp), %rax  →  48 8b 04 24  (rsp as base always needs a SIB byte)
        let e = encode_insn("movq", &["(%rsp)".into(), "%rax".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x48, 0x8b, 0x04, 0x24]);
    }

    #[test]
    fn encode_alu_mem_operands() {
        // addl %eax, -4(%rbp)  →  01 45 fc  (store direction: mem is r/m dest)
        let e = encode_insn("addl", &["%eax".into(), "-4(%rbp)".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x01, 0x45, 0xfc]);

        // addl $1, (%rax)  →  83 00 01  (imm8, group /0, mem r/m)
        let e = encode_insn("addl", &["$1".into(), "(%rax)".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x83, 0x00, 0x01]);
    }

    #[test]
    fn encode_mov_extended_regs_sib() {
        // movl (%r8,%r9,4), %r10d  →  47 8b 14 88
        // REX = 0100 0111 (R,X,B all set for r10/r9/r8); modrm mod00 reg=010(r10&7) rm=100(SIB);
        // sib = scale10(4) index001(r9&7) base000(r8&7)
        let e = encode_insn("movl", &["(%r8,%r9,4)".into(), "%r10d".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x47, 0x8b, 0x14, 0x88]);
    }

    #[test]
    fn encode_push_pop_mem() {
        // pushq (%rax) → ff 30  (FF /6, no REX needed)
        let e = encode_insn("pushq", &["(%rax)".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xff, 0x30]);
        // popq (%rax) → 8f 00  (8F /0)
        let e = encode_insn("popq", &["(%rax)".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x8f, 0x00]);
    }

    #[test]
    fn unsuffixed_collision_mnemonics_still_work() {
        // "call"/"sub"/"mul"/"imul"/"shl"/"sal"/"rol" all end in a letter
        // that looks like a size suffix — regression test for the
        // strip_size_suffix mnemonic-collision bug that used to make these
        // fall through to the catch-all (previously NOP, now an error).
        // Bare (unsuffixed) mnemonics default to 64-bit in this assembler
        // (size is inferred purely from the mnemonic, not the registers),
        // hence the REX.W prefix on both cases below.
        assert_eq!(
            encode_insn("call", &["puts".into()]).unwrap().bytes[0],
            0xe8
        );
        assert_eq!(
            encode_insn("sub", &["%eax".into(), "%ebx".into()])
                .unwrap()
                .bytes,
            vec![0x48, 0x29, 0xc3]
        );
        assert_eq!(
            encode_insn("mul", &["%eax".into()]).unwrap().bytes,
            vec![0x48, 0xf7, 0xe0]
        );
    }

    // SSE/SSE2 tests below have expected byte sequences cross-checked
    // against real `as`/`objdump` (binutils 2.46) disassembly of the same
    // AT&T source, not just hand-derived from the Intel manual.

    #[test]
    fn encode_movsd_mem_load() {
        // movsd (%rax), %xmm0  →  f2 0f 10 00
        let e = encode_insn("movsd", &["(%rax)".into(), "%xmm0".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x0f, 0x10, 0x00]);
    }

    #[test]
    fn encode_addsd_rr() {
        // addsd %xmm1, %xmm0  →  f2 0f 58 c1
        let e = encode_insn("addsd", &["%xmm1".into(), "%xmm0".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x0f, 0x58, 0xc1]);
    }

    #[test]
    fn encode_xorps_zeroing_idiom() {
        // xorps %xmm0, %xmm0  →  0f 57 c0  (no prefix, no REX)
        let e = encode_insn("xorps", &["%xmm0".into(), "%xmm0".into()]).unwrap();
        assert_eq!(e.bytes, vec![0x0f, 0x57, 0xc0]);
    }

    #[test]
    fn encode_cvtsi2sdq_gpr64() {
        // cvtsi2sdq %rdi, %xmm0  →  f2 48 0f 2a c7  (REX.W from explicit q suffix)
        let e = encode_insn("cvtsi2sdq", &["%rdi".into(), "%xmm0".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x48, 0x0f, 0x2a, 0xc7]);
    }

    #[test]
    fn encode_cvttsd2si_to_32bit_gpr() {
        // cvttsd2si %xmm0, %eax  →  f2 0f 2c c0  (no REX.W: dst is %eax)
        let e = encode_insn("cvttsd2si", &["%xmm0".into(), "%eax".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x0f, 0x2c, 0xc0]);
    }

    #[test]
    fn encode_cvttsd2si_to_64bit_gpr_infers_rexw_from_register() {
        // cvttsd2si %xmm0, %rax  →  f2 48 0f 2c c0  (REX.W inferred purely
        // from the %rax destination register, no suffix on the mnemonic).
        let e = encode_insn("cvttsd2si", &["%xmm0".into(), "%rax".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x48, 0x0f, 0x2c, 0xc0]);
    }

    #[test]
    fn encode_movsd_sib_mem_operand() {
        // movsd 8(%rbx,%rax,8), %xmm2  →  f2 0f 10 54 c3 08
        // modrm = mod01 reg=010(xmm2) rm=100(SIB); sib = scale11(8) index000(rax) base011(rbx); disp8=8
        let e = encode_insn("movsd", &["8(%rbx,%rax,8)".into(), "%xmm2".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x0f, 0x10, 0x54, 0xc3, 0x08]);
    }

    #[test]
    fn encode_sse_extended_xmm_regs() {
        // addsd %xmm9, %xmm10  →  f2 45 0f 58 d1  (REX.R for xmm10 reg field, REX.B for xmm9 rm field)
        let e = encode_insn("addsd", &["%xmm9".into(), "%xmm10".into()]).unwrap();
        assert_eq!(e.bytes, vec![0xf2, 0x45, 0x0f, 0x58, 0xd1]);
    }

    #[test]
    fn encode_unsupported_sse_shape_errors() {
        // movsd mem, mem is not a valid operand pair.
        let err = encode_insn("movsd", &["(%rax)".into(), "(%rbx)".into()]).unwrap_err();
        assert!(err.to_string().contains("sse mov"));
    }
}
