/**
 * Loading spinner component
 */

import { attribute, BaseComponent, customElement, html } from '../../core/component';

export type SpinnerSize = 'sm' | 'md' | 'lg';

@customElement('ui-spinner')
export class UISpinner extends BaseComponent {
  @attribute() size: SpinnerSize = 'md';

  protected template(): string {
    const sizeMap = {
      sm: '16px',
      md: '24px',
      lg: '32px',
    };

    const sizeValue = sizeMap[this.size];

    return html`
      <div
        class="spinner"
        style="width: ${sizeValue}; height: ${sizeValue};"
        role="status"
        aria-label="Loading"
      >
        <span class="sr-only">Loading...</span>
      </div>

      <style>
        :host {
          display: inline-flex;
          align-items: center;
          justify-content: center;
        }

        .spinner {
          border: 2px solid var(--border-color);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        .sr-only {
          position: absolute;
          width: 1px;
          height: 1px;
          padding: 0;
          margin: -1px;
          overflow: hidden;
          clip: rect(0, 0, 0, 0);
          white-space: nowrap;
          border: 0;
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
