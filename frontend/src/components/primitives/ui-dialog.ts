/**
 * Dialog/Modal component
 */

import { BaseComponent, customElement, attribute, html, escapeHtml } from '../../core/component';

@customElement('ui-dialog')
export class UIDialog extends BaseComponent {
  @attribute({ type: 'boolean' }) open = false;
  @attribute() title = '';
  @attribute() size: 'sm' | 'md' | 'lg' | 'xl' = 'md';

  protected onInit(): void {
    // Close on escape
    this.handleKeydown = this.handleKeydown.bind(this);
  }

  protected onMount(): void {
    document.addEventListener('keydown', this.handleKeydown);
  }

  protected onDestroy(): void {
    document.removeEventListener('keydown', this.handleKeydown);
  }

  private handleKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape' && this.open) {
      this.close();
    }
  }

  protected template(): string {
    if (!this.open) {
      return '';
    }

    const sizeClasses = {
      sm: 'max-w-sm',
      md: 'max-w-md',
      lg: 'max-w-lg',
      xl: 'max-w-xl',
    };

    return html`
      <div class="dialog-backdrop" onclick="this.closest('ui-dialog').handleBackdropClick(event)">
        <div class="dialog ${sizeClasses[this.size]}" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
          ${this.title ? `
            <div class="dialog-header">
              <h2 id="dialog-title" class="dialog-title">${escapeHtml(this.title)}</h2>
              <button class="dialog-close" onclick="this.closest('ui-dialog').close()" aria-label="Close">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="18" y1="6" x2="6" y2="18"></line>
                  <line x1="6" y1="6" x2="18" y2="18"></line>
                </svg>
              </button>
            </div>
          ` : ''}
          <div class="dialog-body">
            <slot></slot>
          </div>
          <div class="dialog-footer">
            <slot name="footer"></slot>
          </div>
        </div>
      </div>

      <style>
        :host {
          display: contents;
        }

        .dialog-backdrop {
          position: fixed;
          inset: 0;
          z-index: 100;
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 1rem;
          background-color: var(--modal-backdrop);
          animation: fadeIn 0.15s ease-out;
        }

        .dialog {
          position: relative;
          width: 100%;
          max-height: 90vh;
          overflow: hidden;
          display: flex;
          flex-direction: column;
          background-color: var(--bg-modal);
          border-radius: 0.5rem;
          box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
          animation: slideUp 0.2s ease-out;
        }

        .max-w-sm { max-width: 24rem; }
        .max-w-md { max-width: 28rem; }
        .max-w-lg { max-width: 32rem; }
        .max-w-xl { max-width: 36rem; }

        .dialog-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem;
          border-bottom: 1px solid var(--border-color);
        }

        .dialog-title {
          margin: 0;
          font-size: 1.125rem;
          font-weight: 600;
        }

        .dialog-close {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.25rem;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: color 0.15s ease, background-color 0.15s ease;
        }

        .dialog-close:hover {
          color: var(--text-color);
          background-color: var(--bg-input-hover);
        }

        .dialog-body {
          flex: 1;
          padding: 1rem;
          overflow-y: auto;
        }

        .dialog-footer {
          padding: 1rem;
          border-top: 1px solid var(--border-color);
          display: flex;
          justify-content: flex-end;
          gap: 0.5rem;
        }

        .dialog-footer:empty {
          display: none;
        }

        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }

        @keyframes slideUp {
          from {
            opacity: 0;
            transform: translateY(20px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      </style>
    `;
  }

  handleBackdropClick(event: MouseEvent): void {
    if (event.target === event.currentTarget) {
      this.close();
    }
  }

  close(): void {
    this.open = false;
    this.emit('close');
  }

  show(): void {
    this.open = true;
    this.emit('open');
  }
}
