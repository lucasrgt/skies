---
id: "9998"
runner: api
---
# Generated hub rooms are scoped to the caller's org

## Failure modes
- FM-1 a member of another org hears a room's broadcast because it joined the same room key.
- FM-2 a connection broadcasts into a room it never joined.
- FM-3 a connection without an access token joins or broadcasts.
