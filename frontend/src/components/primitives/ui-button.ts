/**
 * Button component with variants
 */

import { BaseComponent, customElement, attribute, html } from '../../core/component';

export type ButtonVariant = 'default' | 'primary' | 'danger' | 'success' | 'warning' | 'ghost';
export type ButtonSize = 'sm' | 'md' | 'lg';

@customElement('ui-button')
export class UIButton extends BaseComponent {
  @attribute() variant: ButtonVariant = 'default';
  @attribute() size: ButtonSize = 'md';
  @attribute({ type: 'boolean' }) disabled = false;
  @attribute({ type: 'boolean' }) loading = false;
  @attribute() type: 'button' | 'submit' | 'reset' = 'button';

  protected template(): string {
    const classes = this.cx(
      'btn',
      `btn-${this.variant}`,
      `btn-${this.size}`,
      { 'btn-loading': this.loading, 'btn-disabled': this.disabled }
    );

    return html`
      <button
        class="${classes}"
        type="${this.type}"
        ?disabled="${this.disabled || this.loading}"
      >
        ${this.loading ? '<span class="spinner"></span>' : ''}
        <slot></slot>
      </button>

      <style>
        :host {
          display: inline-block;
        }

        .btn {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          gap: 0.5rem;
          font-weight: 500;
          border-radius: 0.25rem;
          transition: all 0.15s ease;
          cursor: pointer;
          white-space: nowrap;
        }

        .btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        /* Sizes */
        .btn-sm {
          padding: 0.25rem 0.75rem;
          font-size: 0.875rem;
        }

        .btn-md {
          padding: 0.5rem 1rem;
          font-size: 0.875rem;
        }

        .btn-lg {
          padding: 0.75rem 1.5rem;
          font-size: 1rem;
        }

        /* Variants */
        .btn-default {
          background-color: var(--btn-default-bg);
          color: var(--btn-default-text);
          border: 1px solid var(--btn-default-border);
        }

        .btn-default:hover:not(:disabled) {
          background-color: var(--btn-default-bg-hover);
          border-color: var(--btn-default-border-hover);
        }

        .btn-primary {
          background-color: var(--btn-primary-bg);
          color: var(--color-white);
          border: 1px solid var(--btn-primary-border);
        }

        .btn-primary:hover:not(:disabled) {
          background-color: var(--btn-primary-bg-hover);
          border-color: var(--btn-primary-border-hover);
        }

        .btn-danger {
          background-color: var(--btn-danger-bg);
          color: var(--color-white);
          border: 1px solid var(--btn-danger-border);
        }

        .btn-danger:hover:not(:disabled) {
          background-color: var(--btn-danger-bg-hover);
          border-color: var(--btn-danger-border-hover);
        }

        .btn-success {
          background-color: var(--btn-success-bg);
          color: var(--color-white);
          border: 1px solid var(--btn-success-border);
        }

        .btn-success:hover:not(:disabled) {
          background-color: var(--btn-success-bg-hover);
          border-color: var(--btn-success-border-hover);
        }

        .btn-warning {
          background-color: var(--btn-warning-bg);
          color: var(--color-white);
          border: 1px solid var(--btn-warning-border);
        }

        .btn-warning:hover:not(:disabled) {
          background-color: var(--btn-warning-bg-hover);
          border-color: var(--btn-warning-border-hover);
        }

        .btn-ghost {
          background-color: transparent;
          color: var(--text-color);
          border: 1px solid transparent;
        }

        .btn-ghost:hover:not(:disabled) {
          background-color: var(--bg-input-hover);
        }

        /* Loading spinner */
        .spinner {
          width: 14px;
          height: 14px;
          border: 2px solid currentColor;
          border-top-color: transparent;
          border-radius: 50%;
          animation: spin 0.6s linear infinite;
        }

        @keyframes spin {
          to {
            transform: rotate(360deg);
          }
        }
      </style>
    `;
  }
}
