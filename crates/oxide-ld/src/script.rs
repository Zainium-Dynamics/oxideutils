//! Minimal GNU ld linker-script subset (`ld/ldgram.y` / default ELF script ideas).
//!
//! Supported:
//!   ENTRY(symbol)
//!   SECTIONS { .name : { *(pattern) } ... }
//!   OUTPUT_FORMAT / SEARCH_DIR / INPUT — accepted no-ops
//!
//! Default layout when no -T script is given mirrors the classic x86_64 ELF
//! executable VMA: text @ 0x400000, data after text (page-aligned).

#[derive(Debug, Clone)]
pub struct LinkerScript {
    pub entry: Option<String>,
    /// Output section name → list of input section patterns (e.g. `.text`, `.text.*`)
    pub sections: Vec<OutputSection>,
    pub text_vma: u64,
    pub data_align: u64,
}

#[derive(Debug, Clone)]
pub struct OutputSection {
    pub name: String,
    pub patterns: Vec<String>,
    /// If set, force VMA (else sequential).
    pub vma: Option<u64>,
}

impl Default for LinkerScript {
    fn default() -> Self {
        // Roughly: ld default script for elf_x86_64
        Self {
            entry: Some("_start".into()),
            text_vma: 0x400000,
            data_align: 0x1000,
            sections: vec![
                OutputSection {
                    name: ".interp".into(),
                    patterns: vec![".interp".into()],
                    vma: None,
                },
                OutputSection {
                    name: ".text".into(),
                    patterns: vec![".text".into(), ".text.*".into()],
                    vma: None,
                },
                OutputSection {
                    name: ".rodata".into(),
                    patterns: vec![".rodata".into(), ".rodata.*".into()],
                    vma: None,
                },
                // Constructor/destructor tables — crt0/crti/crtn from any
                // real libc (musl, relibc) read these via linker-provided
                // `__*_array_start/end` symbols (see `linker.rs`
                // `inject_crt_symbols`). `.ctors`/`.dtors` are the legacy
                // (pre-`.init_array`) GCC mechanism; mapped here too since
                // some object files still emit them.
                OutputSection {
                    name: ".init_array".into(),
                    patterns: vec![
                        ".init_array".into(),
                        ".init_array.*".into(),
                        ".ctors".into(),
                        ".ctors.*".into(),
                    ],
                    vma: None,
                },
                OutputSection {
                    name: ".fini_array".into(),
                    patterns: vec![
                        ".fini_array".into(),
                        ".fini_array.*".into(),
                        ".dtors".into(),
                        ".dtors.*".into(),
                    ],
                    vma: None,
                },
                OutputSection {
                    name: ".preinit_array".into(),
                    patterns: vec![".preinit_array".into(), ".preinit_array.*".into()],
                    vma: None,
                },
                // TLS template (Phase 1) — Variant II layout, `.tdata` then
                // `.tbss`, merged into one `PT_TLS` segment by `linker.rs`.
                OutputSection {
                    name: ".tdata".into(),
                    patterns: vec![".tdata".into(), ".tdata.*".into()],
                    vma: None,
                },
                OutputSection {
                    name: ".tbss".into(),
                    patterns: vec![".tbss".into(), ".tbss.*".into()],
                    vma: None,
                },
                OutputSection {
                    name: ".data".into(),
                    patterns: vec![".data".into(), ".data.*".into()],
                    vma: None,
                },
                OutputSection {
                    name: ".bss".into(),
                    patterns: vec![".bss".into(), ".bss.*".into(), "COMMON".into()],
                    vma: None,
                },
            ],
        }
    }
}

impl LinkerScript {
    pub fn parse(text: &str) -> Self {
        let mut script = Self::default();
        let mut in_sections = false;
        let mut current_out: Option<OutputSection> = None;
        let mut collected = Vec::new();

        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let upper = line.to_uppercase();
            if upper.starts_with("ENTRY(") {
                if let Some(inner) = line.strip_prefix("ENTRY(").or_else(|| {
                    // case variants
                    let idx = upper.find("ENTRY(")?;
                    Some(&line[idx + 6..])
                }) {
                    let name = inner.trim().trim_end_matches(')').trim().to_string();
                    if !name.is_empty() {
                        script.entry = Some(name);
                    }
                }
                continue;
            }
            if upper.starts_with("SECTIONS") {
                in_sections = true;
                collected.clear();
                continue;
            }
            if !in_sections {
                continue;
            }
            if line.starts_with('}') && current_out.is_none() {
                in_sections = false;
                if !collected.is_empty() {
                    script.sections = std::mem::take(&mut collected);
                }
                continue;
            }

            // .name : { ... }  or start of output section
            if let Some(rest) = line.split_once(':') {
                let name = rest.0.trim().trim_start_matches('.').trim();
                let name = if rest.0.trim().starts_with('.') {
                    rest.0.trim().to_string()
                } else {
                    format!(".{name}")
                };
                if let Some(prev) = current_out.take() {
                    collected.push(prev);
                }
                current_out = Some(OutputSection {
                    name,
                    patterns: Vec::new(),
                    vma: None,
                });
                // patterns on same line: *( .text )
                collect_patterns(rest.1, current_out.as_mut().unwrap());
                if rest.1.contains('}') {
                    if let Some(o) = current_out.take() {
                        collected.push(o);
                    }
                }
                continue;
            }
            if let Some(ref mut o) = current_out {
                collect_patterns(line, o);
                if line.contains('}') {
                    collected.push(current_out.take().unwrap());
                }
            }
        }
        if let Some(o) = current_out.take() {
            collected.push(o);
        }
        if !collected.is_empty() {
            script.sections = collected;
        }
        script
    }

    /// Match input section name against script patterns (`*` glob suffix),
    /// returning the destination output-section name. Owned `String` so the
    /// result doesn't need to borrow from either `self` or `input_name`
    /// (callers only ever hold it briefly, e.g. as a `BTreeMap` key).
    pub fn map_input_section(&self, input_name: &str) -> Option<String> {
        for out in &self.sections {
            for pat in &out.patterns {
                if section_pattern_match(pat, input_name) {
                    return Some(out.name.clone());
                }
            }
        }
        // Fallback: keep same name for known sections
        match input_name {
            ".text" | ".data" | ".rodata" | ".bss" | ".interp" | ".init_array" | ".fini_array"
            | ".preinit_array" | ".tdata" | ".tbss" => Some(input_name.to_string()),
            _ => None,
        }
    }
}

fn collect_patterns(s: &str, out: &mut OutputSection) {
    // *( .text .text.* ) or *(.text) — split on the grouping punctuation
    // only (not bare `*`, which is also a legitimate glob-suffix character
    // *inside* a pattern like `.text.*`), then split each chunk on
    // whitespace to separate individually-listed patterns.
    for chunk in s.split(|c: char| c == '(' || c == ')' || c == '{' || c == '}') {
        let chunk = chunk.trim().trim_start_matches('*').trim();
        for t in chunk.split_whitespace() {
            let t = t.trim_end_matches(',');
            if t.starts_with('.') || t == "COMMON" {
                out.patterns.push(t.to_string());
            }
        }
    }
}

fn section_pattern_match(pat: &str, name: &str) -> bool {
    if pat == name || pat == "COMMON" && name == "COMMON" {
        return true;
    }
    if let Some(prefix) = pat.strip_suffix(".*") {
        return name.starts_with(prefix);
    }
    if pat.ends_with('*') {
        let prefix = &pat[..pat.len() - 1];
        return name.starts_with(prefix);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entry_and_sections() {
        let s = LinkerScript::parse(
            r#"
            ENTRY(main)
            SECTIONS {
              .text : { *(.text .text.*) }
              .data : { *(.data) }
            }
        "#,
        );
        assert_eq!(s.entry.as_deref(), Some("main"));
        assert!(s.sections.iter().any(|o| o.name == ".text"));
        assert_eq!(s.map_input_section(".text.startup").as_deref(), Some(".text"));
    }

    #[test]
    fn default_maps_standard() {
        let s = LinkerScript::default();
        assert_eq!(s.map_input_section(".text").as_deref(), Some(".text"));
        assert_eq!(s.map_input_section(".rodata.str1.1").as_deref(), Some(".rodata"));
    }
}
