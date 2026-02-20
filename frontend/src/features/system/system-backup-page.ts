/**
 * System Backup page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { showError, showSuccess } from '../../stores/app.store';

interface Backup {
  id: number;
  name: string;
  path: string;
  type: 'scheduled' | 'manual' | 'update';
  size: number;
  time: string;
}

@customElement('system-backup-page')
export class SystemBackupPage extends BaseComponent {
  private backupsQuery = createQuery({
    queryKey: ['/system/backup'],
    queryFn: () => http.get<Backup[]>('/system/backup'),
  });

  private createBackupMutation = createMutation({
    mutationFn: () => http.post('/command', { name: 'Backup' }),
    onSuccess: () => {
      invalidateQueries(['/system/backup']);
      showSuccess('Backup started');
    },
    onError: () => {
      showError('Failed to start backup');
    },
  });

  private deleteBackupMutation = createMutation({
    mutationFn: (id: number) => http.delete<void>(`/system/backup/${id}`),
    onSuccess: () => {
      invalidateQueries(['/system/backup']);
      showSuccess('Backup deleted');
    },
    onError: () => {
      showError('Failed to delete backup');
    },
  });

  protected onInit(): void {
    this.watch(this.backupsQuery.data);
    this.watch(this.backupsQuery.isLoading);
  }

  protected template(): string {
    const backups = this.backupsQuery.data.value ?? [];
    const isLoading = this.backupsQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="backup-page">
        <div class="toolbar">
          <h1 class="page-title">Backups</h1>
          <button
            class="backup-btn"
            onclick="this.closest('system-backup-page').handleCreateBackup()"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
              <polyline points="17 8 12 3 7 8"></polyline>
              <line x1="12" y1="3" x2="12" y2="15"></line>
            </svg>
            Backup Now
          </button>
        </div>

        <div class="backup-section">
          ${
            backups.length === 0
              ? html`
            <div class="empty-state">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
                <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                <polyline points="17 8 12 3 7 8"></polyline>
                <line x1="12" y1="3" x2="12" y2="15"></line>
              </svg>
              <p>No backups found</p>
            </div>
          `
              : html`
            <table class="backup-table">
              <thead>
                <tr>
                  <th>Name</th>
                  <th>Type</th>
                  <th>Size</th>
                  <th>Date</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                ${backups
                  .map(
                    (backup) => html`
                  <tr>
                    <td class="backup-name">${escapeHtml(backup.name)}</td>
                    <td>
                      <span class="type-badge ${backup.type}">${backup.type}</span>
                    </td>
                    <td>${this.formatBytes(backup.size)}</td>
                    <td class="date-cell">${new Date(backup.time).toLocaleString()}</td>
                    <td class="actions-cell">
                      <a
                        class="action-btn"
                        href="/api/v5/system/backup/${backup.id}"
                        download
                        title="Download"
                      >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                          <polyline points="7 10 12 15 17 10"></polyline>
                          <line x1="12" y1="15" x2="12" y2="3"></line>
                        </svg>
                      </a>
                      <button
                        class="action-btn danger"
                        onclick="this.closest('system-backup-page').handleDelete(${backup.id})"
                        title="Delete"
                      >
                        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <polyline points="3 6 5 6 21 6"></polyline>
                          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                        </svg>
                      </button>
                    </td>
                  </tr>
                `,
                  )
                  .join('')}
              </tbody>
            </table>
          `
          }
        </div>
      </div>

      <style>
        .backup-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .backup-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .backup-btn:hover {
          background-color: var(--btn-primary-bg-hover);
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

        .backup-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .empty-state {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 0.5rem;
          padding: 3rem;
          text-align: center;
          color: var(--text-color-muted);
        }

        .backup-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .backup-table th,
        .backup-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .backup-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          background-color: var(--bg-card-alt);
        }

        .backup-name {
          font-weight: 500;
          font-family: monospace;
        }

        .type-badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
          text-transform: capitalize;
        }

        .type-badge.scheduled {
          background-color: var(--color-primary);
          color: var(--color-white);
        }

        .type-badge.manual {
          background-color: var(--color-success);
          color: var(--color-white);
        }

        .type-badge.update {
          background-color: var(--color-warning);
          color: var(--color-white);
        }

        .date-cell {
          white-space: nowrap;
          color: var(--text-color-muted);
        }

        .actions-cell {
          display: flex;
          gap: 0.25rem;
        }

        .action-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          background: transparent;
          border: none;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          cursor: pointer;
          text-decoration: none;
        }

        .action-btn:hover {
          color: var(--color-primary);
          background-color: var(--bg-input-hover);
        }

        .action-btn.danger:hover {
          color: var(--color-danger);
        }
      </style>
    `;
  }

  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  handleCreateBackup(): void {
    this.createBackupMutation.mutate(undefined as never);
  }

  handleDelete(id: number): void {
    if (confirm('Are you sure you want to delete this backup?')) {
      this.deleteBackupMutation.mutate(id);
    }
  }
}
