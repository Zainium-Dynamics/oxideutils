% oxide-objdump(1) OxideUtils 0.1 | Zainium Dynamics
% Zainium Dynamics
% 2026

# NAME

oxide-objdump — display information from object files (OxideUtils)

# SYNOPSIS

**oxide-objdump** [*options*] *file*...

# DESCRIPTION

**oxide-objdump** is part of **OxideUtils**, a product of **Zainium Dynamics**.
It displays headers, symbols, section contents, and disassembly for object files
and archives. It is designed for GNU binutils *compatibility*, but is **not** a
GNU program.

# OPTIONS

See **oxide-objdump --help** and *docs/tools.md* in the source tree for the full list.

Notable:

- **-h** section headers; **-H** help  
- **-d** / **-D** disassemble (x86/x86_64 via iced-x86)  
- **-t** symbols; **-r** relocations (pretty type + symbol)  
- **-x** all headers (+ ELF dynamic / notes / versions)  
- **--sframe**\[=**SEC**\] SFrame header summary  

Behavioural reference: GNU binutils **2.46.1**.

# SEE ALSO

oxide-nm(1), oxide-readelf(1), oxideutils(1)

# AUTHOR

Zainium Dynamics \<alizain@zainiumdynamics.tech\>

# COPYRIGHT

Copyright 2026 Zainium Dynamics. Licence GPLv3 only.
