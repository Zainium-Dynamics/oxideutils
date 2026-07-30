//! Assembly lexer & directive parser for oxide-as.
//!
//! Pseudo-ops mirror GNU gas 2.46.1 `gas/read.c` `potable[]`:
//!   .align / .p2align  → s_align_ptwo (power-of-two alignment)
//!   .balign            → s_align_bytes
//!   .zero / .space / .skip → s_space
//!   .byte/.word/.short/.long/.int/.quad → cons(size)
//!   .ascii             → stringer(no NUL)
//!   .asciz / .string   → stringer(+ NUL)
//!   .globl / .global   → s_globl
//!   .text/.data/.bss/.rodata/.section → section switch

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
    RoData,
    Bss,
    Named(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Directive {
    /// Switch to a section (gas s_text / s_data / obj_elf_section).
    Section(SectionKind),
    /// `.globl` / `.global` / `.xdef` (gas s_globl).
    Global(String),
    /// `.align N` / `.p2align N` — power-of-two alignment (gas s_align_ptwo).
    AlignP2(u64),
    /// `.balign N` — align to N-byte boundary (gas s_align_bytes).
    AlignBytes(u64),
    /// `.zero N` / `.space N` / `.skip N` (gas s_space).
    Zero(u64),
    /// `.ascii "…"` — no trailing NUL (gas stringer bits_appendzero=8+0).
    Ascii(Vec<u8>),
    /// `.asciz` / `.string` — with trailing NUL (gas stringer 8+1).
    Asciz(Vec<u8>),
    /// `.byte`  → cons(1)
    Byte(Vec<u8>),
    /// `.word` / `.short` / `.hword` / `.2byte` → cons(2)
    Word(Vec<u16>),
    /// `.long` / `.int` / `.4byte` → cons(4)
    Long(Vec<u32>),
    /// `.quad` / `.8byte` → cons(8). Entries may be integer literals *or*
    /// symbol references (`.quad ctor_fn` — needed for any real
    /// `.init_array`/vtable/function-pointer-table construction; without
    /// this, `.quad some_symbol` used to silently emit nothing at all).
    Quad(Vec<QuadItem>),
    /// `.cfi_startproc` / `.cfi_endproc` — accepted, no emission yet.
    CfiProc,
    /// `.equ name, value` / `.set name, value` (gas s_setsym) — integer
    /// literal only (no general expression evaluation).
    Set(String, i64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum QuadItem {
    Int(u64),
    /// `sym` or `sym+N`/`sym-N` — the addend an ELF relocation already
    /// carries natively, so no general expression evaluator is needed for
    /// this (very common — e.g. a table entry pointing partway into
    /// another symbol) case.
    Sym(String, i64),
    /// Bare `.` (current location counter), optionally `.+N`/`.-N` —
    /// resolved immediately by the caller to the running section length at
    /// the point this directive is processed (a plain integer, not a
    /// relocation). `. - other_label` (symbol difference) is not
    /// supported — documented gap.
    Here(i64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Directive(Directive),
    Label(String),
    Instruction {
        mnemonic: String,
        operands: Vec<String>,
    },
}

/// Parse a full assembly source into statements (line-oriented gas subset).
pub fn parse_assembly(source: &str) -> Vec<Statement> {
    let mut statements = Vec::new();

    for line in source.lines() {
        let mut text = line.trim();

        // gas app.c strips `#` and `//` comments (and `/* */` is rarer in asm).
        if let Some(pos) = text.find('#') {
            text = &text[..pos].trim_end();
        }
        if let Some(pos) = text.find("//") {
            text = &text[..pos].trim_end();
        }
        // Strip trailing C-style block comment openers on same line.
        if let Some(pos) = text.find("/*") {
            text = &text[..pos].trim_end();
        }
        if text.is_empty() {
            continue;
        }

        // Label may share a line with an instruction: `foo: nop`
        if let Some((label_part, rest)) = split_label(text) {
            statements.push(Statement::Label(label_part.to_string()));
            text = rest.trim();
            if text.is_empty() {
                continue;
            }
        }

        if text.starts_with('.') {
            if let Some(dir) = parse_directive(text) {
                statements.push(Statement::Directive(dir));
            }
            continue;
        }

        // Instruction: mnemonic + comma-separated operands (AT&T style).
        let mut parts = text.split_whitespace();
        if let Some(mnemonic) = parts.next() {
            let operands_str = parts.collect::<Vec<_>>().join(" ");
            let operands = split_operands(&operands_str);
            statements.push(Statement::Instruction {
                mnemonic: mnemonic.to_string(),
                operands,
            });
        }
    }

    statements
}

/// Split comma-separated operands, ignoring commas nested inside `(...)` —
/// needed for SIB memory operands like `(%rax,%rcx,8)`, which contain their
/// own internal commas that must not be split into separate "operands".
fn split_operands(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                cur.push(c);
            }
            ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                let t = cur.trim();
                if !t.is_empty() {
                    out.push(t.to_string());
                }
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    let t = cur.trim();
    if !t.is_empty() {
        out.push(t.to_string());
    }
    out
}

/// `label:` or `label: rest` — returns (label, rest).
fn split_label(text: &str) -> Option<(&str, &str)> {
    // Don't treat `.L0:` style as non-directive — those are labels.
    if text.starts_with('.') && text.chars().nth(1).is_some_and(|c| c.is_ascii_alphabetic()
        && !matches!(
            text.split_once(':').map(|(a, _)| a),
            Some(n) if n.starts_with(".L") || n.starts_with(".l")
        ))
    {
        // `.text` etc. are directives, not labels.
        // Local labels like `.Lfoo:` do start with `.L`.
        if !text.starts_with(".L") && !text.starts_with(".l") {
            return None;
        }
    }
    let colon = text.find(':')?;
    // Avoid matching colons inside strings (rare on same line as label).
    let label = text[..colon].trim();
    if label.is_empty() {
        return None;
    }
    // Directive lines never have bare `name:` without starting with `.` for locals.
    if label.starts_with('.') && !(label.starts_with(".L") || label.starts_with(".l")) {
        return None;
    }
    // Register-like or expression with `:` (segment:offset) — skip if has `%`.
    if label.contains('%') || label.contains('$') {
        return None;
    }
    Some((label, &text[colon + 1..]))
}

fn parse_directive(text: &str) -> Option<Directive> {
    let (name, rest) = split_directive(text)?;
    let arg = rest.trim();

    match name {
        // Sections — gas s_text / s_data / obj_elf
        "text" => Some(Directive::Section(SectionKind::Text)),
        "data" => Some(Directive::Section(SectionKind::Data)),
        "rodata" => Some(Directive::Section(SectionKind::RoData)),
        "bss" => Some(Directive::Section(SectionKind::Bss)),
        "section" => {
            let sec = arg
                .split(|c: char| c == ',' || c.is_whitespace())
                .next()
                .unwrap_or(".text")
                .trim_matches('"');
            let kind = match sec {
                ".text" | "text" => SectionKind::Text,
                ".data" | "data" => SectionKind::Data,
                ".rodata" | "rodata" => SectionKind::RoData,
                ".bss" | "bss" => SectionKind::Bss,
                other => SectionKind::Named(other.to_string()),
            };
            Some(Directive::Section(kind))
        }

        // s_globl
        "globl" | "global" | "xdef" => {
            let name = arg
                .split(|c: char| c == ',' || c.is_whitespace())
                .find(|s| !s.is_empty())
                .unwrap_or("main");
            Some(Directive::Global(name.to_string()))
        }

        // On x86 (unlike e.g. ARM/MIPS), gas's `.align N` takes a *byte*
        // count via `s_align_bytes` — only `.p2align` is power-of-two. Using
        // `AlignP2` for plain `.align` here used to silently over-align by
        // 2^N instead of N bytes (e.g. `.align 16` became a 64KiB align).
        "align" => {
            let n = parse_int(arg.split(',').next().unwrap_or("1")).unwrap_or(1);
            Some(Directive::AlignBytes(n.max(1)))
        }
        "p2align" => {
            let n = parse_int(arg.split(',').next().unwrap_or("0")).unwrap_or(0);
            Some(Directive::AlignP2(n))
        }
        // s_align_bytes
        "balign" => {
            let n = parse_int(arg.split(',').next().unwrap_or("1")).unwrap_or(1);
            Some(Directive::AlignBytes(n.max(1)))
        }

        // s_space
        "zero" | "space" | "skip" => {
            let n = parse_int(arg.split(',').next().unwrap_or("0")).unwrap_or(0);
            Some(Directive::Zero(n))
        }

        // stringer
        "ascii" => Some(Directive::Ascii(parse_string_arg(arg))),
        "asciz" | "string" | "string8" => Some(Directive::Asciz(parse_string_arg(arg))),

        // cons
        "byte" | "dc.b" => Some(Directive::Byte(parse_int_list(arg).into_iter().map(|v| v as u8).collect())),
        "word" | "short" | "hword" | "2byte" | "dc.w" => {
            Some(Directive::Word(parse_int_list(arg).into_iter().map(|v| v as u16).collect()))
        }
        "long" | "int" | "4byte" | "dc.l" => {
            Some(Directive::Long(parse_int_list(arg).into_iter().map(|v| v as u32).collect()))
        }
        "quad" | "8byte" => Some(Directive::Quad(parse_quad_list(arg))),

        "cfi_startproc" | "cfi_endproc" => Some(Directive::CfiProc),

        // gas s_setsym — `.equ name, value` / `.set name, value` (alias).
        // Integer-literal expressions only, matching parse_int's scope.
        "equ" | "set" => {
            let mut parts = arg.splitn(2, ',');
            let sym = parts.next().unwrap_or("").trim();
            let value = parts.next().unwrap_or("").trim();
            if sym.is_empty() {
                None
            } else {
                parse_int(value).map(|v| Directive::Set(sym.to_string(), v as i64))
            }
        }

        // gas s_ignore-ish / unsupported — skip silently
        "file" | "type" | "size" | "ident" | "loc" | "cfi_def_cfa" | "cfi_offset"
        | "cfi_restore" | "cfi_undefined" | "cfi_remember_state" | "cfi_restore_state"
        | "cfi_def_cfa_offset" | "cfi_def_cfa_register" | "cfi_escape" | "cfi_sections"
        | "cfi_personality" | "cfi_lsda" | "cfi_signal_frame" | "cfi_window_save"
        | "cfi_register" | "cfi_same_value" | "cfi_rel_offset" | "cfi_adjust_cfa_offset"
        | "cfi_val_offset" | "cfi_val_expression" | "cfi_expression" => None,

        _ => None,
    }
}

fn split_directive(text: &str) -> Option<(&str, &str)> {
    let t = text.strip_prefix('.')?;
    let end = t
        .find(|c: char| c.is_whitespace() || c == ',' || c == '"' || c == '@')
        .unwrap_or(t.len());
    // name may include dots like `dc.b` — but we already stripped one `.`
    // For `section .text,"ax"` rest starts after first whitespace.
    let name_end = t
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, _)| i)
        .unwrap_or(end.min(t.len()));
    let name = &t[..name_end];
    let rest = t[name_end..].trim_start();
    if name.is_empty() {
        return None;
    }
    Some((name, rest))
}

fn parse_string_arg(arg: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chars = arg.chars().peekable();
    // gas stringer: one or more comma-separated quoted strings
    while let Some(&c) = chars.peek() {
        if c == '"' {
            chars.next();
            while let Some(ch) = chars.next() {
                if ch == '"' {
                    break;
                }
                if ch == '\\' {
                    match chars.next() {
                        Some('n') => out.push(b'\n'),
                        Some('t') => out.push(b'\t'),
                        Some('r') => out.push(b'\r'),
                        Some('0') => out.push(0),
                        Some('\\') => out.push(b'\\'),
                        Some('"') => out.push(b'"'),
                        Some('x') => {
                            let h1 = chars.next().unwrap_or('0');
                            let h2 = chars.next().unwrap_or('0');
                            let s = format!("{h1}{h2}");
                            out.push(u8::from_str_radix(&s, 16).unwrap_or(0));
                        }
                        Some(other) => out.push(other as u8),
                        None => {}
                    }
                } else {
                    out.push(ch as u8);
                }
            }
        } else if c == ',' || c.is_whitespace() {
            chars.next();
        } else {
            // unquoted — take rest as raw (rare)
            break;
        }
    }
    out
}

fn parse_int_list(arg: &str) -> Vec<u64> {
    arg.split(',')
        .filter_map(|s| parse_int(s.trim()))
        .collect()
}

/// Like `parse_int_list`, but entries that aren't an integer literal and
/// look like a bare symbol name (optionally `sym+N`/`sym-N`) become
/// `QuadItem::Sym`, and a bare `.` (optionally `.+N`/`.-N`) becomes
/// `QuadItem::Here`, instead of being silently dropped (`.quad some_fn` —
/// the common function-pointer-table case: `.init_array`/`.fini_array`,
/// vtables, jump tables).
fn parse_quad_list(arg: &str) -> Vec<QuadItem> {
    arg.split(',')
        .filter_map(|tok| {
            let tok = tok.trim();
            if tok.is_empty() {
                return None;
            }
            if let Some(v) = parse_int(tok) {
                return Some(QuadItem::Int(v));
            }
            if tok == "." {
                return Some(QuadItem::Here(0));
            }
            if let Some(rest) = tok.strip_prefix('.') {
                if (rest.starts_with('+') || rest.starts_with('-')) && !rest.starts_with("..") {
                    if let Some(v) = parse_int(rest) {
                        return Some(QuadItem::Here(v as i64));
                    }
                }
            }
            let (sym, addend) = split_symbol_addend(tok);
            let first = sym.chars().next()?;
            (first.is_ascii_alphabetic() || first == '_' || first == '.')
                .then(|| QuadItem::Sym(sym.to_string(), addend))
        })
        .collect()
}

/// Split a trailing `+N`/`-N` off a symbol-ish token (mirrors
/// `encode::split_symbol_addend`, kept separate since it operates on
/// `parser`'s own `parse_int`).
fn split_symbol_addend(text: &str) -> (&str, i64) {
    for (i, c) in text.char_indices().rev() {
        if i == 0 {
            break;
        }
        if c == '+' || c == '-' {
            let (sym, off) = text.split_at(i);
            if let Some(v) = parse_int(off) {
                return (sym, v as i64);
            }
        }
    }
    (text, 0)
}

fn parse_int(s: &str) -> Option<u64> {
    let s = s.trim().trim_start_matches('$');
    if s.is_empty() {
        return None;
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u64::from_str_radix(hex, 16).ok();
    }
    if s.starts_with('0') && s.len() > 1 && s.chars().all(|c| c.is_digit(8)) {
        return u64::from_str_radix(s, 8).ok();
    }
    s.parse::<i64>().ok().map(|v| v as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gas_data_directives() {
        let src = r#"
            .text
            .globl _start
            _start:
                nop
            .data
            .p2align 3
            .align 16
            .byte 1, 2, 0x10
            .word 0x1234
            .long 0xdeadbeef
            .quad 0x1122334455667788
            .quad ctor_fn
            .zero 4
            .ascii "hi"
            .asciz "yo"
            .balign 8
        "#;
        let st = parse_assembly(src);
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Section(SectionKind::Text)))));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Global(n)) if n == "_start")));
        assert!(st.iter().any(|s| matches!(s, Statement::Label(n) if n == "_start")));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::AlignP2(3)))));
        // Plain `.align N` on x86 is a byte count (`s_align_bytes`), not
        // power-of-two — distinct from `.p2align` above.
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::AlignBytes(16)))));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Byte(v)) if v == &[1, 2, 0x10])));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Word(v)) if v == &[0x1234])));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Long(v)) if v == &[0xdeadbeef])));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Quad(v)) if v == &[QuadItem::Int(0x1122334455667788)])));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Quad(v)) if v == &[QuadItem::Sym("ctor_fn".to_string(), 0)])));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Zero(4)))));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Ascii(v)) if v == b"hi")));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::Asciz(v)) if v == b"yo")));
        assert!(st.iter().any(|s| matches!(s, Statement::Directive(Directive::AlignBytes(8)))));
    }

    #[test]
    fn parses_operands() {
        let st = parse_assembly("movq %rsp, %rbp\nxorl %eax, %eax\nret\n");
        match &st[0] {
            Statement::Instruction { mnemonic, operands } => {
                assert_eq!(mnemonic, "movq");
                assert_eq!(operands, &["%rsp".to_string(), "%rbp".to_string()]);
            }
            _ => panic!("expected instruction"),
        }
        match &st[1] {
            Statement::Instruction { mnemonic, operands } => {
                assert_eq!(mnemonic, "xorl");
                assert_eq!(operands, &["%eax".to_string(), "%eax".to_string()]);
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn label_with_instruction_same_line() {
        let st = parse_assembly("foo: ret\n");
        assert_eq!(st.len(), 2);
        assert!(matches!(&st[0], Statement::Label(n) if n == "foo"));
        assert!(matches!(&st[1], Statement::Instruction { mnemonic, .. } if mnemonic == "ret"));
    }

    #[test]
    fn parses_sib_memory_operand_without_splitting_on_inner_commas() {
        let st = parse_assembly("movl (%rax,%rcx,8), %edx\nleaq -8(%rbp,%rax,4), %rdi\n");
        match &st[0] {
            Statement::Instruction { mnemonic, operands } => {
                assert_eq!(mnemonic, "movl");
                assert_eq!(
                    operands,
                    &["(%rax,%rcx,8)".to_string(), "%edx".to_string()]
                );
            }
            _ => panic!("expected instruction"),
        }
        match &st[1] {
            Statement::Instruction { mnemonic, operands } => {
                assert_eq!(mnemonic, "leaq");
                assert_eq!(
                    operands,
                    &["-8(%rbp,%rax,4)".to_string(), "%rdi".to_string()]
                );
            }
            _ => panic!("expected instruction"),
        }
    }

    #[test]
    fn parses_equ_and_set() {
        let st = parse_assembly(".equ FOO, 42\n.set BAR, 0x10\n");
        assert!(st.iter().any(
            |s| matches!(s, Statement::Directive(Directive::Set(n, v)) if n == "FOO" && *v == 42)
        ));
        assert!(st.iter().any(
            |s| matches!(s, Statement::Directive(Directive::Set(n, v)) if n == "BAR" && *v == 0x10)
        ));
    }
}
