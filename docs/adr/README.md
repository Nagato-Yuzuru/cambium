# Architecture Decision Records

Short records of decisions that shape the codebase: one file per decision,
numbered, immutable after acceptance — a superseding decision gets a new ADR
that links back. The research and rationale behind the route live in local
working documents (`docs/architecture.md`, `docs/design.md` — deliberately
untracked); ADRs capture the decision, not the research.

New ADRs normally come out of accepted RFC issues and land in the PR that
implements them (see [../../CONTRIBUTING.md](../../CONTRIBUTING.md)).

Format:

```markdown
# ADR NNNN: <title>

- Status: Accepted | Superseded by NNNN
- Date: YYYY-MM-DD

## Context
## Decision
## Consequences
```

| #                                              | Title                                                            | Status   |
| ---------------------------------------------- | ---------------------------------------------------------------- | -------- |
| [0001](0001-rust-toolchain-pinning.md)          | Rust toolchain pinned in rust-toolchain.toml, mirrored in mise.toml | Accepted |
| [0002](0002-mise-owns-toolchain-just-owns-tasks.md) | mise owns tool provisioning; just owns task definitions       | Accepted |
