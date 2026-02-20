/**
 * Blocklist page showing blocked releases
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { http } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';

interface BlocklistItem {
  id: number;
  seriesId: number;
  sourceTitle: string;
  date: string;
  protocol: 'usenet' | 'torrent';
  indexer: string;
  message?: string;
}

interface BlocklistResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: BlocklistItem[];
}

@customElement('blocklist-page')
export class BlocklistPage extends BaseComponent {
  private blocklistQuery = createQuery({
    queryKey: ['/blocklist'],
    queryFn: () => http.get<BlocklistResponse>('/blocklist'),
  });

  private removeItemMutation = createMutation({
    mutationFn: (id: number) => http.delete<void>(`/blocklist/${id}`),
    onSuccess: () => {
      invalidateQueries(['/blocklist']);
      showSuccess('Item removed from blocklist');
    },
    onError: () => {
      showError('Failed to remove item');
    },
  });

  private clearAllMutation = createMutation({
    mutationFn: () => http.delete<void>('/blocklist/bulk'),
    onSuccess: () => {
      invalidateQueries(['/blocklist']);
      showSuccess('Blocklist cleared');
    },
    onError: () => {
      showError('Failed to clear blocklist');
    },
  });

  protected onInit(): void {
    this.watch(this.blocklistQuery.data);
    this.watch(this.blocklistQuery.isLoading);
    this.watch(this.blocklistQuery.isError);
  }

  protected template(): string {
    const response = this.blocklistQuery.data.value;
    const items = response?.records ?? [];
    const totalRecords = response?.totalRecords ?? 0;
    const isLoading = this.blocklistQuery.isLoading.value;
    const isError = this.blocklistQuery.isError.value;

    return html`
      <div class="blocklist-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Blocklist</h1>
            <span class="item-count">${totalRecords} items</span>
          </div>

          <div class="toolbar-right">
            ${items.length > 0 ? html`
              <button
                class="clear-btn"
                onclick="this.closest('blocklist-page').handleClearAll()"
              >
                Clear All
              </button>
            ` : ''}
            <button
              class="refresh-btn"
              onclick="this.closest('blocklist-page').handleRefresh()"
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

        <div class="blocklist-content">
          ${isLoading ? this.renderLoading() : ''}
          ${isError ? this.renderError() : ''}
          ${!isLoading && !isError ? this.renderContent(items) : ''}
        </div>
      </div>

      <style>
        .blocklist-page {
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

        .clear-btn {
          padding: 0.5rem 1rem;
          background-color: var(--btn-danger-bg);
          border: 1px solid var(--btn-danger-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .clear-btn:hover {
          background-color: var(--btn-danger-bg-hover);
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

        /* Blocklist table */
        .blocklist-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .blocklist-table th,
        .blocklist-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .blocklist-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          white-space: nowrap;
          background-color: var(--bg-card-alt);
        }

        .blocklist-table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

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

        .date-cell {
          white-space: nowrap;
          color: var(--text-color-muted);
        }

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
          color: var(--color-danger);
          background-color: var(--bg-input-hover);
        }

        .source-title {
          max-width: 400px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .message {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          max-width: 200px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
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
        <p>Failed to load blocklist</p>
        <button class="refresh-btn" onclick="this.closest('blocklist-page').handleRefresh()">
          Retry
        </button>
      </div>
    `;
  }

  private renderContent(items: BlocklistItem[]): string {
    if (items.length === 0) {
      return html`
        <div class="empty-container">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"></line>
          </svg>
          <p>Blocklist is empty</p>
        </div>
      `;
    }

    return html`
      <table class="blocklist-table">
        <thead>
          <tr>
            <th>Source Title</th>
            <th>Protocol</th>
            <th>Indexer</th>
            <th>Message</th>
            <th>Date</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          ${items.map((item) => this.renderRow(item)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderRow(item: BlocklistItem): string {
    const date = new Date(item.date);

    return html`
      <tr>
        <td>
          <div class="source-title" title="${escapeHtml(item.sourceTitle)}">
            ${escapeHtml(item.sourceTitle)}
          </div>
        </td>
        <td>
          <span class="protocol-badge ${item.protocol}">${item.protocol}</span>
        </td>
        <td>${escapeHtml(item.indexer)}</td>
        <td>
          <div class="message" title="${escapeHtml(item.message ?? '')}">
            ${escapeHtml(item.message ?? '-')}
          </div>
        </td>
        <td class="date-cell">
          ${date.toLocaleDateString()}
        </td>
        <td>
          <button
            class="action-btn"
            onclick="this.closest('blocklist-page').handleRemove(${item.id})"
            title="Remove from blocklist"
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

  handleRefresh(): void {
    this.blocklistQuery.refetch();
  }

  handleRemove(id: number): void {
    if (confirm('Remove this item from the blocklist?')) {
      this.removeItemMutation.mutate(id);
    }
  }

  handleClearAll(): void {
    if (confirm('Clear all items from the blocklist?')) {
      this.clearAllMutation.mutate(undefined as never);
    }
  }
}
