/**
 * Stats Card Component
 * Glassmorphism card for displaying statistics with optional trend indicator
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';

@customElement('stats-card')
export class StatsCard extends BaseComponent {
  private _title = '';
  private _value = '';
  private _subtitle = '';
  private _icon = '';
  private _trend: 'up' | 'down' | 'neutral' | '' = '';
  private _trendValue = '';
  private _color: 'primary' | 'success' | 'warning' | 'danger' | 'default' = 'default';

  static get observedAttributes(): string[] {
    return ['title', 'value', 'subtitle', 'icon', 'trend', 'trend-value', 'color'];
  }

  attributeChangedCallback(name: string, _old: string, value: string): void {
    switch (name) {
      case 'title':
        this._title = value;
        break;
      case 'value':
        this._value = value;
        break;
      case 'subtitle':
        this._subtitle = value;
        break;
      case 'icon':
        this._icon = value;
        break;
      case 'trend':
        this._trend = value as typeof this._trend;
        break;
      case 'trend-value':
        this._trendValue = value;
        break;
      case 'color':
        this._color = value as typeof this._color;
        break;
    }
    this.requestUpdate();
  }

  protected template(): string {
    return html`
      <div class="stats-card ${this._color}">
        <div class="stats-card-header">
          ${this._icon ? `<div class="stats-card-icon">${this.getIcon()}</div>` : ''}
          ${this._title ? `<span class="stats-card-title">${escapeHtml(this._title)}</span>` : ''}
        </div>

        <div class="stats-card-body">
          <span class="stats-card-value">${escapeHtml(this._value)}</span>
          ${this._trendValue ? this.renderTrend() : ''}
        </div>

        ${this._subtitle ? `<span class="stats-card-subtitle">${escapeHtml(this._subtitle)}</span>` : ''}
      </div>

      <style>
        :host {
          display: block;
        }

        .stats-card {
          padding: 1.25rem;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          -webkit-backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          border-radius: 0.875rem;
          transition: all var(--transition-normal) var(--ease-out-expo);
          position: relative;
          overflow: hidden;
        }

        .stats-card::before {
          content: '';
          position: absolute;
          inset: 0;
          background: linear-gradient(135deg, rgba(255,255,255,0.08) 0%, transparent 50%);
          pointer-events: none;
        }

        .stats-card:hover {
          transform: translateY(-2px);
          box-shadow: var(--shadow-card-hover);
        }

        /* Color variants - accent line on left */
        .stats-card.primary {
          border-left: 3px solid var(--color-primary);
        }
        .stats-card.success {
          border-left: 3px solid var(--color-success);
        }
        .stats-card.warning {
          border-left: 3px solid var(--color-warning);
        }
        .stats-card.danger {
          border-left: 3px solid var(--color-danger);
        }

        .stats-card-header {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          margin-bottom: 0.75rem;
        }

        .stats-card-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 32px;
          height: 32px;
          background: var(--bg-card-alt);
          border-radius: 0.5rem;
          color: var(--text-color-muted);
        }

        .stats-card-icon svg {
          width: 18px;
          height: 18px;
        }

        .primary .stats-card-icon {
          color: var(--color-primary);
          background: rgba(93, 156, 236, 0.15);
        }
        .success .stats-card-icon {
          color: var(--color-success);
          background: rgba(39, 194, 76, 0.15);
        }
        .warning .stats-card-icon {
          color: var(--color-warning);
          background: rgba(255, 144, 43, 0.15);
        }
        .danger .stats-card-icon {
          color: var(--color-danger);
          background: rgba(240, 80, 80, 0.15);
        }

        .stats-card-title {
          font-size: 0.75rem;
          font-weight: 500;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-color-muted);
        }

        .stats-card-body {
          display: flex;
          align-items: baseline;
          gap: 0.75rem;
        }

        .stats-card-value {
          font-size: 2rem;
          font-weight: 700;
          line-height: 1;
          background: linear-gradient(135deg, var(--text-color) 0%, var(--text-color-muted) 100%);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
          background-clip: text;
        }

        .stats-card-trend {
          display: inline-flex;
          align-items: center;
          gap: 0.25rem;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
        }

        .stats-card-trend.up {
          color: var(--color-success);
          background: rgba(39, 194, 76, 0.15);
        }

        .stats-card-trend.down {
          color: var(--color-danger);
          background: rgba(240, 80, 80, 0.15);
        }

        .stats-card-trend.neutral {
          color: var(--text-color-muted);
          background: var(--bg-card-alt);
        }

        .stats-card-trend svg {
          width: 12px;
          height: 12px;
        }

        .stats-card-subtitle {
          display: block;
          margin-top: 0.5rem;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  private renderTrend(): string {
    const arrow = this._trend === 'up'
      ? '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
      : this._trend === 'down'
      ? '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>'
      : '';

    return html`
      <span class="stats-card-trend ${this._trend}">
        ${arrow}
        ${escapeHtml(this._trendValue)}
      </span>
    `;
  }

  private getIcon(): string {
    const icons: Record<string, string> = {
      series: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="7" width="20" height="15" rx="2" ry="2"></rect><polyline points="17 2 12 7 7 2"></polyline></svg>',
      episodes: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"></polygon></svg>',
      download: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>',
      disk: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"></ellipse><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"></path><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"></path></svg>',
      calendar: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect><line x1="16" y1="2" x2="16" y2="6"></line><line x1="8" y1="2" x2="8" y2="6"></line><line x1="3" y1="10" x2="21" y2="10"></line></svg>',
      warning: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>',
      check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>',
    };

    return icons[this._icon] || '';
  }
}
