---
name: sonarr-compat
description: Sonarr v3 API compatibility guard — prevents breaking changes
user-invocable: false
---

# Sonarr API v3 Compatibility

This skill loads automatically when editing files in `src/api/v3/`. It enforces backward compatibility with Sonarr clients.

## Critical Rules

1. **NEVER change v3 response shapes** — external Sonarr clients (Overseerr, Ombi, Tdarr, Bazarr, LunaSea, nzb360) depend on exact JSON structures
2. **All fields must use `camelCase`** — enforced by `#[serde(rename_all = "camelCase")]`
3. **Field types must not change** — e.g., if a field is `i32`, don't change it to `i64`
4. **Null fields must remain nullable** — use `Option<T>` with `#[serde(skip_serializing_if = "Option::is_none")]`
5. **New fields are OK** — you can ADD fields to responses (clients ignore unknown fields)
6. **Removing fields is NEVER OK** — even deprecated ones must remain in responses

## v3 Endpoint Structure

All v3 endpoints are in `src/api/v3/`:
- Route registration in each module's `routes()` function
- Mounted at `/api/v3/` prefix
- Response models in `src/api/v3/models.rs` or inline

## When making changes

- Read the existing v3 response type FIRST
- If you need to change behavior, add a NEW field rather than modifying an existing one
- Test with a Sonarr client (curl the endpoint and verify JSON shape)
- If unsure about a v3 field's purpose, check Sonarr's API documentation

## v3 vs v5

- **v3** (`/api/v3/`): Frozen compatibility layer — add-only changes
- **v5** (`/api/v5/`): Current API — freely evolvable, pir9-native types
- New features should be implemented in v5 first, then mapped to v3 if needed
