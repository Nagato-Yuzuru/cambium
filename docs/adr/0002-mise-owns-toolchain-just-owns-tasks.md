# ADR 0002: mise owns tool provisioning; just owns task definitions

- Status: Accepted (rewrite of the deleted original)
- Date: 2026-07-06
- Related: `mise.toml`, `justfile`, [ADR 0001](0001-rust-toolchain-pinning.md)

## Context

Tool version pinning and task definitions both want a single home. mise could
do both (it has a `[tasks]` table); just only does tasks. Splitting arbitrarily
— or letting each consumer (CI, git hooks, humans) spell out raw cargo
commands — produces multiple places that define "how to run X" and multiple
places that can pin versions, which then drift.

## Decision

- `mise.toml` provisions every pinned tool, and nothing else: `mise install`
  is the whole setup step. Its `[tasks]` table stays empty.
- `justfile` defines every task (fmt / lint / test / deny / spell / coverage /
  bench / ci …).
- Every consumer — CI workflows, prek hooks, humans — invokes the just
  recipes, never raw tool commands, so each command is defined exactly once.
- Hooks and CI enter recipes via `mise exec -- just …`, which both puts the
  pinned tools on PATH without an activated shell and guards the
  `RUSTUP_TOOLCHAIN` footgun ([ADR 0001 §3](0001-rust-toolchain-pinning.md)).

## Consequences

- Command definitions have one home; CI steps and hook entries are one-liners
  that cannot drift from local usage.
- just itself is one more pinned tool, provisioned by mise like everything
  else.
