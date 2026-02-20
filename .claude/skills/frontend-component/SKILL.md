---
name: frontend-component
description: Create a new Web Component following pir9 frontend patterns
user-invocable: true
arguments:
  - name: component
    description: Name of the component (e.g., "episode-card", "filter-bar")
    required: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
---

# Web Component: $ARGUMENTS

You are creating a new Web Component called **$ARGUMENTS** for the pir9 frontend.

## Reference Files

Read these first to understand existing patterns:
- `frontend/src/components/` — existing UI primitives (ui-button, ui-input, ui-dialog, etc.)
- `frontend/src/features/` — feature-specific components (series, movies, add-movie, etc.)
- `frontend/src/core/http.ts` — API client
- `frontend/biome.json` — Linting configuration

## Component Template

```typescript
import { LitElement, html, css } from 'lit';
import { customElement, property } from 'lit/decorators.js';

@customElement('$ARGUMENTS')
export class ComponentName extends LitElement {
  static styles = css`
    :host {
      display: block;
    }
  `;

  @property({ type: String })
  accessor label = '';

  render() {
    return html`
      <div class="container">
        <slot></slot>
      </div>
    `;
  }
}

declare global {
  interface HTMLElementTagNameMap {
    '$ARGUMENTS': ComponentName;
  }
}
```

## Placement

- **Reusable UI primitives** → `frontend/src/components/`
- **Feature-specific components** → `frontend/src/features/<feature>/`
- **Page-level components** → `frontend/src/features/<feature>/<feature>-page.ts`

## Styling

- Use Tailwind CSS 4 utility classes where possible
- CSS custom properties for theming (dark/light mode support)
- `clsx` and `tailwind-merge` for conditional class composition

## Conventions

- **`const` over `let`** when variable is never reassigned
- **No unused imports** — Biome enforces this
- **No implicit `any`** types — always annotate
- **`forEach` callbacks**: use block body `{ }` to avoid implicit return values
- **Import sorting**: alphabetical (Biome auto-sorts)
- **2-space indent, single quotes, trailing commas**

## After Creating

Run linting to verify the component passes all checks:
```bash
cd frontend && npm run lint && npm run typecheck
```
