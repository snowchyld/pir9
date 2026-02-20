/**
 * Alert component for inline messages
 */

import { attribute, BaseComponent, customElement, html, safeHtml } from '../../core/component';

@customElement('ui-alert')
export class UIAlert extends BaseComponent {
  @attribute() variant: 'info' | 'success' | 'warning' | 'danger' = 'info';
  @attribute({ type: 'boolean' }) dismissible = false;

  private dismissed = false;

  protected template(): string {
    if (this.dismissed) {
      return '';
    }

    const icons: Record<string, string> = {
      info: '<svg class="alert-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="16" x2="12" y2="12"></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg>',
      success:
        '<svg class="alert-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>',
      warning:
        '<svg class="alert-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>',
      danger:
        '<svg class="alert-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>',
    };

    return html`
      <div class="alert alert-${this.variant}" role="alert">
        ${safeHtml(icons[this.variant])}
        <div class="alert-content">
          <slot></slot>
        </div>
        ${
          this.dismissible
            ? html`
          <button class="alert-dismiss" onclick="this.closest('ui-alert').dismiss()" aria-label="Dismiss">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        `
            : ''
        }
      </div>

      <style>
        :host {
          display: block;
        }

        .alert {
          display: flex;
          align-items: flex-start;
          gap: 0.75rem;
          padding: 0.75rem 1rem;
          border-radius: 0.375rem;
          border-width: 1px;
          border-style: solid;
        }

        .alert-info {
          background-color: var(--alert-info-bg);
          border-color: var(--alert-info-border);
          color: var(--alert-info-text);
        }

        .alert-success {
          background-color: var(--alert-success-bg);
          border-color: var(--alert-success-border);
          color: var(--alert-success-text);
        }

        .alert-warning {
          background-color: var(--alert-warning-bg);
          border-color: var(--alert-warning-border);
          color: var(--alert-warning-text);
        }

        .alert-danger {
          background-color: var(--alert-danger-bg);
          border-color: var(--alert-danger-border);
          color: var(--alert-danger-text);
        }

        .alert-icon {
          flex-shrink: 0;
          width: 20px;
          height: 20px;
        }

        .alert-info .alert-icon { color: var(--color-info); }
        .alert-success .alert-icon { color: var(--color-success); }
        .alert-warning .alert-icon { color: var(--color-warning); }
        .alert-danger .alert-icon { color: var(--color-danger); }

        .alert-content {
          flex: 1;
          font-size: 0.875rem;
        }

        .alert-dismiss {
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.25rem;
          background: transparent;
          border: none;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          cursor: pointer;
          transition: color 0.15s ease;
        }

        .alert-dismiss:hover {
          color: var(--text-color);
        }
      </style>
    `;
  }

  dismiss(): void {
    this.dismissed = true;
    this.emit('dismiss');
  }
}
