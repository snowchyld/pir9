---
name: trace
description: Trace how a request flows through the pir9 codebase
user-invocable: true
context: fork
arguments:
  - name: feature
    description: Feature or endpoint to trace (e.g., "rss-sync", "POST /series", "episode-import")
    required: true
allowed-tools:
  - Read
  - Grep
  - Glob
  - Task
---

# Execution Path Trace: $ARGUMENTS

You are tracing the execution path of **$ARGUMENTS** through the pir9 codebase.

## Goal

Map the complete flow from entry point to side effects, documenting every layer with `file:line` references.

## Layers to Trace

### 1. Entry Point
- **HTTP endpoint**: Find the route in `src/api/v5/` or `src/api/v3/`
- **Scheduled job**: Find the trigger in `src/core/scheduler.rs`
- **Event handler**: Find the subscriber in `src/core/messaging.rs`

### 2. API Handler
- Extract parameters (Path, Query, Body)
- Authentication/authorization checks
- Input validation

### 3. Service Layer
- Business logic in `src/core/<domain>/services.rs`
- Cross-domain interactions
- Event publications on the event bus

### 4. Repository Layer
- Database queries in `src/core/datastore/repositories.rs`
- SQL operations (SELECT, INSERT, UPDATE, DELETE)
- Transaction boundaries

### 5. Side Effects
- **Event bus publications**: What events are emitted?
- **WebSocket notifications**: Does this trigger real-time updates?
- **File system operations**: Any disk I/O?
- **External API calls**: TMDB, IMDB, indexers, download clients?
- **Background tasks**: Does this spawn async work?

## Output Format

```
## Trace: $ARGUMENTS

### Flow
1. [HTTP/Scheduler/Event] → src/api/v5/xxx.rs:NN
2. Handler extracts [params] → calls service
3. src/core/xxx/services.rs:NN — [business logic description]
4. src/core/datastore/repositories.rs:NN — [SQL query description]
5. Event published: [event type] → src/core/messaging.rs:NN

### Dependencies
- [external service or database table]

### Side Effects
- [list of observable effects]
```

## Notes

- This runs in **forked context** — it won't pollute the main conversation
- Use `Grep` and `Read` to follow function calls through the layers
- Use `Task` with `Explore` subagent for deeper investigation if needed
- Always include file:line references for easy navigation
