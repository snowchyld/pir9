# ADR-002: Maintain Sonarr v3 API Compatibility

## Status
Accepted

## Context

Sonarr has a rich ecosystem of third-party clients and companion apps:
- **Overseerr** / **Ombi** — media request management
- **LunaSea** / **nzb360** — mobile remote control
- **Bazarr** — subtitle management
- **Tdarr** — media transcoding
- **Organizr** / **Heimdall** — dashboard integration

All of these communicate with Sonarr via its v3 REST API. For pir9 to be a viable Sonarr replacement, these tools must work without modification.

## Decision

Maintain a **frozen v3 API** (`/api/v3/`) that exactly replicates Sonarr's response shapes, while developing a new **v5 API** (`/api/v5/`) for pir9-native features.

### Rules for v3
- Response JSON shapes are **immutable** — field names, types, and nullability cannot change
- New fields **may** be added (clients ignore unknown fields)
- Fields **must never** be removed or renamed
- All fields use `camelCase` (Sonarr convention)

### Rules for v5
- Freely evolvable — pir9-native types and field names
- New features are implemented in v5 first
- v3 endpoints may internally delegate to v5 logic with response mapping

## Consequences

### Positive
- **Drop-in Sonarr replacement** — existing tools work immediately
- **Gradual migration** — users can switch from Sonarr without reconfiguring all companion apps
- **Community adoption** — lower barrier to entry

### Negative
- **Dual maintenance** — some endpoints exist in both v3 and v5 with different response shapes
- **Legacy constraints** — v3 shapes sometimes don't match ideal Rust types (requires manual mapping)
- **Testing burden** — v3 responses should be validated against actual Sonarr output

### Neutral
- v5 has no external consumers yet — we can freely evolve it until third-party tools adopt it
