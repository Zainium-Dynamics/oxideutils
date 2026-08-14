# Contributing to OxideUtils (Zainium Dynamics)

Thank you for contributing to **OxideUtils**, a product of **[Zainium Dynamics](https://zainiumdynamics.tech)**.

**Repository:** [github.com/Zainium-Dynamics/oxideutils](https://github.com/Zainium-Dynamics/oxideutils)

## Ownership

- This is **not** a GNU Project package.  
- Product and trademark: **Zainium Dynamics** / **OxideUtils**.  
- Contact: **alizain@zainiumdynamics.tech**

## Licence

Contributions are under **GNU GPLv3 only**. By submitting a patch you agree your contribution is licensed under GPLv3-only for distribution as part of OxideUtils by Zainium Dynamics.

## Development setup

```bash
cd oxideutils
cargo build --release   # config: oxideutils.toml
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check --workspace
```

## Guidelines

1. Prefer GNU-compatible CLI/behaviour unless a documented Oxide/Zainium enhancement.  
2. Keep tools thin; put logic in `oxideutils-core`.  
3. Keep **kernel path** (`no_std` + `alloc`) compiling — run `cargo oxide-kernel` (or the cargo line in docs/building.md) before large core changes.  
4. Add tests for new flags or format paths when practical.  
5. Do not rebrand as GNU; version strings stay Zainium Dynamics.  
6. Document user-facing changes in `docs/` when needed.

## Pull request workflow

1. Fork the repository.
2. Create a feature branch: `git checkout -b feat/my-feature`.
3. Make your changes and commit with conventional-style messages.
4. Push and open a PR against `main`.
5. CI must pass before merge.

## Documentation

- User docs: `docs/` (see `docs/README.md`)  
- Root overview: `README.md`  
- Architecture: `ARCHITECTURE.md`
- Update docs when you change CLI flags or public core API  

## Patch checklist

- [ ] `cargo test --workspace`  
- [ ] `cargo oxide-kernel` if you touched core  
- [ ] `cargo fmt --all`  
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] Docs if user-visible

