# ADR-005: In-Memory + Redis Event Bus for Distributed Mode

## Status
Accepted

## Context

pir9 components need to communicate asynchronously:
- API handlers need to notify the frontend via WebSocket when data changes
- The download queue monitor needs to trigger imports when downloads complete
- The notification service needs to react to grabs, imports, and health changes
- In distributed mode, servers and workers need to coordinate scan requests/results

Options considered:
1. **Direct function calls** — simple but creates tight coupling
2. **In-memory channels** (tokio broadcast/mpsc) — decoupled but single-process only
3. **Redis pub/sub** — distributed but requires Redis
4. **NATS/RabbitMQ** — full message broker, heavy infrastructure

## Decision

Implement a **dual-mode event bus** behind a trait abstraction:
- **In-memory mode** (`memory_bus.rs`): Tokio broadcast channels for standalone deployments
- **Redis mode** (`redis_bus.rs`): Redis pub/sub for distributed server/worker deployments

Selected at startup based on whether `PIR9_REDIS_URL` is configured. The `redis-events` Cargo feature flag controls whether Redis support is compiled in.

## Consequences

### Positive
- **Zero infrastructure for simple deployments** — standalone mode uses in-memory channels with no external dependencies
- **Distributed when needed** — Redis pub/sub scales to multiple workers scanning different NAS mounts
- **Loose coupling** — publishers don't know who subscribes; components can be added/removed independently
- **Fire-and-forget semantics** — publishers aren't blocked by slow subscribers

### Negative
- **No message persistence** — if a subscriber is down when an event fires, it's lost (acceptable for notifications and UI updates; not suitable for critical workflows)
- **Redis becomes a SPOF in distributed mode** — if Redis goes down, server and workers can't communicate
- **Dual implementation** — must maintain both `memory_bus.rs` and `redis_bus.rs` behind the trait

### Neutral
- Events are serialized as JSON for Redis transport — slight overhead vs in-memory, but enables cross-language workers in the future
- Worker heartbeat/online/offline events are only meaningful in distributed mode; they're published but ignored in standalone
