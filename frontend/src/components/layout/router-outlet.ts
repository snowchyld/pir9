/**
 * Router outlet component
 * Placeholder for route-based content with page transitions
 */

import { BaseComponent, customElement } from '../../core/component';

@customElement('router-outlet')
export class RouterOutlet extends BaseComponent {
  protected template(): string {
    return `
      <div class="router-outlet">
        <div class="loading-state">
          <div class="spinner-container">
            <div class="spinner"></div>
            <div class="spinner-glow"></div>
          </div>
          <span class="loading-text">Loading</span>
        </div>
      </div>

      <style>
        :host {
          display: block;
        }

        .router-outlet {
          min-height: 200px;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        /* Page enter animation for child elements */
        .router-outlet > * {
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from {
            opacity: 0;
            transform: translateY(12px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        .loading-state {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 1.25rem;
          padding: 4rem 2rem;
          color: var(--text-color-muted);
          animation: fadeIn var(--transition-normal) var(--ease-out-expo);
        }

        .spinner-container {
          position: relative;
          width: 48px;
          height: 48px;
        }

        .spinner {
          position: absolute;
          inset: 0;
          border: 3px solid var(--border-glass);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        .spinner-glow {
          position: absolute;
          inset: -4px;
          border-radius: 50%;
          background: radial-gradient(
            circle at center,
            rgba(93, 156, 236, 0.2) 0%,
            transparent 70%
          );
          animation: glowPulse 2s ease-in-out infinite;
        }

        .loading-text {
          font-size: 0.875rem;
          font-weight: 500;
          letter-spacing: 0.05em;
        }

        .loading-text::after {
          content: '';
          animation: loadingDots 1.5s infinite;
        }

        @keyframes spin {
          to {
            transform: rotate(360deg);
          }
        }

        @keyframes glowPulse {
          0%, 100% {
            opacity: 0.5;
            transform: scale(1);
          }
          50% {
            opacity: 1;
            transform: scale(1.1);
          }
        }

        @keyframes loadingDots {
          0%, 20% { content: ''; }
          40% { content: '.'; }
          60% { content: '..'; }
          80%, 100% { content: '...'; }
        }

        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
      </style>
    `;
  }
}
