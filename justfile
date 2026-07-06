# Cambium task runner. These recipes are the project's task entrypoints; `mise`
# owns the pinned toolchain (see mise.toml, and docs/adr/0002 for the split).
#
# Tool commands run under `mise exec --` so the rust-toolchain.toml-pinned compiler
# is used even when `just` is invoked outside an activated mise shell — this guards
# the RUSTUP_TOOLCHAIN-shadowing footgun documented in docs/adr/0001 § 3.
set shell := ["bash", "-euo", "pipefail", "-c"]

# List available recipes (default when `just` is run with no arguments).
default:
    @just --list

# Format all code.
fmt:
    cargo fmt --all

# Check formatting without writing.
fmt-check:
    cargo fmt --all -- --check

# Lint with clippy (levels come from [workspace.lints]).
lint:
    cargo clippy --all-targets --all-features --workspace

# Auto-fix what clippy and rustfmt can fix in place (used by autofix.ci in CI).
fix:
    cargo clippy --fix --all-targets --all-features --workspace --allow-dirty --allow-no-vcs
    cargo fmt --all

# Run the test suite (nextest).
test:
    cargo nextest run --workspace

# Coverage as LCOV (llvm-cov over nextest) for Codecov; writes ./lcov.info.
coverage:
    cargo llvm-cov nextest --workspace --lcov --output-path lcov.info

# Spell-check the source tree.
spell:
    typos

# Audit dependencies: licenses, advisories, bans, sources.
deny:
    cargo deny check

# Run benchmarks (populated in W6).
bench:
    cargo bench --workspace

# Install prek git hooks (pre-commit + pre-push).
hooks:
    prek install --install-hooks

# Full local gate — mirrors .github/workflows/ci.yml.
ci: fmt-check lint test deny spell
