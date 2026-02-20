/**
 * History page showing download history
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { type HistoryRecord, type HistoryResponse, http } from '../../core/http';
import { createQuery } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';

@customElement('history-page')
export class HistoryPage extends BaseComponent {
  private page = signal(1);
  private pageSize = 25;

  private historyQuery = createQuery({
    queryKey: ['/history', this.page.value, this.pageSize],
    queryFn: () =>
      http.get<HistoryResponse>('/history', {
        params: { page: this.page.value, pageSize: this.pageSize },
      }),
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
      <div class="history-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">History</h1>
            <span class="item-count">${totalRecords} records</span>
          </div>

          <div class="toolbar-right">
            <button
              class="refresh-btn"
              onclick="this.closest('history-page').handleRefresh()"
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

        <div class="history-content">
          ${isLoading ? this.renderLoading() : ''}
          ${isError ? this.renderError() : ''}
          ${!isLoading && !isError ? this.renderContent(records) : ''}
        </div>

        ${totalPages > 1 ? this.renderPagination(currentPage, totalPages) : ''}
      </div>

      <style>
        .history-page {
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

        /* History table */
        .history-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .history-table th,
        .history-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .history-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          white-space: nowrap;
          background-color: var(--bg-card-alt);
        }

        .history-table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

        /* Event type */
        .event-cell {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .event-icon {
          width: 20px;
          height: 20px;
        }

        .event-icon.grabbed { color: var(--color-primary); }
        .event-icon.imported { color: var(--color-success); }
        .event-icon.failed { color: var(--color-danger); }
        .event-icon.deleted { color: var(--color-warning); }

        .title-link {
          color: var(--link-color);
          text-decoration: none;
        }

        .title-link:hover {
          color: var(--link-hover);
        }

        .quality-badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          background-color: var(--bg-card);
          border-radius: 0.25rem;
        }

        .date-cell {
          white-space: nowrap;
          color: var(--text-color-muted);
        }

        /* Pagination */
        .pagination {
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 0.25rem;
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
          font-size: 0.875rem;
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
        <p>Failed to load history</p>
        <button class="refresh-btn" onclick="this.closest('history-page').handleRefresh()">
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
            <circle cx="12" cy="12" r="10"></circle>
            <polyline points="12 6 12 12 16 14"></polyline>
          </svg>
          <p>No history records</p>
        </div>
      `;
    }

    return html`
      <table class="history-table">
        <thead>
          <tr>
            <th>Event</th>
            <th>Source Title</th>
            <th>Quality</th>
            <th>Date</th>
          </tr>
        </thead>
        <tbody>
          ${records.map((record) => this.renderRow(record)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderRow(record: HistoryRecord): string {
    const eventIcon = this.getEventIcon(record.eventType);
    const date = new Date(record.date);
    const quality = record.quality?.quality?.name ?? '-';

    return html`
      <tr>
        <td>
          <div class="event-cell">
            ${safeHtml(eventIcon)}
            <span>${this.formatEventType(record.eventType)}</span>
          </div>
        </td>
        <td>
          <a class="title-link" href="/series/${record.seriesId}" onclick="event.preventDefault(); this.closest('history-page').handleSeriesClick(${record.seriesId})">
            ${escapeHtml(record.sourceTitle)}
          </a>
        </td>
        <td>
          <span class="quality-badge">${escapeHtml(quality)}</span>
        </td>
        <td class="date-cell">
          ${this.formatDate(date)}
        </td>
      </tr>
    `;
  }

  private getEventIcon(eventType: string): string {
    const icons: Record<string, string> = {
      grabbed:
        '<svg class="event-icon grabbed" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>',
      downloadFolderImported:
        '<svg class="event-icon imported" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>',
      downloadFailed:
        '<svg class="event-icon failed" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>',
      episodeFileDeleted:
        '<svg class="event-icon deleted" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"></polyline><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path></svg>',
    };
    return icons[eventType] ?? icons.grabbed;
  }

  private formatEventType(eventType: string): string {
    const names: Record<string, string> = {
      grabbed: 'Grabbed',
      downloadFolderImported: 'Imported',
      downloadFailed: 'Failed',
      episodeFileDeleted: 'Deleted',
      episodeFileRenamed: 'Renamed',
    };
    return names[eventType] ?? eventType;
  }

  private formatDate(date: Date): string {
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const days = Math.floor(diff / (1000 * 60 * 60 * 24));

    if (days === 0) {
      return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    } else if (days === 1) {
      return 'Yesterday';
    } else if (days < 7) {
      return `${days} days ago`;
    } else {
      return date.toLocaleDateString();
    }
  }

  private renderPagination(currentPage: number, totalPages: number): string {
    const pages: (number | string)[] = [];

    // Always show first page
    pages.push(1);

    // Show ellipsis or pages near current
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

    // Always show last page
    if (totalPages > 1 && !pages.includes(totalPages)) {
      pages.push(totalPages);
    }

    return html`
      <div class="pagination">
        <button
          class="page-btn"
          ?disabled="${currentPage === 1}"
          onclick="this.closest('history-page').goToPage(${currentPage - 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
        ${pages
          .map((p) =>
            typeof p === 'number'
              ? `<button class="page-btn ${p === currentPage ? 'active' : ''}" onclick="this.closest('history-page').goToPage(${p})">${p}</button>`
              : `<span class="page-ellipsis">${p}</span>`,
          )
          .join('')}
        <button
          class="page-btn"
          ?disabled="${currentPage === totalPages}"
          onclick="this.closest('history-page').goToPage(${currentPage + 1})"
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

  handleSeriesClick(seriesId: number): void {
    navigate(`/series/${seriesId}`);
  }

  goToPage(page: number): void {
    this.page.set(page);
    this.historyQuery.refetch();
  }
}
