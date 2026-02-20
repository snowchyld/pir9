/**
 * System Updates page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery } from '../../core/query';
import { showError, showSuccess } from '../../stores/app.store';

interface Update {
  version: string;
  branch: string;
  releaseDate: string;
  fileName: string;
  url: string;
  installed: boolean;
  installable: boolean;
  latest: boolean;
  changes: {
    version: string;
    date: string;
    changes: string[];
  };
}

@customElement('system-updates-page')
export class SystemUpdatesPage extends BaseComponent {
  private updatesQuery = createQuery({
    queryKey: ['/update'],
    queryFn: () => http.get<Update[]>('/update'),
  });

  private installUpdateMutation = createMutation({
    mutationFn: () => http.post('/command', { name: 'ApplicationUpdate' }),
    onSuccess: () => {
      showSuccess('Update started - pir9 will restart');
    },
    onError: () => {
      showError('Failed to start update');
    },
  });

  protected onInit(): void {
    this.watch(this.updatesQuery.data);
    this.watch(this.updatesQuery.isLoading);
  }

  protected template(): string {
    const updates = this.updatesQuery.data.value ?? [];
    const isLoading = this.updatesQuery.isLoading.value;
    const currentVersion = updates.find((u) => u.installed);
    const availableUpdate = updates.find((u) => u.latest && !u.installed);

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="updates-page">
        <h1 class="page-title">Updates</h1>

        <div class="current-section">
          <h2 class="section-title">Current Version</h2>
          <div class="version-info">
            <span class="version-number">${escapeHtml(currentVersion?.version ?? 'Unknown')}</span>
            <span class="version-branch">${escapeHtml(currentVersion?.branch ?? '')}</span>
          </div>
        </div>

        ${
          availableUpdate
            ? html`
          <div class="update-section available">
            <div class="update-header">
              <div class="update-info">
                <h2 class="section-title">Update Available</h2>
                <div class="version-info">
                  <span class="version-number">${escapeHtml(availableUpdate.version)}</span>
                  <span class="version-date">${new Date(availableUpdate.releaseDate).toLocaleDateString()}</span>
                </div>
              </div>
              ${
                availableUpdate.installable
                  ? html`
                <button
                  class="install-btn"
                  onclick="this.closest('system-updates-page').handleInstall()"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                    <polyline points="7 10 12 15 17 10"></polyline>
                    <line x1="12" y1="15" x2="12" y2="3"></line>
                  </svg>
                  Install Update
                </button>
              `
                  : ''
              }
            </div>

            ${
              availableUpdate.changes?.changes?.length > 0
                ? html`
              <div class="changelog">
                <h3 class="changelog-title">Changes</h3>
                <ul class="changes-list">
                  ${availableUpdate.changes.changes
                    .map(
                      (change) => html`
                    <li>${escapeHtml(change)}</li>
                  `,
                    )
                    .join('')}
                </ul>
              </div>
            `
                : ''
            }
          </div>
        `
            : html`
          <div class="update-section uptodate">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
              <polyline points="22 4 12 14.01 9 11.01"></polyline>
            </svg>
            <p>You're running the latest version</p>
          </div>
        `
        }

        <div class="history-section">
          <h2 class="section-title">Recent Updates</h2>
          <div class="history-list">
            ${updates
              .filter((u) => u.installed || !u.latest)
              .slice(0, 10)
              .map(
                (update) => html`
              <div class="history-item ${update.installed ? 'installed' : ''}">
                <div class="history-version">
                  <span class="version-number">${escapeHtml(update.version)}</span>
                  ${update.installed ? '<span class="installed-badge">Current</span>' : ''}
                </div>
                <div class="history-date">${new Date(update.releaseDate).toLocaleDateString()}</div>
              </div>
            `,
              )
              .join('')}
          </div>
        </div>
      </div>

      <style>
        .updates-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .loading-container {
          display: flex;
          justify-content: center;
          padding: 4rem;
        }

        .loading-spinner {
          width: 32px;
          height: 32px;
          border: 3px solid var(--border-color);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        @keyframes spin {
          to { transform: rotate(360deg); }
        }

        .current-section, .update-section, .history-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
        }

        .version-info {
          display: flex;
          align-items: baseline;
          gap: 0.75rem;
        }

        .version-number {
          font-size: 1.25rem;
          font-weight: 600;
        }

        .version-branch, .version-date {
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .update-section.available {
          border-color: var(--color-success);
          background-color: rgba(var(--color-success-rgb, 92, 184, 92), 0.05);
        }

        .update-section.uptodate {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 0.5rem;
          padding: 3rem;
          text-align: center;
          color: var(--color-success);
        }

        .update-header {
          display: flex;
          align-items: flex-start;
          justify-content: space-between;
          gap: 1rem;
        }

        .install-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background-color: var(--color-success);
          border: 1px solid var(--color-success);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .install-btn:hover {
          opacity: 0.9;
        }

        .changelog {
          margin-top: 1.5rem;
          padding-top: 1rem;
          border-top: 1px solid var(--border-color);
        }

        .changelog-title {
          font-size: 0.875rem;
          font-weight: 600;
          margin: 0 0 0.75rem 0;
        }

        .changes-list {
          margin: 0;
          padding-left: 1.25rem;
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .changes-list li {
          margin-bottom: 0.25rem;
        }

        .history-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .history-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 0.75rem;
          background-color: var(--bg-card-alt);
          border-radius: 0.375rem;
        }

        .history-item.installed {
          border: 1px solid var(--color-primary);
        }

        .history-version {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .history-version .version-number {
          font-size: 0.875rem;
        }

        .installed-badge {
          font-size: 0.625rem;
          padding: 0.125rem 0.375rem;
          background-color: var(--color-primary);
          color: var(--color-white);
          border-radius: 0.25rem;
          font-weight: 500;
        }

        .history-date {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  handleInstall(): void {
    if (confirm('Install update and restart pir9?')) {
      this.installUpdateMutation.mutate(undefined as never);
    }
  }
}
