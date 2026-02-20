/**
 * Progress Ring Component
 * Circular progress indicator with glassmorphism styling
 */

import { BaseComponent, customElement, html } from '../../core/component';

@customElement('progress-ring')
export class ProgressRing extends BaseComponent {
  private _value = 0;
  private _max = 100;
  private _size = 80;
  private _strokeWidth = 6;
  private _label = '';
  private _sublabel = '';
  private _color: 'primary' | 'success' | 'warning' | 'danger' = 'primary';

  static get observedAttributes(): string[] {
    return ['value', 'max', 'size', 'stroke-width', 'label', 'sublabel', 'color'];
  }

  attributeChangedCallback(name: string, _old: string, value: string): void {
    switch (name) {
      case 'value':
        this._value = parseFloat(value) || 0;
        break;
      case 'max':
        this._max = parseFloat(value) || 100;
        break;
      case 'size':
        this._size = parseInt(value) || 80;
        break;
      case 'stroke-width':
        this._strokeWidth = parseInt(value) || 6;
        break;
      case 'label':
        this._label = value;
        break;
      case 'sublabel':
        this._sublabel = value;
        break;
      case 'color':
        this._color = value as typeof this._color;
        break;
    }
    this.requestUpdate();
  }

  protected template(): string {
    const percentage = Math.min(100, Math.max(0, (this._value / this._max) * 100));
    const radius = (this._size - this._strokeWidth) / 2;
    const circumference = 2 * Math.PI * radius;
    const offset = circumference - (percentage / 100) * circumference;
    const center = this._size / 2;

    const colorVar = `var(--color-${this._color})`;
    const glowVar = `var(--glow-${this._color})`;

    return html`
      <div class="progress-ring-container">
        <svg
          class="progress-ring"
          width="${this._size}"
          height="${this._size}"
          viewBox="0 0 ${this._size} ${this._size}"
        >
          <!-- Glow filter -->
          <defs>
            <filter id="glow-${this._color}" x="-50%" y="-50%" width="200%" height="200%">
              <feGaussianBlur stdDeviation="3" result="coloredBlur"/>
              <feMerge>
                <feMergeNode in="coloredBlur"/>
                <feMergeNode in="SourceGraphic"/>
              </feMerge>
            </filter>
          </defs>

          <!-- Background circle -->
          <circle
            class="progress-ring-bg"
            cx="${center}"
            cy="${center}"
            r="${radius}"
            stroke-width="${this._strokeWidth}"
          />

          <!-- Progress circle -->
          <circle
            class="progress-ring-fill ${this._color}"
            cx="${center}"
            cy="${center}"
            r="${radius}"
            stroke-width="${this._strokeWidth}"
            stroke-dasharray="${circumference}"
            stroke-dashoffset="${offset}"
            filter="url(#glow-${this._color})"
          />
        </svg>

        <div class="progress-ring-content">
          ${this._label ? `<span class="progress-ring-label">${this._label}</span>` : ''}
          ${this._sublabel ? `<span class="progress-ring-sublabel">${this._sublabel}</span>` : ''}
        </div>
      </div>

      <style>
        :host {
          display: inline-flex;
        }

        .progress-ring-container {
          position: relative;
          display: inline-flex;
          align-items: center;
          justify-content: center;
        }

        .progress-ring {
          transform: rotate(-90deg);
        }

        .progress-ring-bg {
          fill: none;
          stroke: var(--progress-ring-bg);
        }

        .progress-ring-fill {
          fill: none;
          stroke-linecap: round;
          transition: stroke-dashoffset var(--transition-slow) var(--ease-out-expo);
        }

        .progress-ring-fill.primary {
          stroke: var(--color-primary);
        }

        .progress-ring-fill.success {
          stroke: var(--color-success);
        }

        .progress-ring-fill.warning {
          stroke: var(--color-warning);
        }

        .progress-ring-fill.danger {
          stroke: var(--color-danger);
        }

        .progress-ring-content {
          position: absolute;
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          text-align: center;
        }

        .progress-ring-label {
          font-size: 1.25rem;
          font-weight: 600;
          line-height: 1.2;
        }

        .progress-ring-sublabel {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }
}
