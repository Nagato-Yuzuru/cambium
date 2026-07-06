# Contributing

Solo project for now: work is tracked in issues (templates: Task / Bug / RFC),
lands via PRs with green CI, and follows Conventional Commits — the PR
template's checklist is the whole contract.

## Setup

```sh
mise install   # pinned toolchain + tools (mise.toml + rust-toolchain.toml)
just hooks     # git hooks (fmt/typos on commit; clippy/test/deny on push)
just           # lists recipes; `just ci` mirrors the CI gate
```
