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
- Add `Closes #N` in the footer to auto-close the issue when merged to `main`

```
feat: add cli_path option (#5)

Allow users to specify a custom path to the claude CLI binary.

Closes #5
```

## Branch policy

**Only `develop` → `main` merges are allowed.** All feature branches and fixes must go through `develop` first.

```
feature/* ──→ develop ──→ main
hotfix/*  ──→ develop ──→ main
```

This is enforced by CI (`enforce-branch-policy.yml`) and branch protection rules on `main`.

### PR title for develop → main merges

Use `chore: merge develop into main` as the PR title. Since these PRs bundle multiple change types (feat, fix, docs, ci, etc.), `chore` is the appropriate prefix.

## Development

See [CLAUDE.md](CLAUDE.md) for build commands, testing strategy, and code conventions.

See [docs/releasing.md](docs/releasing.md) for release workflow and version bump rules.
