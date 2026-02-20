/**
 * 404 Not Found page
 */

import { BaseComponent, customElement, html } from '../core/component';
import { navigate } from '../router';

@customElement('not-found-page')
export class NotFoundPage extends BaseComponent {
  protected template(): string {
    return html`
      <div class="not-found-page">
        <div class="not-found-content">
          <h1 class="error-code">404</h1>
          <h2 class="error-title">Page Not Found</h2>
          <p class="error-message">
            The page you're looking for doesn't exist or has been moved.
          </p>
          <button class="home-btn" onclick="this.closest('not-found-page').handleGoHome()">
            Go to Series
          </button>
        </div>
      </div>

      <style>
        .not-found-page {
          display: flex;
          align-items: center;
          justify-content: center;
          min-height: 60vh;
          padding: 2rem;
        }

        .not-found-content {
          text-align: center;
        }

        .error-code {
          font-size: 6rem;
          font-weight: 700;
          line-height: 1;
          color: var(--color-primary);
          margin: 0;
        }

        .error-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 1rem 0 0.5rem;
        }

        .error-message {
          color: var(--text-color-muted);
          margin: 0 0 1.5rem;
        }

        .home-btn {
          padding: 0.75rem 1.5rem;
          background-color: var(--btn-primary-bg);
          color: var(--color-white);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          font-weight: 500;
          cursor: pointer;
          transition: background-color 0.15s ease;
        }

        .home-btn:hover {
          background-color: var(--btn-primary-bg-hover);
        }
      </style>
    `;
  }

  handleGoHome(): void {
    navigate('/');
  }
}
