---
name: frontend-store
description: Create a TanStack Query store for an API resource
user-invocable: true
arguments:
  - name: resource
    description: Name of the API resource (e.g., "series", "episodes", "history")
    required: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
---

# TanStack Query Store: $ARGUMENTS

You are creating a TanStack Query store for the **$ARGUMENTS** API resource.

## Reference Files

Read these first:
- `frontend/src/stores/` — existing store patterns (theme.store.ts, app.store.ts)
- `frontend/src/core/http.ts` — API client with typed fetch helpers
- `frontend/src/features/` — see how existing features consume stores

## Store Template

Create `frontend/src/stores/$ARGUMENTS.store.ts`:

```typescript
import { queryOptions, QueryClient } from '@tanstack/query-core';
import { http } from '../core/http';

// Types
export interface Resource {
  id: number;
  // ... fields matching the v5 API response (camelCase)
}

// Query keys
export const resourceKeys = {
  all: ['$ARGUMENTS'] as const,
  lists: () => [...resourceKeys.all, 'list'] as const,
  list: (filters: Record<string, unknown>) => [...resourceKeys.all, 'list', filters] as const,
  details: () => [...resourceKeys.all, 'detail'] as const,
  detail: (id: number) => [...resourceKeys.all, 'detail', id] as const,
};

// Query options
export const listResourceOptions = () =>
  queryOptions({
    queryKey: resourceKeys.lists(),
    queryFn: () => http.get<Resource[]>('/api/v5/$ARGUMENTS'),
  });

export const getResourceOptions = (id: number) =>
  queryOptions({
    queryKey: resourceKeys.detail(id),
    queryFn: () => http.get<Resource>(`/api/v5/$ARGUMENTS/${id}`),
  });

// Mutations
export const createResource = (queryClient: QueryClient) => ({
  mutationFn: (data: Partial<Resource>) =>
    http.post<Resource>('/api/v5/$ARGUMENTS', data),
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: resourceKeys.lists() });
  },
});

export const updateResource = (queryClient: QueryClient) => ({
  mutationFn: (data: Resource) =>
    http.put<Resource>(`/api/v5/$ARGUMENTS/${data.id}`, data),
  onSuccess: (_data: Resource, variables: Resource) => {
    queryClient.invalidateQueries({ queryKey: resourceKeys.detail(variables.id) });
    queryClient.invalidateQueries({ queryKey: resourceKeys.lists() });
  },
});

export const deleteResource = (queryClient: QueryClient) => ({
  mutationFn: (id: number) =>
    http.delete(`/api/v5/$ARGUMENTS/${id}`),
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: resourceKeys.lists() });
  },
});
```

## Patterns

- **Query keys**: Hierarchical array structure for granular invalidation
- **`queryOptions()`**: Type-safe query configuration (TanStack Query v5+)
- **Invalidation**: Invalidate parent keys to cascade to all children
- **Optimistic updates**: For mutations where instant UI feedback matters, use `onMutate`/`onError`/`onSettled`

## After Creating

```bash
cd frontend && npm run lint && npm run typecheck
```
