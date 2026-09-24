# health

Liveness for the app. `Ping` echoes a message so a probe can confirm the host is up and the slice
pipeline (model binding → Handle → ToHttp) is wired end-to-end.

## Boundaries

- **Inside**: a trivial liveness echo (`Ping`). It is a seed — replace it with your first real feature.
- **Outside**: real readiness checks (database, dependencies) belong in ASP.NET HealthChecks or their own
  module; this is a placeholder, not a monitoring surface.

## Design notes

### Ping is a scaffold seed
`Ping` exists so a fresh app has one passing slice + test (SKY0001/SKY0003 green) and a route to curl.
It carries no domain meaning — delete it once you generate a real slice.
