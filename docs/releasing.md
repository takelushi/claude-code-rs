# Releasing

## Overview

Releases are automated via [release-please](https://github.com/googleapis/release-please). The workflow:

1. Merge PRs to `main` using [Conventional Commits](https://www.conventionalcommits.org/)
2. release-please bot creates/updates a Release PR on `main`
3. Merging the Release PR triggers: GitHub Release + tag creation → `cargo publish`

## Commit message format

release-please uses commit messages to determine version bumps:

| Prefix | Version bump | Example |
|--------|-------------|---------|
| `fix:` | patch (0.1.0 → 0.1.1) | `fix: handle empty response` |
| `feat:` | minor (0.1.0 → 0.2.0) | `feat: add timeout config` |
| `feat!:` or `BREAKING CHANGE:` footer | major (0.1.0 → 1.0.0) | `feat!: redesign config API` |
| `chore:`, `docs:`, `ci:`, `test:`, `refactor:` | no release | `docs: update README` |

While the crate is pre-1.0, `release-please-config.json` is configured with:
- `bump-minor-pre-major: true` — breaking changes bump minor, not major
- `bump-patch-for-minor-pre-major: true` — features bump patch, not minor

## Branch protection

`main` branch requires the following status checks to pass:

- Test (Rust stable) — default, no-default-features, all-features
- Clippy (all-features)
- Rustfmt
- Publish dry-run

Configured via GitHub API. Admin enforcement is enabled.

## GitHub Secrets

| Secret | Purpose |
|--------|---------|
| `CARGO_REGISTRY_TOKEN` | crates.io API token for `cargo publish` |

## Configuration files

| File | Purpose |
|------|---------|
| `.github/workflows/release-please.yml` | release-please action + publish job |
| `release-please-config.json` | Release-please settings (release type, pre-1.0 bump behavior) |
| `.release-please-manifest.json` | Tracks current version (updated by release-please) |

## Manual release (escape hatch)

If release-please fails or you need to release manually:

```sh
# 1. Bump version in Cargo.toml
# 2. Update CHANGELOG.md
# 3. Commit and tag
git tag v0.x.y
git push origin v0.x.y
# 4. Publish
cargo publish --all-features
```

Note: manual tags will NOT trigger `release-please.yml` (it only runs on `main` push). You must run `cargo publish` locally.
