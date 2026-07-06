## What & why

<!-- What does this change do, and why? Link any related issue (e.g. Closes #12). -->

## Checklist

- [ ] `just ci` passes locally (fmt, clippy, tests, `cargo deny`, typos)
- [ ] Tests cover the change: a positive case asserting concrete values, a negative case asserting the specific error, and boundaries (empty / one / many / off-by-one)
- [ ] Public items have doc comments stating panics, units, and invariants
- [ ] No new dependency — or a new one is justified and passes `cargo deny`
- [ ] Commits follow Conventional Commits (`type: summary`)
