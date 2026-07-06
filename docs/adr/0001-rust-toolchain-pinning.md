# ADR 0001: Rust toolchain pinned in rust-toolchain.toml, mirrored in mise.toml

- Status: Accepted (rewrite of the deleted original)
- Date: 2026-07-06
- Related: `mise.toml` `[tools].rust`, `justfile` header, [ADR 0002](0002-mise-owns-toolchain-just-owns-tasks.md)

## 1. Context

Reproducible builds need exactly one pinned compiler, but two tools can claim
that job: rustup (via `rust-toolchain.toml`) and mise (via `[tools].rust`).
rustup's file is honored natively by CI runners, rust-analyzer, and every
cargo invocation, so it is the natural authority. However, a developer machine
may carry a *global* mise configuration that also pins `rust` — and mise
implements its Rust support by exporting `RUSTUP_TOOLCHAIN`, which overrides
`rust-toolchain.toml` wherever that variable is set.

## 2. Decision

- `rust-toolchain.toml` is the single authoritative toolchain pin (channel and
  components).
- `mise.toml` repeats the same version, solely so the project-level mise
  config shadows any ambient global mise `rust` pin with the correct value.
  The two files must be bumped together.

## 3. The RUSTUP_TOOLCHAIN shadowing footgun

With a global mise `rust` pin active, any mise-activated shell exports
`RUSTUP_TOOLCHAIN=<global version>`. cargo then silently ignores
`rust-toolchain.toml` — builds run on the wrong compiler with no warning.
Running commands through `mise exec --` (as the justfile and the prek hooks
do) re-enters mise with the project config, whose `[tools].rust` overrides the
global pin and restores the version this repository expects. Outside
`just`/`mise exec`, either work in a shell where the project mise config is
active, or run `env -u RUSTUP_TOOLCHAIN cargo …`.

## 4. Consequences

- CI (mise-action + rustup) and local builds agree on the compiler regardless
  of the developer's global mise state.
- Cost: the version lives in two files. A bump must touch both; drift shows up
  as a local/CI compiler mismatch, so keep the pair in one commit.
