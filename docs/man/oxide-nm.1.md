% OXIDE-NM(1) OxideUtils | Zainium Dynamics
% Zainium Dynamics
% 2026

# NAME

oxide-nm - list symbols from object files

# SYNOPSIS

**oxide-nm** [*options*] *file*...

# DESCRIPTION

**oxide-nm** lists symbols from ELF (and other formats supported by the core library) object files and archives. Product of **Zainium Dynamics** (OxideUtils). Not a GNU project.

# OPTIONS

**-g**, **--extern-only**
: Only external symbols.

**-u**, **--undefined-only**
: Only undefined symbols.

**-U**, **--defined-only**
: Only defined symbols.

**-C**, **--demangle**
: Demangle Rust / C++ names.

**-n**, **--numeric-sort**
: Sort by address.

**-S**, **--print-size**
: Print symbol size (GNU-compatible; **not** size-sort).

**--size-sort**
: Sort by size.

**-p**, **--no-sort**
: Symbol table order.

**-r**, **--reverse-sort**
: Reverse sort.

**-A**, **-o**, **--print-file-name**
: Prefix each line with the file name.

**-V**, **--version**
: Version banner.

# SEE ALSO

**oxide-objdump**(1), **nm**(1)

# COPYRIGHT

Copyright (C) 2026 Zainium Dynamics. Licence: GPLv3 only.
