# Architecture Decision Records

Significant technical decisions for pir9, documented with context and rationale. ADRs are immutable once accepted — if a decision changes, create a new ADR that supersedes the old one.

## ADR Template

```markdown
# ADR-XXX: [Title]

## Status
[Proposed | Accepted | Deprecated | Superseded by ADR-XXX]

## Context
What is the issue that motivated this decision?

## Decision
What did we decide?

## Consequences

### Positive
- Benefit 1

### Negative
- Drawback 1

### Neutral
- Side effect 1
```

## Index

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [001](./001-axum-framework.md) | Use Axum as web framework | Accepted | 2025-06 |
| [002](./002-sonarr-v3-compatibility.md) | Maintain Sonarr v3 API compatibility | Accepted | 2025-06 |
| [003](./003-sqlx-compile-time-queries.md) | Use SQLx with compile-time checked queries | Accepted | 2025-07 |
| [004](./004-dual-sqlite-postgres.md) | Support both SQLite and PostgreSQL | Accepted | 2025-08 |
| [005](./005-event-bus-architecture.md) | In-memory + Redis event bus for distributed mode | Accepted | 2025-09 |

## When to Write an ADR

Write one when:
- Choosing between multiple valid technical approaches
- Adopting a new library or framework
- Changing an existing architectural pattern
- Making a decision that will be hard to reverse

Don't write one for:
- Obvious choices or trivial decisions
- Style preferences covered by clippy/Biome
- Temporary workarounds (use `.context/debt.md` instead)
