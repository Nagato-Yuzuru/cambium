# Contributing to Cambium

Cambium runs on an issue-first, TDD-first workflow. This document is the
contract for how changes land. Deep design rationale lives in local working
documents (`docs/architecture.md`, `docs/design.md` — deliberately untracked);
the committed record of decisions is [docs/adr/](docs/adr/), and issues carry
English summaries of whatever design context they need.

## Setup

```sh
mise install   # provisions the pinned toolchain and tools (mise.toml + rust-toolchain.toml)
just hooks     # installs prek git hooks (fmt/typos on commit; clippy/test/deny on push)
just           # lists all recipes; `just ci` mirrors the CI gate
```

See [docs/adr/0001](docs/adr/0001-rust-toolchain-pinning.md) for why the Rust
toolchain is pinned in two places, and
[docs/adr/0002](docs/adr/0002-mise-owns-toolchain-just-owns-tasks.md) for the
mise/just split.

## Workflow

1. **Every change starts from an issue.** Pick one or open one with the
   matching template (Task / Bug report / RFC). Issues carry acceptance
   criteria and belong to a milestone; milestones mirror the project's week
   plan.
2. **Branch per issue**: `<type>/<issue>-<slug>`, e.g. `feat/4-reader`.
   `gh issue develop <n> --name <branch> --checkout` links it to the issue.
3. **TDD** (next section). Run `just ci` before pushing.
4. **Pull request** with `Closes #<n>`. CI, autofix.ci and CodeRabbit run on
   every PR; squash-merge when green.
5. **Conventional Commits** subjects (`feat:`, `fix:`, `test:`, `docs:`,
   `perf:`, `ci:`, `chore:`), scope = crate short name: `feat(reader): …`.

## TDD policy

- **Red → green → refactor.** Write the failing test first and watch it fail;
  only then implement. Commits and pushes are always green (the pre-push hook
  runs the suite).
- **A behavior is tested when it has all three**: a positive case asserting
  specific values, a negative case asserting the specific error variant, and
  boundary cases (empty / one / many / off-by-one).
- **Language-level features are corpus-first** once the vertical-slice
  checkpoint lands: add `crates/cambium/tests/corpus/<feature>.scm` with
  `;; => expected` expectations, watch it fail, then implement.
- **Snapshots are regression nets, not TDD drivers.** insta snapshots (datum
  trees, CoreExpr pretty-print, disassembly) are added after the behavior is
  green and reviewed with `cargo insta review`.

## RFCs and ADRs

Proposal and record are two artifacts:

- **RFC = an issue** using the RFC template. Required whenever a change moves
  a crate boundary, deviates from R7RS-small, or picks between
  architecture-level alternatives. The issue thread is the discussion archive.
- **ADR = the decision record** in [docs/adr/](docs/adr/), added by the PR
  that implements the accepted RFC (next number, short:
  Status/Context/Decision/Consequences).

Decisions below that bar go straight into code comments, in the same change
that makes them true.
