# OxideUtils release process

Manual checklist for cutting a release. CI does **not** create tags or
GitHub Releases.

**Repository:** [github.com/Zainium-Dynamics/oxideutils](https://github.com/Zainium-Dynamics/oxideutils)

Supported tag examples:
- `v0.1.0`
- `v0.1.0-alpha`
- `v0.2.0-rc.1`

## 1) Pre-flight checks

Run from repo root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

## 2) Version

Ensure the workspace version in `Cargo.toml` matches the intended
release (tag `v0.1.0-alpha` corresponds to crate version `0.1.0`).

## 3) Commit

```bash
git add -u
git commit -m "release: prepare $VERSION"
```

Do not add any `Co-authored-by:` trailer.

## 4) Create an annotated tag

```bash
git tag -a "v$VERSION" -m "OxideUtils v$VERSION"
git show --no-patch --pretty=fuller "v$VERSION"
```

## 5) Push the branch and the tag

```bash
git push origin main
git push origin "v$VERSION"
```

Pushing the tag does **not** publish a GitHub Release.

## 6) Publish the GitHub Release

Create the release on GitHub for that tag (web UI or `gh`):

```bash
gh release create "v$VERSION" \
  --title "OxideUtils v$VERSION" \
  --notes "Source snapshot for v$VERSION." \
  --verify-tag
```

Add `--prerelease` for `-alpha`, `-beta`, or `-rc` tags.

Publishing the release triggers `.github/workflows/release.yml`, which
attaches:

- `dist/oxideutile-<tag>.tar.zst` — full workspace source (`git archive`)
- `dist/oxideutile-<tag>.sha256` — SHA-256 of that archive

`target/` and other generated files are not in the archive.

## 7) Verify assets

```bash
TAG="v0.1.0-alpha"
gh release download "$TAG" --pattern "oxideutile-${TAG}.*"
sha256sum -c "oxideutile-${TAG}.sha256"
```

If something is wrong, fix it and cut a new patch tag (for example
`v0.1.1`). Do not move an existing pushed tag.
