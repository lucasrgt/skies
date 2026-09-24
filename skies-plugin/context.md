This project uses **Skies**: an opinionated convention bundle for .NET, React, and Flutter (the Rails mindset
without the Rails mechanism). Vertical slices and marked domain types on the backend, MVVM with generated typed
clients on the frontend, architecture doctors (SKY*, SKYFE*, SKYFL*), and one `skies` CLI.

How work is delivered: **spec first, evidence at the end.** Every feature gets `.specs/<id>-<slug>/` with the
failure modes written before the code, black-box E2E in `e2e/`, and a `receipt.json` from `skies proof record`.
Follow the `skies-sdd` skill for any feature, endpoint, or screen.

Hard rules:
- Generate shapes with `skies g …` before hand-writing boilerplate.
- One slice = one file with Input/Output/Handle/Map. Handlers use AppDb directly; never a repository or unit of work.
- A module writes only its own entities; reference other modules by id, never an EF foreign key.
- Error codes are `*ErrorCodes` constants; user-facing copy lives in the frontend i18n catalogs.
- Views never touch data; only ViewModels consume the generated client.
- Every test lives in a spec (`.specs/<id>-<slug>/e2e/`, titled `FM-n: …`), nowhere else. Never write tests after
  the code to cover it. An isolated system gets its own spec with isolated cases, failure modes written first.
- `skies doctor` findings are fixed in the code, never suppressed.

Nothing runs automatically: there is no gate and no required hook. `skies proof status` shows receipts whose files
changed; rerun them with `skies proof verify` when the change could affect them.

Reference: `docs/backend-conventions.md`, `docs/frontend-conventions.md`, `docs/doctor-rules.md`, `docs/cli.md`,
`docs/decisions.md` in this plugin.
