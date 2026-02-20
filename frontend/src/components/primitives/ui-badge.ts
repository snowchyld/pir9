/**
 * Badge/label component
 */

import { BaseComponent, customElement, attribute, html } from '../../core/component';

export type BadgeVariant = 'default' | 'primary' | 'success' | 'danger' | 'warning' | 'info';

@customElement('ui-badge')
export class UIBadge extends BaseComponent {
  @attribute() variant: BadgeVariant = 'default';

  protected template(): string {
    return html`
      <span class="badge badge-${this.variant}">
        <slot></slot>
      </span>

      <style>
        :host {
          display: inline-flex;
        }

        .badge {
          display: inline-flex;
          align-items: center;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
          white-space: nowrap;
        }

        .badge-default {
          background-color: var(--bg-card);
          color: var(--text-color);
        }

        .badge-primary {
          background-color: var(--color-primary);
          color: var(--color-white);
        }

        .badge-success {
          background-color: var(--color-success);
          color: var(--color-white);
        }

        .badge-danger {
          background-color: var(--color-danger);
          color: var(--color-white);
        }

        .badge-warning {
          background-color: var(--color-warning);
          color: var(--color-white);
        }

        .badge-info {
          background-color: var(--color-info);
          color: var(--color-white);
        }
      </style>
    `;
  }
}
