# Golden

A [Skies](https://skies.build) application: a vertical-slice .NET backend whose architecture is checked at build time,
with every feature accepted by a spec and its receipt.

## Run it

```bash
dotnet build                                          # also writes the OpenAPI contract to src/*/contract/
dotnet run --project src/Golden.Api  # http://localhost:8080
```

## Work on it

```bash
skies doctor          # the architecture doctors and the root allowlist (Skies.toml)
dotnet test           # every spec's E2E
skies g web-app Web --path clients/web   # a React web client, when you want one
```

A feature starts as a spec under `.specs/` (see `.specs/README.md`); `AGENTS.md` is the operating manual.
