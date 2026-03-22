# Contributing to oxidize

Thank you for your interest in contributing! This document describes our branching strategy, commit conventions, and coding guidelines.

---

## Branching Strategy

| Branch | Purpose |
|---|---|
| `main` | Always green, always releasable. Direct pushes are forbidden. |
| `dev` | Integration branch. Feature branches merge here first. |
| `feature/<slug>` | New features or significant changes. Branch from `dev`. |
| `fix/<slug>` | Bug fixes. Branch from `dev` (or `main` for hot-fixes). |
| `chore/<slug>` | Tooling, CI, documentation, dependency bumps. |

### Workflow

```
feature/<slug>  →  dev  →  main
```

1. Branch from `dev`: `git checkout -b feature/my-feature dev`
2. Develop, commit, push.
3. Open a PR targeting `dev`.
4. After review, merge with **squash-and-merge** (preserves a linear history on `dev`).
5. Periodically, `dev` is merged into `main` via a release PR.

---

## Commit Conventions

We follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/).

### Format

```
<type>(<scope>): <short summary>

[optional body]

[optional footer(s)]
```

### Types

| Type | When to use |
|---|---|
| `feat` | A new user-visible feature |
| `fix` | A bug fix |
| `refactor` | Internal restructuring with no behaviour change |
| `test` | Adding or fixing tests |
| `chore` | Build system, CI, dependency updates |
| `docs` | Documentation only |
| `perf` | Performance improvement |
| `style` | Code style (whitespace, formatting): no logic change |

### Scopes

Use the crate name as the scope: `parser`, `ir`, `typeck`, `codegen`, `runtime`, `cli`, `tests`, `ci`.

### Examples

```
feat(parser): lower ForEach statements to IrStmt::ForEach
fix(runtime): correct JArray bounds check for negative indices
test(codegen): add differential test for while-loop translation
chore(ci): pin ubuntu-latest to ubuntu-24.04 in matrix
```

---

## Code Style

- Run `cargo fmt` before every commit (`cargo fmt --all`).
- All `cargo clippy -- -D warnings` warnings must be resolved; do not suppress with `#[allow]` without a comment explaining why.
- No `unsafe` code without a visible `// SAFETY:` comment and a review from a second contributor.

---

## Pull Request Checklist

Before requesting review, verify:

- [ ] `cargo fmt --all` passes
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` passes
- [ ] `cargo test --all-targets` passes
- [ ] New public items have doc-comments
- [ ] Any `unsafe` block has a `// SAFETY:` justification

---

## Setting Up the Development Environment

```bash
# 1. Install Rust via rustup (https://rustup.rs)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. Clone the repository
git clone https://github.com/YOUR_USERNAME/oxidize.git
cd oxidize

# 3. Build everything
cargo build --all-targets

# 4. Run tests
cargo test --all-targets

# 5. Install cargo-nextest (faster test runner, used in differential tests)
cargo install cargo-nextest --locked
```

---

## Making a Release

Releases are automated via `.github/workflows/release.yml`. Pushing a `v*` tag triggers a four-platform build (Linux x86_64, Windows x86_64, macOS Intel, macOS Apple Silicon) and creates a GitHub Release with the binaries attached.

### Version scheme

We follow [Semantic Versioning](https://semver.org/): `vMAJOR.MINOR.PATCH`.

Pre-release suffixes (`alpha`, `beta`, `rc`) are automatically marked as GitHub pre-releases:
- `v0.3.0-alpha.1` — early preview, may be unstable
- `v0.3.0-beta.1` — feature-complete, still being tested
- `v0.3.0-rc.1` — release candidate; only critical fixes accepted
- `v0.3.0` — stable release

### Release checklist

1. **Ensure `main` is green.** All CI checks on `main` must pass before tagging.

2. **Update `Cargo.toml` versions.** Bump the `version` field in the root `Cargo.toml` (and any crate `Cargo.toml` files that are published independently):
   ```bash
   # Example: bump to 0.3.0
   sed -i 's/^version = ".*"/version = "0.3.0"/' Cargo.toml
   cargo build --all-targets   # make sure Cargo.lock updates cleanly
   ```

3. **Commit the version bump.**
   ```bash
   git add Cargo.toml Cargo.lock
   git commit -m "chore: bump version to 0.3.0"
   git push origin main
   ```

4. **Tag and push.**
   ```bash
   git tag v0.3.0
   git push origin v0.3.0
   ```
   For a pre-release:
   ```bash
   git tag v0.3.0-alpha.1
   git push origin v0.3.0-alpha.1
   ```

5. **Verify the release.** GitHub Actions will run the release workflow. Once it completes (typically 10–15 minutes), check the [Releases page](../../releases) to confirm all four binaries are attached and the release notes were generated.

6. **Announce** (if this is a stable release): update the README badge if the `latest-release` badge version is hard-coded, and post a note in any relevant discussion channels.

### Hotfixes

If a critical bug is found in a released version:

1. Branch from the release tag: `git checkout -b fix/critical-bug v0.3.0`
2. Apply the fix, add a test, commit.
3. Merge back into `main` (and `dev`) via PR.
4. Tag a patch release: `v0.3.1`.

---

## Reporting Issues

Use GitHub Issues. Tag with one of: `bug`, `enhancement`, `question`, `stage-0` … `stage-N`.
