# .specs

One folder per feature spec, named `<id>-<slug>` (`0001-auth`, `0002-cancel-reservation`). `skies spec new <slug>`
creates the next one.

```
.specs/0002-cancel-reservation/
  spec.md        goal, behavior, failure modes (FM-1..n), out of scope; frontmatter names the runner
  e2e/           black-box cases, one or more per failure mode, titled "FM-n: ..."
  receipt.json   written by `skies proof record`: the cases failed before the change and pass after it
```

The E2E cases here compile into the tests project (namespace `Specs.S<id>`). A case maps to a failure mode by
its title prefix, nothing else: every FM needs at least one case, and every `FM-*` case must be in `spec.md`.
