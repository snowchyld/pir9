/**
 * Queue page showing download progress
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { http, type QueueItem, type QueueResponse } from '../../core/http';
import { createMutation, invalidateQueries, useQueueQuery } from '../../core/query';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

type QueueSortKey = 'status' | 'title' | 'episode' | 'protocol' | 'progress' | 'timeleft';

@customElement('queue-page')
export class QueuePage extends BaseComponent {
  private queueQuery = useQueueQuery();
  private sortKey: QueueSortKey = 'timeleft';
  private sortDirection: 'asc' | 'desc' = 'asc';

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

  private importItemMutation = createMutation({
    mutationFn: (id: number) => http.post<{ success: boolean }>(`/queue/${id}/import`),
    onSuccess: (result: { success: boolean }) => {
      if (result.success) {
        invalidateQueries(['/queue']);
        showSuccess('Download imported to library');
      } else {
        showError('Import failed — could not match series or episodes');
      }
    },
    onError: () => {
      showError('Failed to import download');
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
          table-layout: fixed;
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

        .queue-table th.sortable {
          cursor: pointer;
          user-select: none;
          transition: color 0.15s ease;
        }

        .queue-table th.sortable:hover {
          color: var(--pir9-blue, var(--color-primary));
        }

        .queue-table th.sortable.sorted {
          color: var(--pir9-blue, var(--color-primary));
        }

        .queue-table th .sort-icon {
          display: inline-block;
          vertical-align: middle;
          margin-left: 0.25rem;
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
        .status-icon.completed { color: var(--color-success, #2ecc71); }
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

        .action-btn.import {
          color: var(--color-success, #2ecc71);
        }

        .action-btn.import:hover {
          color: var(--color-white, #fff);
          background-color: var(--color-success, #2ecc71);
        }

        .action-buttons {
          display: flex;
          gap: 0.25rem;
          justify-content: flex-end;
        }

        .title-cell,
        .episode-cell {
          max-width: 0;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
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

    const sorted = this.sortItems(items);
    const sortIcon =
      this.sortDirection === 'asc'
        ? '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
        : '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>';

    const th = (label: string, key: QueueSortKey): string => {
      const isSorted = this.sortKey === key;
      return `<th class="sortable ${isSorted ? 'sorted' : ''}" onclick="this.closest('queue-page').handleSort('${key}')">${label}${isSorted ? sortIcon : ''}</th>`;
    };

    return html`
      <table class="queue-table">
        <colgroup>
          <col style="width: 10%">
          <col style="width: 15%">
          <col style="width: 22%">
          <col style="width: 8%">
          <col style="width: 8%">
          <col style="width: 18%">
          <col style="width: 10%">
          <col style="width: 6%">
        </colgroup>
        <thead>
          <tr>
            ${safeHtml(th('Status', 'status'))}
            ${safeHtml(th('Series', 'title'))}
            ${safeHtml(th('Episode', 'episode'))}
            <th>Quality</th>
            ${safeHtml(th('Protocol', 'protocol'))}
            ${safeHtml(th('Progress', 'progress'))}
            ${safeHtml(th('Time Left', 'timeleft'))}
            <th></th>
          </tr>
        </thead>
        <tbody>
          ${sorted.map((item) => this.renderRow(item)).join('')}
        </tbody>
      </table>
    `;
  }

  private isImportable(item: QueueItem): boolean {
    return item.status === 'completed' || item.trackedDownloadState === 'importPending';
  }

  private renderRow(item: QueueItem): string {
    const progress = item.size > 0 ? ((item.size - item.sizeleft) / item.size) * 100 : 0;
    const statusIcon = this.getStatusIcon(item.status);
    const seriesTitle = item.series?.title ?? item.title;
    const hasDbSeries = item.seriesId != null && item.seriesId > 0 && item.series?.titleSlug;
    const episodeLabel = item.episode
      ? `S${String(item.episode.seasonNumber).padStart(2, '0')}E${String(item.episode.episodeNumber).padStart(2, '0')}${item.episode.title ? ` - ${item.episode.title}` : ''}`
      : '-';
    const importable = this.isImportable(item);

    return html`
      <tr>
        <td>
          <div class="status-cell">
            ${safeHtml(statusIcon)}
            <span>${importable ? 'ready to import' : escapeHtml(item.status)}</span>
          </div>
        </td>
        <td class="title-cell">
          ${
            hasDbSeries
              ? `<a class="title-link" href="/series/${escapeHtml(item.series!.titleSlug)}" onclick="event.preventDefault(); this.closest('queue-page').handleSeriesClick('${escapeHtml(item.series!.titleSlug)}')" title="${escapeHtml(seriesTitle)}">${escapeHtml(this.truncate(seriesTitle, 32))}</a>`
              : `<span title="${escapeHtml(seriesTitle)}">${escapeHtml(this.truncate(seriesTitle, 32))}</span>`
          }
        </td>
        <td class="episode-cell" title="${escapeHtml(episodeLabel)}">
          <div>${escapeHtml(this.truncate(episodeLabel, 64))}</div>
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
          <div class="action-buttons">
            ${
              importable
                ? `<button
                    class="action-btn import"
                    onclick="this.closest('queue-page').handleImport(${item.id})"
                    title="Import to library"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                      <polyline points="7 10 12 15 17 10"></polyline>
                      <line x1="12" y1="15" x2="12" y2="3"></line>
                    </svg>
                  </button>`
                : ''
            }
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
          </div>
        </td>
      </tr>
    `;
  }

  private sortItems(items: QueueItem[]): QueueItem[] {
    return [...items].sort((a, b) => {
      const aVal = this.getSortValue(a, this.sortKey);
      const bVal = this.getSortValue(b, this.sortKey);
      let cmp = aVal < bVal ? -1 : aVal > bVal ? 1 : 0;
      if (this.sortDirection === 'desc') cmp = -cmp;
      return cmp;
    });
  }

  private getSortValue(item: QueueItem, key: QueueSortKey): string | number {
    switch (key) {
      case 'status': {
        const priority: Record<string, number> = {
          downloading: 0,
          queued: 1,
          paused: 2,
          completed: 3,
          warning: 4,
          error: 5,
        };
        return priority[item.status.toLowerCase()] ?? 99;
      }
      case 'title':
        return (item.series?.title ?? item.title).toLowerCase();
      case 'episode':
        return item.episode ? item.episode.seasonNumber * 10000 + item.episode.episodeNumber : 0;
      case 'protocol':
        return item.protocol;
      case 'progress':
        return item.size > 0 ? (item.size - item.sizeleft) / item.size : 0;
      case 'timeleft': {
        if (!item.timeleft) return Number.MAX_SAFE_INTEGER;
        return this.parseTimeleft(item.timeleft);
      }
      default:
        return 0;
    }
  }

  private parseTimeleft(timeleft: string): number {
    // Handles formats like "HH:MM:SS", "MM:SS", "D.HH:MM:SS"
    const dayParts = timeleft.split('.');
    let days = 0;
    let timePart = timeleft;
    if (dayParts.length === 2) {
      days = parseInt(dayParts[0], 10) || 0;
      timePart = dayParts[1];
    }
    const parts = timePart.split(':').map((p) => parseInt(p, 10) || 0);
    if (parts.length === 3) {
      return days * 86400 + parts[0] * 3600 + parts[1] * 60 + parts[2];
    }
    if (parts.length === 2) {
      return days * 86400 + parts[0] * 60 + parts[1];
    }
    return 0;
  }

  private getStatusIcon(status: string): string {
    const icons: Record<string, string> = {
      downloading:
        '<svg class="status-icon downloading animate-spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>',
      paused:
        '<svg class="status-icon paused" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="6" y="4" width="4" height="16"></rect><rect x="14" y="4" width="4" height="16"></rect></svg>',
      queued:
        '<svg class="status-icon queued" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>',
      completed:
        '<svg class="status-icon completed" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><polyline points="9 12 12 15 16 10"></polyline></svg>',
      error:
        '<svg class="status-icon error" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>',
    };
    return icons[status.toLowerCase()] ?? icons.queued;
  }

  private truncate(text: string, max: number): string {
    return text.length > max ? `${text.slice(0, max)}\u2026` : text;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  handleRefresh(): void {
    this.queueQuery.refetch();
  }

  handleSeriesClick(titleSlug: string): void {
    navigate(`/series/${titleSlug}`);
  }

  handleImport(id: number): void {
    this.importItemMutation.mutate(id);
  }

  handleRemove(id: number): void {
    if (confirm('Remove this item from the queue?')) {
      this.removeItemMutation.mutate({ id, removeFromClient: true });
    }
  }

  handleSort(key: QueueSortKey): void {
    if (this.sortKey === key) {
      this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      this.sortKey = key;
      this.sortDirection = 'asc';
    }
    this.requestUpdate();
  }
}
