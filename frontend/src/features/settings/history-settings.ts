/**
 * Import History settings page
 * Shows previously imported downloads with ability to clear them for reimport
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { type HistoryRecord, type HistoryResponse, http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

@customElement('history-settings')
export class HistorySettings extends BaseComponent {
  private page = signal(1);
  private pageSize = 50;

  private historyQuery = createQuery({
    queryKey: ['/history', 'imports', this.page.value, this.pageSize],
    queryFn: () =>
      http.get<HistoryResponse>('/history', {
        params: {
          page: this.page.value,
          pageSize: this.pageSize,
          eventType: 3,
          sortKey: 'date',
          sortDirection: 'descending',
        },
      }),
  });

  private removeItemMutation = createMutation({
    mutationFn: (id: number) => http.delete<void>(`/history/${id}`),
    onSuccess: () => {
      invalidateQueries(['/history']);
      showSuccess('Import record removed');
    },
    onError: () => {
      showError('Failed to remove record');
    },
  });

  private clearAllMutation = createMutation({
    mutationFn: () => http.delete<void>('/history?eventType=3'),
    onSuccess: () => {
      invalidateQueries(['/history']);
      showSuccess('All import history cleared');
    },
    onError: () => {
      showError('Failed to clear history');
    },
  });

  protected onInit(): void {
    this.watch(this.page);
    this.watch(this.historyQuery.data);
    this.watch(this.historyQuery.isLoading);
    this.watch(this.historyQuery.isError);
  }

  protected template(): string {
    const response = this.historyQuery.data.value;
    const records = response?.records ?? [];
    const totalRecords = response?.totalRecords ?? 0;
    const currentPage = this.page.value;
    const totalPages = Math.ceil(totalRecords / this.pageSize);
    const isLoading = this.historyQuery.isLoading.value;
    const isError = this.historyQuery.isError.value;

    return html`
      <div class="history-settings">
        <div class="section-header">
          <div class="header-left">
            <h2 class="section-title">Import History</h2>
            <span class="item-count">${totalRecords} records</span>
          </div>
          <div class="header-right">
            ${
              records.length > 0
                ? html`
              <button
                class="clear-btn"
                onclick="this.closest('history-settings').handleClearAll()"
              >
                Clear All Imports
              </button>
            `
                : ''
            }
            <button
              class="refresh-btn"
              onclick="this.closest('history-settings').handleRefresh()"
              title="Refresh"
            >
              <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="23 4 23 10 17 10"></polyline>
                <polyline points="1 20 1 14 7 14"></polyline>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
              </svg>
            </button>
          </div>
        </div>

        <p class="section-hint">
          Previously imported downloads are listed below. Removing a record allows the system to
          reimport that release if it is still available in your download client.
        </p>

        <div class="history-content">
          ${isLoading ? this.renderLoading() : ''}
          ${isError ? this.renderError() : ''}
          ${!isLoading && !isError ? this.renderContent(records) : ''}
        </div>

        ${totalPages > 1 ? this.renderPagination(currentPage, totalPages) : ''}
      </div>

      <style>
        .history-settings {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .section-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          flex-wrap: wrap;
          gap: 1rem;
        }

        .header-left {
          display: flex;
          align-items: baseline;
          gap: 0.75rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0;
        }

        .item-count {
          color: var(--text-color-muted);
          font-size: 0.875rem;
        }

        .header-right {
          display: flex;
          gap: 0.5rem;
        }

        .section-hint {
          margin: 0;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          line-height: 1.5;
        }

        .clear-btn {
          padding: 0.4rem 0.875rem;
          background-color: var(--btn-danger-bg);
          border: 1px solid var(--btn-danger-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.8125rem;
          cursor: pointer;
        }

        .clear-btn:hover {
          background-color: var(--btn-danger-bg-hover);
        }

        .refresh-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.4rem;
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
          padding: 3rem 2rem;
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

        /* Table */
        .history-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.8125rem;
        }

        .history-table th,
        .history-table td {
          padding: 0.625rem 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .history-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          white-space: nowrap;
          background-color: var(--bg-card-alt);
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.025em;
        }

        .history-table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

        .source-title {
          max-width: 450px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .quality-badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.6875rem;
          font-weight: 500;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
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

        /* Pagination */
        .pagination {
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 0.25rem;
          padding-top: 0.5rem;
        }

        .page-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          min-width: 32px;
          height: 32px;
          padding: 0 0.5rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.8125rem;
          cursor: pointer;
        }

        .page-btn:hover:not(:disabled) {
          background-color: var(--bg-input-hover);
        }

        .page-btn.active {
          background-color: var(--color-primary);
          border-color: var(--color-primary);
          color: var(--color-white);
        }

        .page-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .page-ellipsis {
          padding: 0 0.5rem;
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
        <p>Failed to load import history</p>
        <button class="refresh-btn" onclick="this.closest('history-settings').handleRefresh()">
          Retry
        </button>
      </div>
    `;
  }

  private renderContent(records: HistoryRecord[]): string {
    if (records.length === 0) {
      return html`
        <div class="empty-container">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
            <polyline points="22 4 12 14.01 9 11.01"></polyline>
          </svg>
          <p>No import history</p>
        </div>
      `;
    }

    return html`
      <table class="history-table">
        <thead>
          <tr>
            <th>Source Title</th>
            <th>Quality</th>
            <th>Date</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          ${records.map((record) => this.renderRow(record)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderRow(record: HistoryRecord): string {
    const date = new Date(record.date);
    const quality = record.quality?.quality?.name ?? '-';

    return html`
      <tr>
        <td>
          <div class="source-title" title="${escapeHtml(record.sourceTitle)}">
            ${escapeHtml(record.sourceTitle)}
          </div>
        </td>
        <td>
          <span class="quality-badge">${escapeHtml(quality)}</span>
        </td>
        <td class="date-cell">
          ${this.formatDate(date)}
        </td>
        <td>
          <button
            class="action-btn"
            onclick="this.closest('history-settings').handleRemove(${record.id})"
            title="Remove import record"
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

  private formatDate(date: Date): string {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));

    if (days === 0) {
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    if (days === 1) {
      return 'Yesterday';
    }
    if (days < 7) {
      return `${days} days ago`;
    }
    return date.toLocaleDateString();
  }

  private renderPagination(currentPage: number, totalPages: number): string {
    const pages: (number | string)[] = [];
    pages.push(1);

    if (currentPage > 3) {
      pages.push('...');
    }

    for (
      let i = Math.max(2, currentPage - 1);
      i <= Math.min(totalPages - 1, currentPage + 1);
      i++
    ) {
      if (!pages.includes(i)) {
        pages.push(i);
      }
    }

    if (currentPage < totalPages - 2) {
      pages.push('...');
    }

    if (totalPages > 1 && !pages.includes(totalPages)) {
      pages.push(totalPages);
    }

    return html`
      <div class="pagination">
        <button
          class="page-btn"
          ${currentPage === 1 ? 'disabled' : ''}
          onclick="this.closest('history-settings').goToPage(${currentPage - 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
        ${pages
          .map((p) =>
            typeof p === 'number'
              ? `<button class="page-btn ${p === currentPage ? 'active' : ''}" onclick="this.closest('history-settings').goToPage(${p})">${p}</button>`
              : `<span class="page-ellipsis">${p}</span>`,
          )
          .join('')}
        <button
          class="page-btn"
          ${currentPage === totalPages ? 'disabled' : ''}
          onclick="this.closest('history-settings').goToPage(${currentPage + 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
        </button>
      </div>
    `;
  }

  handleRefresh(): void {
    this.historyQuery.refetch();
  }

  handleRemove(id: number): void {
    if (confirm('Remove this import record? This allows the release to be reimported.')) {
      this.removeItemMutation.mutate(id);
    }
  }

  handleClearAll(): void {
    if (
      confirm(
        'Clear all import history? This removes all records of previously imported downloads.',
      )
    ) {
      this.clearAllMutation.mutate(undefined as never);
    }
  }

  goToPage(page: number): void {
    this.page.set(page);
    this.historyQuery.refetch();
  }
}
