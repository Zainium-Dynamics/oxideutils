//! Text-level macro expansion and conditional assembly — a reduced-scale
//! `gas/macro.c` + `gas/cond.c`: `.macro`/`.endm` (named + positional `\arg`
//! substitution, `name=default` params), `.rept`/`.endr`, and
//! `.if`/`.ifdef`/`.ifndef`/`.else`/`.endif`. Runs as a line-text pass
//! *before* `parser::parse_assembly` — every other directive/instruction is
//! passed through unchanged, so the rest of the assembler never needs to
//! know macros existed.
//!
//! Scope, documented rather than hidden: no `.elseif` chains, no general
//! expression evaluator in `.if` (bare integer, or `.ifdef`/`.ifndef`
//! against the set of names introduced by `.equ`/`.set`/`.macro`), no
//! `.altmacro`, no `.irp`/`.irpc`.

use std::collections::{HashMap, VecDeque};

struct MacroDef {
    /// `(name, default_value_text)` — default is `None` when the param has
    /// no `=value` in its `.macro` declaration.
    params: Vec<(String, Option<String>)>,
    body: Vec<String>,
}

/// Expand every `.macro`/`.rept`/`.if` construct in `source`, returning
/// plain assembly text with none of them left — safe to feed straight into
/// `parser::parse_assembly`.
pub fn preprocess(source: &str) -> String {
    let mut lines: VecDeque<String> = source.lines().map(|s| s.to_string()).collect();
    let mut macros: HashMap<String, MacroDef> = HashMap::new();
    let mut known_defined: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut known_values: HashMap<String, i64> = HashMap::new();
    let mut out: Vec<String> = Vec::new();
    // Each entry: (this_branch_active, condition_was_true, in_else).
    let mut if_stack: Vec<(bool, bool, bool)> = Vec::new();

    while let Some(line) = lines.pop_front() {
        let trimmed = line.trim();
        let word = first_word(trimmed);
        let enclosing_active = if_stack.iter().all(|(active, _, _)| *active);

        match word {
            ".macro" => {
                let def = trimmed[".macro".len()..].trim();
                let (name, params) = parse_macro_decl(def);
                let body = capture_until(&mut lines, ".macro", ".endm");
                if enclosing_active {
                    known_defined.insert(name.clone());
                    macros.insert(name, MacroDef { params, body });
                }
            }
            ".rept" => {
                let count = if enclosing_active {
                    eval_int_atom(trimmed[".rept".len()..].trim(), &known_values).unwrap_or(0)
                } else {
                    0
                };
                let body = capture_until(&mut lines, ".rept", ".endr");
                if enclosing_active {
                    for _ in 0..count {
                        for l in body.iter().rev() {
                            lines.push_front(l.clone());
                        }
                    }
                }
            }
            ".if" => {
                let cond = enclosing_active
                    && eval_int_atom(trimmed[".if".len()..].trim(), &known_values).unwrap_or(0) != 0;
                if_stack.push((enclosing_active && cond, cond, false));
            }
            ".ifdef" | ".ifndef" => {
                let name = trimmed[word.len()..].trim();
                let is_def = known_defined.contains(name);
                let cond = if word == ".ifdef" { is_def } else { !is_def };
                if_stack.push((enclosing_active && cond, cond, false));
            }
            ".else" => {
                if let Some((active, cond_was_true, in_else)) = if_stack.pop() {
                    let _ = active;
                    let parent_active = if_stack.iter().all(|(a, _, _)| *a);
                    if_stack.push((parent_active && !cond_was_true, cond_was_true, true));
                    let _ = in_else;
                }
            }
            ".endif" => {
                if_stack.pop();
            }
            _ if enclosing_active => {
                // `.equ`/`.set NAME, value` — tracked for `.ifdef`/`.rept`
                // conditions, but still passed through unchanged since the
                // real assembler still needs to see them.
                if matches!(word, ".equ" | ".set") {
                    let rest = trimmed[word.len()..].trim();
                    if let Some((name, val)) = rest.split_once(',') {
                        let name = name.trim().to_string();
                        if let Some(v) = eval_int_atom(val.trim(), &known_values) {
                            known_values.insert(name.clone(), v);
                        }
                        known_defined.insert(name);
                    }
                    out.push(line);
                    continue;
                }
                if let Some(mdef) = macros.get(word) {
                    let args_text = trimmed[word.len()..].trim();
                    let expanded = expand_macro(mdef, args_text);
                    for l in expanded.into_iter().rev() {
                        lines.push_front(l);
                    }
                } else {
                    out.push(line);
                }
            }
            _ => {} // inside an inactive .if/.ifdef branch — drop the line
        }
    }

    out.join("\n")
}

fn first_word(trimmed: &str) -> &str {
    trimmed.split_whitespace().next().unwrap_or("")
}

/// Consume lines (already past the opening directive) until the matching
/// `end` for `start`/`end`, tracking nesting depth so an inner
/// `.macro`/`.rept` of the *same* kind doesn't end the outer one early.
fn capture_until(lines: &mut VecDeque<String>, start: &str, end: &str) -> Vec<String> {
    let mut depth = 1i32;
    let mut body = Vec::new();
    while let Some(l) = lines.pop_front() {
        let w = first_word(l.trim());
        if w == start {
            depth += 1;
        } else if w == end {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
        body.push(l);
    }
    body
}

/// `.macro NAME p1, p2=default, p3` -> `(NAME, [(p1,None),(p2,Some("default")),(p3,None)])`.
fn parse_macro_decl(def: &str) -> (String, Vec<(String, Option<String>)>) {
    let mut parts = def.splitn(2, char::is_whitespace);
    let name = parts.next().unwrap_or("").trim().to_string();
    let rest = parts.next().unwrap_or("");
    let params = rest
        .split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((n, v)) => (n.trim().to_string(), Some(v.trim().to_string())),
            None => (p.to_string(), None),
        })
        .collect();
    (name, params)
}

/// Split macro-invocation arguments on top-level commas (parens still
/// protect commas inside them, same rule as instruction operands).
fn split_macro_args(s: &str) -> Vec<String> {
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
                out.push(cur.trim().to_string());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.trim().is_empty() || !out.is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn expand_macro(def: &MacroDef, args_text: &str) -> Vec<String> {
    let args = split_macro_args(args_text);
    let mut subst: HashMap<String, String> = HashMap::new();
    for (i, (name, default)) in def.params.iter().enumerate() {
        let value = args
            .get(i)
            .filter(|a| !a.is_empty())
            .cloned()
            .or_else(|| default.clone())
            .unwrap_or_default();
        subst.insert(name.clone(), value.clone());
        subst.insert((i + 1).to_string(), value);
    }
    def.body.iter().map(|l| substitute_params(l, &subst)).collect()
}

/// Replace every `\name` (longest match first, so `\10` doesn't get read as
/// `\1` + literal `0`) with its bound value; `\\` is a literal backslash.
fn substitute_params(line: &str, subst: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            if chars[i + 1] == '\\' {
                out.push('\\');
                i += 2;
                continue;
            }
            let start = i + 1;
            let mut end = start;
            while end < chars.len() && (chars[end].is_ascii_alphanumeric() || chars[end] == '_') {
                end += 1;
            }
            if end > start {
                let name: String = chars[start..end].iter().collect();
                if let Some(val) = subst.get(&name) {
                    out.push_str(val);
                    i = end;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Evaluate a bare integer literal or a previously-seen `.equ`/`.set` name —
/// the practically-useful slice of `.if`/`.rept` conditions (no general
/// arithmetic/comparison operators).
fn eval_int_atom(text: &str, known: &HashMap<String, i64>) -> Option<i64> {
    let text = text.trim();
    if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).ok();
    }
    if let Ok(v) = text.parse::<i64>() {
        return Some(v);
    }
    known.get(text).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_macro_named_and_positional_args() {
        let src = "\
.macro add3 dst, a, b
    movl \\a, \\dst
    addl \\b, \\dst
.endm
add3 %eax, $1, $2
";
        let out = preprocess(src);
        assert!(out.contains("movl $1, %eax"));
        assert!(out.contains("addl $2, %eax"));
        assert!(!out.contains(".macro"));
        assert!(!out.contains(".endm"));
    }

    #[test]
    fn macro_default_param() {
        let src = "\
.macro greet who=world
    .ascii \"\\who\"
.endm
greet
";
        let out = preprocess(src);
        assert!(out.contains(".ascii \"world\""));
    }

    #[test]
    fn rept_repeats_body_n_times() {
        let src = "\
.rept 3
    nop
.endr
";
        let out = preprocess(src);
        assert_eq!(out.matches("nop").count(), 3);
    }

    #[test]
    fn if_zero_drops_body_else_keeps_else_branch() {
        let src = "\
.if 0
    int3
.else
    nop
.endif
";
        let out = preprocess(src);
        assert!(!out.contains("int3"));
        assert!(out.contains("nop"));
    }

    #[test]
    fn if_nonzero_keeps_body_drops_else() {
        let src = "\
.if 1
    nop
.else
    int3
.endif
";
        let out = preprocess(src);
        assert!(out.contains("nop"));
        assert!(!out.contains("int3"));
    }

    #[test]
    fn ifdef_tracks_equ_names() {
        let src = "\
.equ FEATURE_X, 1
.ifdef FEATURE_X
    nop
.endif
.ifndef FEATURE_Y
    int3
.endif
";
        let out = preprocess(src);
        assert!(out.contains("nop"));
        assert!(out.contains("int3"));
    }

    #[test]
    fn nested_if_inside_macro_body() {
        let src = "\
.macro pick n
.if \\n
    movl $1, %eax
.else
    movl $0, %eax
.endif
.endm
pick 1
pick 0
";
        let out = preprocess(src);
        assert_eq!(out.matches("movl $1, %eax").count(), 1);
        assert_eq!(out.matches("movl $0, %eax").count(), 1);
    }
}
