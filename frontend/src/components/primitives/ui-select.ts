/**
 * Select dropdown component
 */

import { attribute, BaseComponent, customElement, escapeHtml, html } from '../../core/component';

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

@customElement('ui-select')
export class UISelect extends BaseComponent {
  @attribute() name = '';
  @attribute() value = '';
  @attribute() placeholder = 'Select...';
  @attribute({ type: 'boolean' }) disabled = false;
  @attribute({ type: 'boolean' }) required = false;
  @attribute() error = '';

  private _options: SelectOption[] = [];

  get options(): SelectOption[] {
    return this._options;
  }

  set options(value: SelectOption[]) {
    this._options = value;
    if (this._isConnected) {
      this.requestUpdate();
    }
  }

  protected template(): string {
    const hasError = !!this.error;

    return html`
      <div class="select-wrapper">
        <select
          class="select ${hasError ? 'select-error' : ''}"
          name="${this.name}"
          ?disabled="${this.disabled}"
          ?required="${this.required}"
          onchange="this.closest('ui-select').handleChange(event)"
        >
          ${this.placeholder ? `<option value="" disabled ${!this.value ? 'selected' : ''}>${escapeHtml(this.placeholder)}</option>` : ''}
          ${this._options
            .map(
              (opt) => `
            <option
              value="${escapeHtml(opt.value)}"
              ${opt.disabled ? 'disabled' : ''}
              ${opt.value === this.value ? 'selected' : ''}
            >
              ${escapeHtml(opt.label)}
            </option>
          `,
            )
            .join('')}
        </select>
        <div class="select-icon">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="6 9 12 15 18 9"></polyline>
          </svg>
        </div>
        ${this.error ? `<span class="select-error-text">${escapeHtml(this.error)}</span>` : ''}
      </div>

      <style>
        :host {
          display: block;
        }

        .select-wrapper {
          position: relative;
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
        }

        .select {
          width: 100%;
          padding: 0.5rem 2rem 0.5rem 0.75rem;
          background-color: var(--bg-input);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          font-size: 0.875rem;
          cursor: pointer;
          appearance: none;
          transition: border-color 0.15s ease, box-shadow 0.15s ease;
        }

        .select:focus {
          outline: none;
          border-color: var(--border-input-focus);
          box-shadow: 0 0 0 3px var(--shadow-input-focus);
        }

        .select:disabled {
          background-color: var(--bg-input-readonly);
          color: var(--form-input-disabled);
          cursor: not-allowed;
        }

        .select-error {
          border-color: var(--border-input-error);
        }

        .select-error:focus {
          box-shadow: 0 0 0 3px var(--shadow-input-error);
        }

        .select-icon {
          position: absolute;
          right: 0.75rem;
          top: 50%;
          transform: translateY(-50%);
          pointer-events: none;
          color: var(--text-color-muted);
        }

        .select-error-text {
          font-size: 0.75rem;
          color: var(--color-danger);
        }
      </style>
    `;
  }

  handleChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    this.value = select.value;
    this.emit('change', { value: select.value });
  }
}
