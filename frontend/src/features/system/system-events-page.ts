/**
 * System Events page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createQuery } from '../../core/query';
import { signal } from '../../core/reactive';

interface CommandRecord {
  id: number;
  name: string;
  commandName: string;
  message: string;
  started: string;
  ended: string;
  duration: string;
  status: 'queued' | 'started' | 'completed' | 'failed' | 'cancelled';
  trigger: string;
}

interface CommandResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: CommandRecord[];
}

@customElement('system-events-page')
export class SystemEventsPage extends BaseComponent {
  private page = signal(1);
  private pageSize = 25;

  private eventsQuery = createQuery({
    queryKey: ['/command', this.page.value, this.pageSize],
    queryFn: () =>
      http.get<CommandResponse>('/command', {
        params: {
          page: this.page.value,
          pageSize: this.pageSize,
        },
      }),
  });

  protected onInit(): void {
    this.watch(this.eventsQuery.data);
    this.watch(this.eventsQuery.isLoading);
    this.watch(this.page);
  }

  protected template(): string {
    const response = this.eventsQuery.data.value;
    const events = response?.records ?? [];
    const totalRecords = response?.totalRecords ?? 0;
    const currentPage = this.page.value;
    const totalPages = Math.ceil(totalRecords / this.pageSize);
    const isLoading = this.eventsQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="events-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Events</h1>
            <span class="item-count">${totalRecords} events</span>
          </div>
          <button
            class="refresh-btn"
            onclick="this.closest('system-events-page').handleRefresh()"
            title="Refresh"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="23 4 23 10 17 10"></polyline>
              <polyline points="1 20 1 14 7 14"></polyline>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
            </svg>
          </button>
        </div>

        <div class="events-section">
          ${
            events.length === 0
              ? html`
            <div class="empty-state">No events found</div>
          `
              : html`
            <table class="events-table">
              <thead>
                <tr>
                  <th>Command</th>
                  <th>Message</th>
                  <th>Started</th>
                  <th>Duration</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                ${events
                  .map(
                    (event) => html`
                  <tr>
                    <td class="command-name">${escapeHtml(event.commandName)}</td>
                    <td class="message-cell">${escapeHtml(event.message || '-')}</td>
                    <td class="date-cell">${new Date(event.started).toLocaleString()}</td>
                    <td>${escapeHtml(event.duration || '-')}</td>
                    <td>
                      <span class="status-badge ${event.status}">${event.status}</span>
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

        ${totalPages > 1 ? this.renderPagination(currentPage, totalPages) : ''}
      </div>

      <style>
        .events-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
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

        .events-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .empty-state {
          padding: 2rem;
          text-align: center;
          color: var(--text-color-muted);
        }

        .events-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .events-table th,
        .events-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .events-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          background-color: var(--bg-card-alt);
        }

        .command-name {
          font-weight: 500;
        }

        .message-cell {
          max-width: 300px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          color: var(--text-color-muted);
        }

        .date-cell {
          white-space: nowrap;
          color: var(--text-color-muted);
        }

        .status-badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
          text-transform: capitalize;
        }

        .status-badge.completed {
          background-color: var(--color-success);
          color: var(--color-white);
        }

        .status-badge.started, .status-badge.queued {
          background-color: var(--color-primary);
          color: var(--color-white);
        }

        .status-badge.failed {
          background-color: var(--color-danger);
          color: var(--color-white);
        }

        .status-badge.cancelled {
          background-color: var(--color-warning);
          color: var(--color-white);
        }

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
      </style>
    `;
  }

  private renderPagination(currentPage: number, totalPages: number): string {
    return html`
      <div class="pagination">
        <button
          class="page-btn"
          ?disabled="${currentPage === 1}"
          onclick="this.closest('system-events-page').goToPage(${currentPage - 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
        <span class="page-btn active">${currentPage} / ${totalPages}</span>
        <button
          class="page-btn"
          ?disabled="${currentPage === totalPages}"
          onclick="this.closest('system-events-page').goToPage(${currentPage + 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
        </button>
      </div>
    `;
  }

  handleRefresh(): void {
    this.eventsQuery.refetch();
  }

  goToPage(page: number): void {
    this.page.set(page);
    this.eventsQuery.refetch();
  }
}
