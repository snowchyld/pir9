/**
 * Toast notification container
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { dismissToast, type Toast, toasts } from '../../stores/app.store';

@customElement('toast-container')
export class ToastContainer extends BaseComponent {
  protected onInit(): void {
    this.watch(toasts);
  }

  protected template(): string {
    const items = toasts.value;

    return html`
      <div class="toast-container" role="region" aria-label="Notifications">
        ${items.map((toast) => this.renderToast(toast)).join('')}
      </div>

      <style>
        .toast-container {
          position: fixed;
          bottom: 1.5rem;
          right: 1.5rem;
          z-index: 100;
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
          max-width: 420px;
          pointer-events: none;
        }

        .toast {
          display: flex;
          align-items: flex-start;
          gap: 0.875rem;
          padding: 1rem 1.25rem;
          background: var(--bg-popover);
          backdrop-filter: blur(var(--glass-blur-strong)) saturate(var(--glass-saturation));
          -webkit-backdrop-filter: blur(var(--glass-blur-strong)) saturate(var(--glass-saturation));
          border: 1px solid var(--border-glass);
          border-radius: 0.875rem;
          box-shadow: var(--shadow-popover);
          animation: toastSlideIn var(--transition-normal) var(--ease-out-expo);
          pointer-events: auto;
          position: relative;
          overflow: hidden;
        }

        .toast::before {
          content: '';
          position: absolute;
          left: 0;
          top: 0;
          bottom: 0;
          width: 4px;
        }

        .toast-info::before {
          background: linear-gradient(180deg, var(--color-info), rgba(93, 156, 236, 0.5));
          box-shadow: 0 0 12px var(--color-info);
        }

        .toast-success::before {
          background: linear-gradient(180deg, var(--color-success), rgba(39, 194, 76, 0.5));
          box-shadow: 0 0 12px var(--color-success);
        }

        .toast-warning::before {
          background: linear-gradient(180deg, var(--color-warning), rgba(255, 144, 43, 0.5));
          box-shadow: 0 0 12px var(--color-warning);
        }

        .toast-error::before {
          background: linear-gradient(180deg, var(--color-danger), rgba(240, 80, 80, 0.5));
          box-shadow: 0 0 12px var(--color-danger);
        }

        .toast-icon {
          flex-shrink: 0;
          width: 22px;
          height: 22px;
          filter: drop-shadow(0 0 4px currentColor);
        }

        .toast-icon.info { color: var(--color-info); }
        .toast-icon.success { color: var(--color-success); }
        .toast-icon.warning { color: var(--color-warning); }
        .toast-icon.error { color: var(--color-danger); }

        .toast-content {
          flex: 1;
          min-width: 0;
          padding-top: 1px;
        }

        .toast-title {
          font-weight: 600;
          font-size: 0.9rem;
          margin-bottom: 0.25rem;
        }

        .toast-message {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          word-break: break-word;
          line-height: 1.4;
        }

        .toast-close {
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          border-radius: 0.375rem;
          color: var(--text-color-muted);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all var(--transition-fast) var(--ease-out-expo);
        }

        .toast-close:hover {
          color: var(--text-color);
          background-color: var(--bg-input-hover);
          transform: scale(1.1);
        }

        .toast-close:active {
          transform: scale(0.95);
        }

        @keyframes toastSlideIn {
          from {
            opacity: 0;
            transform: translateX(100%) scale(0.9);
          }
          to {
            opacity: 1;
            transform: translateX(0) scale(1);
          }
        }

        /* Toast progress bar for auto-dismiss */
        .toast::after {
          content: '';
          position: absolute;
          bottom: 0;
          left: 0;
          right: 0;
          height: 2px;
          background: linear-gradient(90deg, transparent, rgba(255,255,255,0.2), transparent);
          animation: toastProgress 5s linear forwards;
        }

        @keyframes toastProgress {
          from { transform: scaleX(1); transform-origin: left; }
          to { transform: scaleX(0); transform-origin: left; }
        }

        @media (max-width: 480px) {
          .toast-container {
            left: 1rem;
            right: 1rem;
            bottom: 1rem;
            max-width: none;
          }

          .toast {
            border-radius: 0.75rem;
          }
        }
      </style>
    `;
  }

  private renderToast(toast: Toast): string {
    return html`
      <div
        class="toast toast-${toast.type}"
        role="alert"
        aria-live="polite"
      >
        ${this.renderIcon(toast.type)}
        <div class="toast-content">
          ${toast.title ? `<div class="toast-title">${escapeHtml(toast.title)}</div>` : ''}
          <div class="toast-message">${escapeHtml(toast.message)}</div>
        </div>
        <button
          class="toast-close"
          onclick="this.closest('toast-container').handleDismiss('${toast.id}')"
          aria-label="Dismiss"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18"></line>
            <line x1="6" y1="6" x2="18" y2="18"></line>
          </svg>
        </button>
      </div>
    `;
  }

  private renderIcon(type: Toast['type']): string {
    const icons: Record<Toast['type'], string> = {
      info: '<svg class="toast-icon info" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="16" x2="12" y2="12"></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg>',
      success:
        '<svg class="toast-icon success" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>',
      warning:
        '<svg class="toast-icon warning" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>',
      error:
        '<svg class="toast-icon error" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>',
    };

    return icons[type];
  }

  handleDismiss(id: string): void {
    dismissToast(id);
  }
}
