% OXIDE-READELF(1) OxideUtils | Zainium Dynamics
% Zainium Dynamics
% 2026

# NAME

oxide-readelf - display information about ELF files

# SYNOPSIS

**oxide-readelf** [*options*] *file*...

# DESCRIPTION

**oxide-readelf** displays structural information from ELF object files, executables, and shared libraries. It is a memory-safe, GNU-readelf-oriented tool from **OxideUtils** (Zainium Dynamics). It is **not** a GNU project.

Behavioural reference: GNU binutils **2.46.1**.

# OPTIONS

**-h**, **--file-header**
: Display the ELF file header.

**-H**, **--help**
: Show help (GNU-style: **-h** is the file header).

**-S**, **--section-headers**
: Display section headers. Lists SHF_COMPRESSED sections when present.

**-l**, **--program-headers**
: Display program headers (segments).

**-s**, **--symbols**
: Display symbol tables.

**-d**, **--dynamic**
: Display the dynamic section (resolves NEEDED/SONAME strings).

**-r**, **--relocs**
: Display relocations with type names and symbol names where known.

**-n**, **--notes**
: Display notes (e.g. GNU build-id).

**-V**, **--version-info**
: Display symbol versioning (`.gnu.version`, `.gnu.version_r`, `.gnu.version_d`).

**--got-contents**
: Display Global Offset Table section contents (GNU binutils 2.46).

**--sframe**\[=**NAME**\]
: Display SFrame stack-trace section header (default **.sframe**). Full FRE decode is not yet implemented.

**-u**, **--unwind**
: Summarise `.eh_frame` / `.eh_frame_hdr` (not a full CIE/FDE dump).

**-a**, **--all**
: Equivalent to **-h -l -S -s -r -d -n -V -u --got-contents**.

**-v**, **--version**
: Print OxideUtils / Zainium Dynamics version banner.

# EXIT STATUS

**0** success · **1** operational error · **2** usage error

# EXAMPLES

```
oxide-readelf -h -S /bin/ls
oxide-readelf -V --got-contents -u /bin/ls
oxide-readelf -a /lib64/libc.so.6
```

# SEE ALSO

**oxide-objdump**(1), **oxide-nm**(1), **readelf**(1)

# COPYRIGHT

Copyright (C) 2026 Zainium Dynamics. Licence: GPLv3 only.
