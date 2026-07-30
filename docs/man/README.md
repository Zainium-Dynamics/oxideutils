# Man pages (source)

Markdown sources for OxideUtils manual pages (Zainium Dynamics).

Convert with pandoc when packaging, e.g.:

```bash
pandoc oxide-objdump.1.md -s -t man -o oxide-objdump.1
```

| Page | Tool |
|------|------|
| oxide-objdump.1.md | oxide-objdump |
| oxide-readelf.1.md | oxide-readelf |
| oxide-nm.1.md | oxide-nm |
| oxide-ar.1.md | oxide-ar |
| *(remaining tools — extend from docs/tools.md)* | |

Install example:

```bash
cp oxide-objdump.1 /usr/local/share/man/man1/
mandb   # if required
man oxide-objdump
```
