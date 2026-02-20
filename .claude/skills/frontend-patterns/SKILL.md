---
name: frontend-patterns
description: Frontend conventions for the pir9 TypeScript/Web Components codebase
user-invocable: false
---

# pir9 Frontend Conventions

This skill provides background context when editing TypeScript files in the pir9 frontend.

## Tech Stack

- **Components**: Lit (Web Components with shadow DOM)
- **State management**: TanStack Query (query-core, no React)
- **Routing**: Navigo
- **Styling**: Tailwind CSS 4 + `clsx`/`tailwind-merge`
- **Build**: Vite
- **Lint/Format**: Biome 2.x

## Biome Rules (from `frontend/biome.json`)

- **`const` over `let`** — always use `const` unless reassignment is required
- **No unused imports** — remove them or Biome will flag
- **No implicit `any`** — annotate all types explicitly (warning level)
- **`forEach` callbacks**: must use block body `{ }` — no implicit return values
- **Import sorting**: automatic, alphabetical order
- **Formatting**: 2-space indent, single quotes, trailing commas
- **`void` in unions**: suppress `noConfusingVoidType` with `biome-ignore` when `void` is semantically correct (e.g., `Promise<void>` in union types)
- **`organizeImports`**: configured under `assist.actions.source.organizeImports` (Biome 2.x)

## Component Patterns

### Lit Web Components
```typescript
@customElement('my-component')
export class MyComponent extends LitElement {
  static styles = css`...`;

  @property({ type: String })
  accessor label = '';

  @state()
  accessor _loading = false;

  render() {
    return html`...`;
  }
}
```

### TanStack Query usage (without React)
```typescript
import { QueryClient } from '@tanstack/query-core';

const queryClient = new QueryClient();
const observer = new QueryObserver(queryClient, queryOptions);
observer.subscribe((result) => {
  // update component state
});
```

## File Organization

- `frontend/src/components/` — reusable UI primitives (ui-button, ui-input, ui-dialog, etc.)
- `frontend/src/features/` — feature modules (series/, movies/, activity/, etc.)
- `frontend/src/stores/` — TanStack Query stores
- `frontend/src/core/` — shared utilities (http client, router, etc.)

## API Client

`frontend/src/core/http.ts` provides typed HTTP helpers:
- `http.get<T>(url)` / `http.post<T>(url, body)` / `http.put<T>(url, body)` / `http.delete(url)`
- All requests go to the v5 API (`/api/v5/`)
- Returns typed JSON responses

## After Editing

Always run:
```bash
cd frontend && npm run lint && npm run typecheck
```

The PostToolUse hook in `.claude/hooks/check-biome.sh` will auto-run Biome on edited files.
