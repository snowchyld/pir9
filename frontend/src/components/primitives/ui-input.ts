/**
 * Input component with validation states
 */

import { attribute, BaseComponent, customElement, escapeHtml, html } from '../../core/component';

export type InputType = 'text' | 'password' | 'email' | 'number' | 'search' | 'tel' | 'url';

@customElement('ui-input')
export class UIInput extends BaseComponent {
  @attribute() type: InputType = 'text';
  @attribute() name = '';
  @attribute() value = '';
  @attribute() placeholder = '';
  @attribute({ type: 'boolean' }) disabled = false;
  @attribute({ type: 'boolean' }) readonly = false;
  @attribute({ type: 'boolean' }) required = false;
  @attribute() error = '';
  @attribute() hint = '';

  protected template(): string {
    const hasError = !!this.error;

    return html`
      <div class="input-wrapper">
        <input
          class="input ${hasError ? 'input-error' : ''}"
          type="${this.type}"
          name="${this.name}"
          value="${escapeHtml(this.value)}"
          placeholder="${this.placeholder}"
          ?disabled="${this.disabled}"
          ?readonly="${this.readonly}"
          ?required="${this.required}"
          oninput="this.closest('ui-input').handleInput(event)"
          onchange="this.closest('ui-input').handleChange(event)"
        />
        ${this.error ? `<span class="input-error-text">${escapeHtml(this.error)}</span>` : ''}
        ${!this.error && this.hint ? `<span class="input-hint">${escapeHtml(this.hint)}</span>` : ''}
      </div>

      <style>
        :host {
          display: block;
        }

        .input-wrapper {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
        }

        .input {
          width: 100%;
          padding: 0.5rem 0.75rem;
          background-color: var(--bg-input);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          font-size: 0.875rem;
          transition: border-color 0.15s ease, box-shadow 0.15s ease;
        }

        .input::placeholder {
          color: var(--text-color-muted);
        }

        .input:focus {
          outline: none;
          border-color: var(--border-input-focus);
          box-shadow: 0 0 0 3px var(--shadow-input-focus);
        }

        .input:disabled {
          background-color: var(--bg-input-readonly);
          color: var(--form-input-disabled);
          cursor: not-allowed;
        }

        .input:read-only {
          background-color: var(--bg-input-readonly);
        }

        .input-error {
          border-color: var(--border-input-error);
        }

        .input-error:focus {
          box-shadow: 0 0 0 3px var(--shadow-input-error);
        }

        .input-error-text {
          font-size: 0.75rem;
          color: var(--color-danger);
        }

        .input-hint {
          font-size: 0.75rem;
          color: var(--text-help);
        }
      </style>
    `;
  }

  handleInput(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.value = input.value;
    this.emit('input', { value: input.value });
  }

  handleChange(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.emit('change', { value: input.value });
  }
}
