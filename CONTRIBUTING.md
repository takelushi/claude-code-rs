# Contributing

## Commit message format

This project follows [Conventional Commits](https://www.conventionalcommits.org/).

```
<type>: <description> (#<issue>)

<body>

<footer>
```

### Types

| Type | Purpose |
|------|---------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `ci` | CI/CD changes |
| `test` | Test additions or corrections |
| `refactor` | Code change that neither fixes a bug nor adds a feature |
| `chore` | Maintenance tasks |

### Issue references

When a commit is related to a GitHub issue:

- Add `(#N)` to the subject line for traceability
- Use `Refs #N` in commit footers or PR bodies to link without closing (for all work on `develop`)
- Use `Closes #N` in **`develop` → `main`** PR bodies to auto-close on merge

This two-step approach ensures issues are visibly linked from the start, while only closing when changes reach `main`.

```
feat: add cli_path option (#5)

Allow users to specify a custom path to the claude CLI binary.

Refs #5
```

## Branch policy

**Only `develop` → `main` merges are allowed.** All feature branches and fixes must go through `develop` first.

```
feature/* ──→ develop ──→ main
hotfix/*  ──→ develop ──→ main
```

This is enforced by CI (`enforce-branch-policy.yml`) and branch protection rules on `main`.

### Merge strategy

`develop` → `main` merges **must use "Create a merge commit"** (no squash, no rebase). This preserves individual commit messages so that release-please can analyze conventional commit prefixes (`feat:`, `fix:`, etc.) to determine version bumps and generate changelogs. Squash merging would collapse all commits into a single `chore:` commit, causing release-please to miss releasable changes.

This is enforced in the GitHub repository settings (Settings → General → Pull Requests → only "Allow merge commits" enabled for `main`).
### PR title for develop → main merges

Use `chore: merge develop into main` as the PR title. Since these PRs bundle multiple change types (feat, fix, docs, ci, etc.), `chore` is the appropriate prefix.

## Development

See [CLAUDE.md](CLAUDE.md) for build commands, testing strategy, and code conventions.

See [docs/releasing.md](docs/releasing.md) for release workflow and version bump rules.
