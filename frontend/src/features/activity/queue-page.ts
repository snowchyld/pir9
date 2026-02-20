/**
 * Queue page showing download progress
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { useQueueQuery, createMutation, invalidateQueries } from '../../core/query';
import { http, type QueueItem, type QueueResponse } from '../../core/http';
import { navigate } from '../../router';
import { showSuccess, showError } from '../../stores/app.store';

@customElement('queue-page')
export class QueuePage extends BaseComponent {
  private queueQuery = useQueueQuery();

  private removeItemMutation = createMutation({
    mutationFn: (params: { id: number; removeFromClient?: boolean; blocklist?: boolean }) =>
      http.delete<void>(`/queue/${params.id}`, {
        params: {
          removeFromClient: params.removeFromClient,
          blocklist: params.blocklist,
        },
      }),
    onSuccess: () => {
      invalidateQueries(['/queue']);
      showSuccess('Item removed from queue');
    },
    onError: () => {
      showError('Failed to remove item from queue');
    },
  });

  protected onInit(): void {
    this.watch(this.queueQuery.data);
    this.watch(this.queueQuery.isLoading);
    this.watch(this.queueQuery.isError);
  }

  protected template(): string {
    const response = this.queueQuery.data.value as QueueResponse | undefined;
    const items = response?.records ?? [];
    const isLoading = this.queueQuery.isLoading.value;
    const isError = this.queueQuery.isError.value;

    return html`
      <div class="queue-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Queue</h1>
            <span class="item-count">${items.length} items</span>
          </div>

          <div class="toolbar-right">
            <button
              class="refresh-btn"
              onclick="this.closest('queue-page').handleRefresh()"
              title="Refresh"
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="23 4 23 10 17 10"></polyline>
                <polyline points="1 20 1 14 7 14"></polyline>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
              </svg>
            </button>
          </div>
        </div>

        <div class="queue-content">
          ${isLoading ? this.renderLoading() : ''}
          ${isError ? this.renderError() : ''}
          ${!isLoading && !isError ? this.renderContent(items) : ''}
        </div>
      </div>

      <style>
        .queue-page {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          flex-wrap: wrap;
          gap: 1rem;
        }

        .toolbar-left {
          display: flex;
          align-items: baseline;
          gap: 1rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .item-count {
          color: var(--text-color-muted);
          font-size: 0.875rem;
        }

        .toolbar-right {
          display: flex;
          gap: 0.5rem;
        }

        .refresh-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.5rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          cursor: pointer;
        }

        .refresh-btn:hover {
          background-color: var(--btn-default-bg-hover);
        }

        /* Loading / Error */
        .loading-container, .error-container, .empty-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1rem;
          padding: 4rem 2rem;
          text-align: center;
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

        /* Queue table */
        .queue-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .queue-table th,
        .queue-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .queue-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          white-space: nowrap;
          background-color: var(--bg-card-alt);
        }

        .queue-table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

        /* Status column */
        .status-cell {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .status-icon {
          width: 16px;
          height: 16px;
        }

        .status-icon.downloading { color: var(--color-primary); }
        .status-icon.paused { color: var(--color-warning); }
        .status-icon.queued { color: var(--text-color-muted); }
        .status-icon.error { color: var(--color-danger); }

        /* Progress */
        .progress-cell {
          min-width: 150px;
        }

        .progress-bar {
          height: 6px;
          background-color: var(--bg-progress);
          border-radius: 3px;
          overflow: hidden;
          margin-bottom: 0.25rem;
        }

        .progress-fill {
          height: 100%;
          background-color: var(--color-primary);
          transition: width 0.3s ease;
        }

        .progress-text {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        /* Protocol badge */
        .protocol-badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
        }

        .protocol-badge.usenet {
          background-color: var(--color-usenet);
          color: var(--color-white);
        }

        .protocol-badge.torrent {
          background-color: var(--color-torrent);
          color: var(--color-white);
        }

        /* Actions */
        .action-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.25rem;
          background: transparent;
          border: none;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          cursor: pointer;
        }

        .action-btn:hover {
          color: var(--text-color);
          background-color: var(--bg-input-hover);
        }

        .action-btn.danger:hover {
          color: var(--color-danger);
        }

        .title-link {
          color: var(--link-color);
          text-decoration: none;
        }

        .title-link:hover {
          color: var(--link-hover);
        }

        .subtitle {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  private renderLoading(): string {
    return html`
      <div class="loading-container">
        <div class="loading-spinner"></div>
      </div>
    `;
  }

  private renderError(): string {
    return html`
      <div class="error-container">
        <p>Failed to load queue</p>
        <button class="refresh-btn" onclick="this.closest('queue-page').handleRefresh()">
          Retry
        </button>
      </div>
    `;
  }

  private renderContent(items: QueueItem[]): string {
    if (items.length === 0) {
      return html`
        <div class="empty-container">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
            <polyline points="7 10 12 15 17 10"></polyline>
            <line x1="12" y1="15" x2="12" y2="3"></line>
          </svg>
          <p>Queue is empty</p>
        </div>
      `;
    }

    return html`
      <table class="queue-table">
        <thead>
          <tr>
            <th>Status</th>
            <th>Series</th>
            <th>Episode</th>
            <th>Quality</th>
            <th>Protocol</th>
            <th>Progress</th>
            <th>Time Left</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          ${items.map((item) => this.renderRow(item)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderRow(item: QueueItem): string {
    const progress = item.size > 0 ? ((item.size - item.sizeleft) / item.size) * 100 : 0;
    const statusIcon = this.getStatusIcon(item.status);

    return html`
      <tr>
        <td>
          <div class="status-cell">
            ${safeHtml(statusIcon)}
            <span>${escapeHtml(item.status)}</span>
          </div>
        </td>
        <td>
          <a class="title-link" href="/series/${item.seriesId}" onclick="event.preventDefault(); this.closest('queue-page').handleSeriesClick(${item.seriesId})">
            ${escapeHtml(item.title.split(' - ')[0] ?? item.title)}
          </a>
        </td>
        <td>
          <div>${escapeHtml(item.title.split(' - ').slice(1).join(' - ') || '-')}</div>
        </td>
        <td>-</td>
        <td>
          <span class="protocol-badge ${item.protocol}">${item.protocol}</span>
        </td>
        <td class="progress-cell">
          <div class="progress-bar">
            <div class="progress-fill" style="width: ${progress}%"></div>
          </div>
          <div class="progress-text">
            ${this.formatSize(item.size - item.sizeleft)} / ${this.formatSize(item.size)}
          </div>
        </td>
        <td>${item.timeleft ?? '-'}</td>
        <td>
          <button
            class="action-btn danger"
            onclick="this.closest('queue-page').handleRemove(${item.id})"
            title="Remove from queue"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
          </button>
        </td>
      </tr>
    `;
  }

  private getStatusIcon(status: string): string {
    const icons: Record<string, string> = {
      downloading: '<svg class="status-icon downloading animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>',
      paused: '<svg class="status-icon paused" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="6" y="4" width="4" height="16"></rect><rect x="14" y="4" width="4" height="16"></rect></svg>',
      queued: '<svg class="status-icon queued" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>',
      error: '<svg class="status-icon error" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>',
    };
    return icons[status.toLowerCase()] ?? icons.queued;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(1))} ${sizes[i]}`;
  }

  handleRefresh(): void {
    this.queueQuery.refetch();
  }

  handleSeriesClick(seriesId: number): void {
    navigate(`/series/${seriesId}`);
  }

  handleRemove(id: number): void {
    if (confirm('Remove this item from the queue?')) {
      this.removeItemMutation.mutate({ id, removeFromClient: true });
    }
  }
}
