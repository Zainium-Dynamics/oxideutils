% OXIDE-AR(1) OxideUtils | Zainium Dynamics
% Zainium Dynamics
% 2026

# NAME

oxide-ar - create, modify, and extract from archives

# SYNOPSIS

**oxide-ar** [**-**]**{dmpqrstx}**\[**cDsuv**\] *archive* [*member*...]

# DESCRIPTION

**oxide-ar** is a GNU **ar**-oriented archive tool from OxideUtils (Zainium Dynamics). Supports create with symbol index (`rcs`), list, extract, delete, and ranlib-style index rebuild.

# OPERATIONS

| Key | Meaning |
|-----|---------|
| **r** | Replace or add members |
| **q** | Quick append |
| **d** | Delete members |
| **t** | Table of contents |
| **p** | Print members to stdout |
| **x** | Extract members |
| **s** | Write / rebuild symbol index (ranlib) |

# MODIFIERS

**c** — create archive (suppress “creating” noise in GNU; Oxide always creates as needed)  
**v** — verbose  
**D** — deterministic (zero timestamps / uids)

# EXAMPLES

```
oxide-ar rcs libfoo.a a.o b.o
oxide-ar t libfoo.a
oxide-ar d libfoo.a a.o
oxide-ar s libfoo.a
```

# SEE ALSO

**ar**(1), **ranlib**(1), **oxide-nm**(1)

# COPYRIGHT

Copyright (C) 2026 Zainium Dynamics. Licence: GPLv3 only.
