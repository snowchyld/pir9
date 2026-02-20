/**
 * Progress bar component
 */

import { attribute, BaseComponent, customElement, html } from '../../core/component';

@customElement('ui-progress')
export class UIProgress extends BaseComponent {
  @attribute({ type: 'number' }) value = 0;
  @attribute({ type: 'number' }) max = 100;
  @attribute() variant: 'default' | 'success' | 'warning' | 'danger' = 'default';
  @attribute({ type: 'boolean' }) showLabel = false;
  @attribute() size: 'sm' | 'md' | 'lg' = 'md';

  protected template(): string {
    const percent = Math.min(100, Math.max(0, (this.value / this.max) * 100));

    return html`
      <div class="progress-wrapper">
        <div class="progress progress-${this.size}">
          <div
            class="progress-bar progress-${this.variant}"
            style="width: ${percent}%"
            role="progressbar"
            aria-valuenow="${this.value}"
            aria-valuemin="0"
            aria-valuemax="${this.max}"
          ></div>
        </div>
        ${this.showLabel ? `<span class="progress-label">${Math.round(percent)}%</span>` : ''}
      </div>

      <style>
        :host {
          display: block;
        }

        .progress-wrapper {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .progress {
          flex: 1;
          background-color: var(--bg-progress);
          border-radius: 9999px;
          overflow: hidden;
        }

        .progress-sm { height: 4px; }
        .progress-md { height: 8px; }
        .progress-lg { height: 12px; }

        .progress-bar {
          height: 100%;
          border-radius: 9999px;
          transition: width 0.3s ease;
        }

        .progress-default {
          background-color: var(--color-primary);
        }

        .progress-success {
          background-color: var(--color-success);
        }

        .progress-warning {
          background-color: var(--color-warning);
        }

        .progress-danger {
          background-color: var(--color-danger);
        }

        .progress-label {
          font-size: 0.75rem;
          font-weight: 500;
          color: var(--text-color-muted);
          min-width: 3rem;
          text-align: right;
        }
      </style>
    `;
  }
}
