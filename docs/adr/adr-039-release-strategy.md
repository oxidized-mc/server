# ADR-039: Release Strategy & Automation

| Field    | Value                        |
|----------|------------------------------|
| Status   | Accepted                     |
| Date     | 2026-03-22                   |
| Phases   | All                          |
| Deciders | Oxidized Core Team           |

## Context

Oxidized has a solid CI pipeline (lint, test×3, cargo-deny, MSRV, security audit) and
enforces conventional commits, but has **no release automation**. All seven crates sit at
`0.1.0`, there are no git tags, no prebuilt binaries, no automated changelog generation,
and no development releases. As the project matures, we need a repeatable, automated
release pipeline that:

1. Determines version bumps from conventional commit messages
2. Generates changelogs automatically
3. Produces development (nightly) binaries on every push to `main`
4. Produces stable releases via a deliberate review-and-merge flow
5. Builds cross-platform binaries and attaches them to GitHub Releases

## Decision Drivers

- **Rust-native tooling preferred** — avoid pulling in Node.js/Python runtimes in CI where
  a Rust tool exists
- **Conventional commits already enforced** — leverage existing discipline
- **Workspace versioning** — all crates share one version via `workspace.package.version`
- **No crates.io publishing yet** — binary distribution is the priority (server, not library)
- **GitHub-native** — all CI/CD runs on GitHub Actions; prefer first-party integrations
- **Minimal manual intervention** — maintainers should review, not operate

## Considered Options

### 1. semantic-release (Node.js)
The dominant JS ecosystem tool. Very mature, plugin-based.
- ❌ Requires Node.js runtime in CI
- ❌ Plugin ecosystem is JS-centric; Cargo.toml support is a community plugin
- ❌ Heavier dependency footprint for a Rust project

### 2. cargo-release
Rust-native, handles workspace versioning and crates.io publishing.
- ✅ Rust-native, understands Cargo workspaces
- ❌ Designed for manual/semi-automated use (CLI tool, not CI-first)
- ❌ No release PR concept — changes version directly on branch
- ❌ No built-in changelog generation

### 3. cargo-dist
Rust binary distribution tool (Axo). Handles cross-compilation, installers, GitHub Releases.
- ✅ Excellent binary distribution with installers (shell, PowerShell, Homebrew, MSI)
- ❌ Opinionated about project structure (wants to own the release workflow)
- ❌ Overkill for pre-1.0 project; better suited for stable library+binary distribution
- 🔄 Can be adopted later for crates.io publishing + installer generation

### 4. release-please + git-cliff (chosen)
Google's release-please for PR management + git-cliff for changelog generation.
- ✅ release-please is GitHub-native (Action), understands Rust/Cargo workspaces
- ✅ git-cliff is Rust-native, deeply configurable, conventional commit first-class
- ✅ Release PR model: accumulates changes, maintainer reviews before release
- ✅ Automatic version bump calculation from commit types
- ✅ Clean separation: git-cliff owns changelog format, release-please owns workflow

## Decision

**Use release-please for automated release PR management and git-cliff for changelog
generation.** Build cross-platform binaries via GitHub Actions matrix builds.

### Version Scheme

- **Semantic Versioning 2.0.0** — `MAJOR.MINOR.PATCH[-PRERELEASE]`
- **Pre-1.0 rule:** Breaking changes bump **minor** (0.1.0 → 0.2.0), per SemVer spec
- **Workspace-unified:** All crates share one version number

### Bump Rules from Conventional Commits

| Commit prefix | Bump |
|--------------|------|
| `feat!:` or `BREAKING CHANGE:` footer | Minor (pre-1.0) / Major (post-1.0) |
| `feat(scope):` | Minor |
| `fix(scope):`, `perf(scope):` | Patch |
| `refactor`, `test`, `docs`, `chore`, `ci` | No version bump |

### Two Release Flows

**Development releases** — Every push to `main` triggers a rolling `nightly` pre-release
with cross-platform binaries. Version: `{next}-dev.{run_number}`.

**Stable releases** — release-please accumulates conventional commits into a release PR.
When a maintainer merges that PR, release-please creates a git tag (`v0.X.Y`) and GitHub
Release. A downstream workflow builds and attaches binaries.

### Build Targets

| Target triple | OS | Archive format |
|---------------|----|---------------|
| `x86_64-unknown-linux-gnu` | Ubuntu | `.tar.gz` |
| `x86_64-unknown-linux-musl` | Ubuntu (static) | `.tar.gz` |
| `x86_64-pc-windows-msvc` | Windows | `.zip` |
| `x86_64-apple-darwin` | macOS Intel | `.tar.gz` |
| `aarch64-apple-darwin` | macOS Apple Silicon | `.tar.gz` |

### Docker Images

Container images are published to GitHub Container Registry (GHCR) at
`ghcr.io/dodoflix/oxidized`.

**Nightly images** — built after every successful CI run on `main`. Tagged as
`:nightly` (rolling) and `:sha-<7chars>` (pinnable).

**Stable images** — built when a GitHub Release is published. Tagged with full
version (`:0.2.0`), minor (`:0.2`), major (`:0`), and `:latest`.

The Dockerfile uses a multi-stage build:
1. **Build stage:** `rust:1-bookworm` — compiles the server binary
2. **Runtime stage:** `debian:bookworm-slim` — minimal image with just the
   binary, `ca-certificates`, and a non-root `oxidized` user

The container exposes port `25565/tcp` and uses `/data` as the working
directory volume for persistent world data.

### Tools

| Tool | Version | Purpose |
|------|---------|---------|
| git-cliff | latest | Changelog generation from conventional commits |
| release-please | v4 | Release PR management, version bumping, tag creation |
| commitlint | latest | Conventional commit enforcement in CI |
| Docker (buildx) | v6 | Multi-stage container image builds |

## Consequences

### Positive

- Every push to `main` produces downloadable dev binaries (fast feedback for testers)
- Docker images provide a zero-install deployment option
- Stable releases are deliberate (maintainer reviews the release PR)
- Changelog is always up-to-date and consistent
- Version bumps are deterministic from commit messages
- Cross-platform binaries available for all major OS/arch combinations

### Negative

- release-please is a Google-maintained JS tool (not Rust-native), adding a Node.js
  dependency to CI
- Two changelog tools (git-cliff for format, release-please for PR content) could drift —
  mitigated by using release-please's `changelog-type: github` and git-cliff for the
  committed CHANGELOG.md
- Rolling `nightly` tag means GitHub won't notify watchers of each dev release

### Neutral

- Adds ~4 new workflow files to `.github/workflows/`
- crates.io publishing deferred to a future enhancement
- Artifact signing deferred to post-1.0

## Compliance

- [ ] `cliff.toml` exists at repo root and `git-cliff --dry-run` succeeds
- [ ] `release-please-config.json` and `.release-please-manifest.json` exist
- [ ] `.github/workflows/release-please.yml` runs on push to main
- [ ] `.github/workflows/dev-release.yml` produces nightly pre-releases
- [ ] `.github/workflows/release-binaries.yml` builds 5 targets on stable release
- [ ] `.github/workflows/docker.yml` builds and pushes images to GHCR
- [ ] `.github/workflows/commit-lint.yml` validates PR commit messages
- [ ] All version bumps are deterministic from conventional commit prefixes

## Related ADRs

- [ADR-002](adr-002-error-handling.md) — Error handling (all crates)
- [ADR-003](adr-003-crate-architecture.md) — Crate workspace architecture
- [ADR-034](adr-034-testing-strategy.md) — Testing strategy (CI pipeline)
