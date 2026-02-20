/**
 * Metadata Settings page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createQuery } from '../../core/query';
import { showSuccess } from '../../stores/app.store';

interface Metadata {
  id: number;
  name: string;
  enable: boolean;
  implementation: string;
}

@customElement('metadata-settings')
export class MetadataSettings extends BaseComponent {
  private metadataQuery = createQuery({
    queryKey: ['/metadata'],
    queryFn: () => http.get<Metadata[]>('/metadata'),
  });

  protected onInit(): void {
    this.watch(this.metadataQuery.data);
    this.watch(this.metadataQuery.isLoading);
  }

  protected template(): string {
    const metadata = this.metadataQuery.data.value ?? [];
    const isLoading = this.metadataQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="settings-section">
        <h2 class="section-title">Metadata</h2>
        <p class="section-description">
          Configure metadata providers to write additional files alongside your media.
        </p>

        <div class="metadata-list">
          ${metadata
            .map(
              (m) => html`
            <div class="metadata-card ${m.enable ? '' : 'disabled'}" onclick="this.closest('metadata-settings').handleEdit(${m.id})">
              <div class="metadata-info">
                <div class="metadata-name">${escapeHtml(m.name)}</div>
                <div class="metadata-type">${escapeHtml(m.implementation)}</div>
              </div>
              <div class="metadata-status">
                <span class="status-badge ${m.enable ? 'enabled' : 'disabled'}">
                  ${m.enable ? 'Enabled' : 'Disabled'}
                </span>
              </div>
            </div>
          `,
            )
            .join('')}
        </div>
      </div>

      <style>
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

        .settings-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 0.5rem 0;
        }

        .section-description {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin: 0 0 1.5rem 0;
        }

        .metadata-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .metadata-card {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem;
          background-color: var(--bg-card-alt);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          cursor: pointer;
          transition: border-color 0.15s;
        }

        .metadata-card:hover {
          border-color: var(--color-primary);
        }

        .metadata-card.disabled {
          opacity: 0.6;
        }

        .metadata-name {
          font-weight: 500;
          margin-bottom: 0.25rem;
        }

        .metadata-type {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .status-badge {
          font-size: 0.75rem;
          padding: 0.25rem 0.5rem;
          border-radius: 9999px;
          font-weight: 500;
        }

        .status-badge.enabled {
          background-color: var(--color-success);
          color: var(--color-white);
        }

        .status-badge.disabled {
          background-color: var(--bg-card);
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  handleEdit(id: number): void {
    showSuccess(`Edit metadata ${id}`);
  }
}
