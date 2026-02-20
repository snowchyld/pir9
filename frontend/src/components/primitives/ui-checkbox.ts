/**
 * Checkbox component
 */

import { BaseComponent, customElement, attribute, html, escapeHtml } from '../../core/component';

@customElement('ui-checkbox')
export class UICheckbox extends BaseComponent {
  @attribute() name = '';
  @attribute({ type: 'boolean' }) checked = false;
  @attribute({ type: 'boolean' }) disabled = false;
  @attribute() label = '';

  protected template(): string {
    return html`
      <label class="checkbox-wrapper ${this.disabled ? 'disabled' : ''}">
        <input
          type="checkbox"
          class="checkbox-input"
          name="${this.name}"
          ?checked="${this.checked}"
          ?disabled="${this.disabled}"
          onchange="this.closest('ui-checkbox').handleChange(event)"
        />
        <span class="checkbox-box">
          <svg class="checkbox-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
            <polyline points="20 6 9 17 4 12"></polyline>
          </svg>
        </span>
        ${this.label ? `<span class="checkbox-label">${escapeHtml(this.label)}</span>` : '<slot></slot>'}
      </label>

      <style>
        :host {
          display: inline-block;
        }

        .checkbox-wrapper {
          display: inline-flex;
          align-items: center;
          gap: 0.5rem;
          cursor: pointer;
          user-select: none;
        }

        .checkbox-wrapper.disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .checkbox-input {
          position: absolute;
          opacity: 0;
          width: 0;
          height: 0;
        }

        .checkbox-box {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 18px;
          height: 18px;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          transition: all 0.15s ease;
        }

        .checkbox-input:checked + .checkbox-box {
          background-color: var(--color-primary);
          border-color: var(--color-primary);
        }

        .checkbox-input:focus-visible + .checkbox-box {
          box-shadow: 0 0 0 3px var(--shadow-input-focus);
        }

        .checkbox-icon {
          width: 12px;
          height: 12px;
          color: var(--color-white);
          opacity: 0;
          transform: scale(0.5);
          transition: all 0.15s ease;
        }

        .checkbox-input:checked + .checkbox-box .checkbox-icon {
          opacity: 1;
          transform: scale(1);
        }

        .checkbox-label {
          font-size: 0.875rem;
          color: var(--text-color);
        }
      </style>
    `;
  }

  handleChange(event: Event): void {
    const input = event.target as HTMLInputElement;
    this.checked = input.checked;
    this.emit('change', { checked: input.checked });
  }
}
