---
name: release
description: Guided release workflow — version bump, lint, test, build, push
user-invocable: true
disable-model-invocation: true
allowed-tools:
  - Read
  - Edit
  - Grep
  - Glob
  - Bash
---

# Release Workflow

Guided release process for pir9. This skill is **manual-only** — it only runs when explicitly invoked with `/release`.

## Steps

### 1. Read current version
```bash
grep '^version' Cargo.toml | head -1
```

### 2. Analyze recent commits
```bash
git log --oneline $(git describe --tags --abbrev=0 2>/dev/null || echo HEAD~20)..HEAD
```

Determine bump type from conventional commit prefixes:
- `fix:` → patch (0.11.1 → 0.11.2)
- `feat:` → minor (0.11.1 → 0.12.0)
- Breaking changes → major (0.11.1 → 1.0.0)

### 3. Bump version

Update version in these files:
- `Cargo.toml` (line 3)
- `services/pir9-imdb/Cargo.toml` (if IMDB service was changed)
- `CLAUDE.md` "Current version" line

### 4. Run quality checks
```bash
# Rust
cargo clippy -- -D warnings
cargo test

# Frontend
cd frontend && npm run lint && npm run typecheck
```

### 5. Build release
```bash
make release
```

This builds the Docker image with the release tag.

### 6. Push to registry
```bash
make push
```

Pushes to `reg.pir9.org:2443/pir9:latest`.

### 7. Tag the release
```bash
git tag -a v{VERSION} -m "Release v{VERSION}"
git push origin v{VERSION}
```

## Checklist

Before proceeding with each step, verify:
- [ ] All tests pass
- [ ] No clippy warnings
- [ ] Frontend lint and typecheck clean
- [ ] Version bumped in all required files
- [ ] Commit message follows conventional commits format
- [ ] Docker build succeeds

## Notes

- Always ask for confirmation before pushing to the registry or creating tags
- If any check fails, stop and report the issue — do not proceed with a broken release
- The `make release` target handles the full Docker build including frontend
