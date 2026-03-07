/**
 * Queue page showing download progress
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { http, type QueueItem, type QueueResponse } from '../../core/http';
import { createMutation, invalidateQueries, useQueueQuery } from '../../core/query';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';
import type { QueueMatchDialog } from './queue-match-dialog';

import './queue-match-dialog';

type QueueSortKey =
  | 'status'
  | 'title'
  | 'episode'
  | 'quality'
  | 'protocol'
  | 'progress'
  | 'timeleft';
type ContentTab = 'all' | 'shows' | 'movies' | 'anime' | 'completed';

// Module-level state survives component destruction/recreation (SPA navigation)
let savedSortKey: QueueSortKey = 'timeleft';
let savedSortDirection: 'asc' | 'desc' = 'asc';
let savedActiveTab: ContentTab = 'all';

@customElement('queue-page')
export class QueuePage extends BaseComponent {
  private queueQuery = useQueueQuery();
  private sortKey: QueueSortKey = savedSortKey;
  private sortDirection: 'asc' | 'desc' = savedSortDirection;
  private activeTab: ContentTab = savedActiveTab;
  private dialogOpen = false;

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

  private removeTrackedMutation = createMutation({
    mutationFn: (id: number) => http.delete<void>(`/queue/tracked/${id}`),
    onSuccess: () => {
      invalidateQueries(['/queue']);
      showSuccess('Removed completed item');
    },
    onError: () => {
      showError('Failed to remove completed item');
    },
  });

  private clearImportedMutation = createMutation({
    mutationFn: () => http.delete<void>('/queue/tracked', { params: { status: 4 } }),
    onSuccess: () => {
      invalidateQueries(['/queue']);
      showSuccess('Cleared imported download tracking — torrents will reappear for reimport');
    },
    onError: () => {
      showError('Failed to clear imported downloads');
    },
  });

  protected onInit(): void {
    this.watch(this.queueQuery.data);
    this.watch(this.queueQuery.isLoading);
    this.watch(this.queueQuery.isError);
  }

  // Suppress re-renders while the match dialog is open so the 5s poll
  // doesn't destroy the dialog DOM via innerHTML replacement
  requestUpdate(): void {
    if (this.dialogOpen) return;
    super.requestUpdate();
  }

  protected template(): string {
    const response = this.queueQuery.data.value as QueueResponse | undefined;
    const allItems = response?.records ?? [];
    const completedItems = response?.completedRecords ?? [];
    const isLoading = this.queueQuery.isLoading.value;
    const isError = this.queueQuery.isError.value;

    // Count per content type
    const showsCount = allItems.filter((i) => this.isShow(i)).length;
    const moviesCount = allItems.filter((i) => this.isMovie(i)).length;
    const animeCount = allItems.filter((i) => i.contentType === 'anime').length;
    const completedCount = completedItems.length;

    // Filter by active tab
    const items = this.activeTab === 'completed' ? completedItems : this.filterByTab(allItems);

    return html`
      <div class="queue-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Queue</h1>
            <span class="item-count">${items.length} items</span>
          </div>

          <div class="toolbar-right">
            ${
              this.activeTab === 'completed' && completedCount > 0
                ? `<button
                    class="clear-imported-btn"
                    onclick="this.closest('queue-page').handleClearImported()"
                    title="Clear all import tracking"
                  >
                    Clear All
                  </button>`
                : ''
            }
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

        <!-- Content type tabs -->
        <div class="content-tabs">
          ${safeHtml(this.renderTab('all', 'All', allItems.length))}
          ${safeHtml(this.renderTab('shows', 'Shows', showsCount))}
          ${safeHtml(this.renderTab('movies', 'Movies', moviesCount))}
          ${safeHtml(this.renderTab('anime', 'Anime', animeCount))}
          ${safeHtml(this.renderTab('completed', 'Completed', completedCount))}
        </div>

        <div class="queue-content">
          ${isLoading ? this.renderLoading() : ''}
          ${isError ? this.renderError() : ''}
          ${!isLoading && !isError ? this.renderContent(items) : ''}
        </div>
      </div>

      <queue-match-dialog></queue-match-dialog>

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

        /* Clear all button (Completed tab toolbar) */
        .clear-imported-btn {
          flex-shrink: 0;
          padding: 0.375rem 0.75rem;
          background-color: var(--color-danger, #e74c3c);
          color: var(--color-white, #fff);
          border: none;
          border-radius: 0.25rem;
          font-size: 0.8125rem;
          font-weight: 500;
          cursor: pointer;
          transition: opacity 0.15s;
        }

        .clear-imported-btn:hover {
          opacity: 0.85;
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
        .status-icon.stalled { color: var(--color-warning); }
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

        .progress-fill.stalled {
          background-color: var(--color-warning);
        }

        .progress-text {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .peer-info {
          font-size: 0.6875rem;
          color: var(--text-color-muted);
          margin-top: 0.125rem;
        }

        .peer-info.stalled {
          color: var(--color-danger);
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

        /* Content type tabs */
        .content-tabs {
          display: flex;
          gap: 0.25rem;
          padding: 0.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.625rem;
        }

        .content-tab {
          display: flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 1rem;
          background: transparent;
          border: none;
          border-radius: 0.5rem;
          color: var(--text-color-muted);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.15s ease;
        }

        .content-tab:hover {
          color: var(--text-color);
          background: var(--bg-input-hover);
        }

        .content-tab.active {
          background: var(--color-primary);
          color: var(--color-white);
        }

        .tab-count {
          display: inline-flex;
          align-items: center;
          justify-content: center;
          min-width: 1.25rem;
          height: 1.25rem;
          padding: 0 0.375rem;
          font-size: 0.6875rem;
          font-weight: 600;
          border-radius: 9999px;
          background: rgba(255, 255, 255, 0.15);
        }

        .content-tab:not(.active) .tab-count {
          background: var(--bg-progress);
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
            ${safeHtml(th('Title', 'title'))}
            ${safeHtml(th(this.activeTab === 'movies' ? 'File' : 'Episode', 'episode'))}
            ${safeHtml(th('Quality', 'quality'))}
            ${safeHtml(th('Protocol', 'protocol'))}
            ${safeHtml(th(this.activeTab === 'completed' ? 'Size' : 'Progress', 'progress'))}
            ${safeHtml(th(this.activeTab === 'completed' ? 'Date' : 'Time Left', 'timeleft'))}
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
    if (item.status === 'stalled') return false;
    const isCompleted =
      item.status === 'completed' || item.trackedDownloadState === 'importPending';
    if (!isCompleted) return false;
    // Must have a valid series or movie match in the DB (id > 0 means real DB record)
    const hasSeriesMatch = item.seriesId != null && item.seriesId > 0;
    const hasMovieMatch = item.movieId != null && item.movieId > 0;
    return hasSeriesMatch || hasMovieMatch;
  }

  private renderRow(item: QueueItem): string {
    const progress = item.size > 0 ? ((item.size - item.sizeleft) / item.size) * 100 : 0;
    const statusIcon = this.getStatusIcon(item.status);
    const isMovie = item.contentType === 'movie' || (item.movieId != null && item.movieId > 0);
    const displayTitle = isMovie
      ? (item.movie?.title ?? item.title)
      : (item.series?.title ?? item.title);
    const hasDbLink = isMovie
      ? item.movie?.titleSlug != null
      : item.seriesId != null && item.seriesId > 0 && item.series?.titleSlug != null;
    const linkPath = isMovie
      ? `/movies/${item.movie?.titleSlug ?? ''}`
      : `/series/${item.series?.titleSlug ?? ''}`;
    const linkSlug = isMovie ? (item.movie?.titleSlug ?? '') : (item.series?.titleSlug ?? '');
    const episodeLabel = isMovie
      ? item.outputPath
        ? (item.outputPath.split('/').pop() ?? item.title)
        : item.title
      : item.episode
        ? `S${String(item.episode.seasonNumber).padStart(2, '0')}E${String(item.episode.episodeNumber).padStart(2, '0')}${item.episode.title ? ` - ${item.episode.title}` : ''}`
        : '-';
    const importable = this.isImportable(item);
    const isCompleted = this.activeTab === 'completed';

    return html`
      <tr>
        <td>
          <div class="status-cell">
            ${safeHtml(statusIcon)}
            <span>${isCompleted ? 'imported' : importable ? 'ready to import' : escapeHtml(item.status)}</span>
          </div>
        </td>
        <td class="title-cell">
          ${
            hasDbLink
              ? `<a class="title-link" href="${escapeHtml(linkPath)}" onclick="event.preventDefault(); this.closest('queue-page').${isMovie ? 'handleMovieClick' : 'handleSeriesClick'}('${escapeHtml(linkSlug)}')" title="${escapeHtml(displayTitle)}">${escapeHtml(this.truncate(displayTitle, 32))}</a>`
              : `<span title="${escapeHtml(displayTitle)}">${escapeHtml(this.truncate(displayTitle, 32))}</span>`
          }
        </td>
        <td class="episode-cell" title="${isMovie ? `Source: ${escapeHtml(item.title)}` : escapeHtml(episodeLabel)}">
          <div>${escapeHtml(this.truncate(episodeLabel, 64))}</div>
        </td>
        <td>${escapeHtml(item.quality?.quality?.name ?? '-')}</td>
        <td>
          <span class="protocol-badge ${item.protocol}">${item.protocol}</span>
        </td>
        <td class="progress-cell">
          ${
            isCompleted
              ? `<div class="progress-text">${this.formatSize(item.size)}</div>`
              : `<div class="progress-bar">
                  <div class="progress-fill ${item.status === 'stalled' ? 'stalled' : ''}" style="width: ${progress}%"></div>
                </div>
                <div class="progress-text">
                  ${this.formatSize(item.size - item.sizeleft)} / ${this.formatSize(item.size)}
                </div>
                ${safeHtml(this.renderPeerInfo(item))}`
          }
        </td>
        <td>${isCompleted ? (item.added ? this.formatDate(item.added) : '-') : (item.timeleft ?? '-')}</td>
        <td>
          <div class="action-buttons">
            ${
              isCompleted
                ? `<button
                    class="action-btn danger"
                    onclick="this.closest('queue-page').handleRemoveCompleted(${item.id})"
                    title="Remove from completed (torrent will reappear for reimport)"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <line x1="18" y1="6" x2="6" y2="18"></line>
                      <line x1="6" y1="6" x2="18" y2="18"></line>
                    </svg>
                  </button>`
                : `<button
                    class="action-btn"
                    onclick="this.closest('queue-page').handleEditMatch(${item.id})"
                    title="Fix series/episode match"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
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
                  </button>`
            }
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
          stalled: 1,
          queued: 2,
          paused: 3,
          completed: 4,
          warning: 5,
          error: 6,
        };
        return priority[item.status.toLowerCase()] ?? 99;
      }
      case 'title':
        return (item.series?.title ?? item.title).toLowerCase();
      case 'episode':
        return item.episode ? item.episode.seasonNumber * 10000 + item.episode.episodeNumber : 0;
      case 'quality':
        return item.quality?.quality?.name?.toLowerCase() ?? '';
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
      stalled:
        '<svg class="status-icon stalled" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>',
      error:
        '<svg class="status-icon error" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>',
    };
    return icons[status.toLowerCase()] ?? icons.queued;
  }

  private renderPeerInfo(item: QueueItem): string {
    if (item.seeds == null && item.leechers == null) return '';
    const seeds = item.seeds ?? 0;
    const leechers = item.leechers ?? 0;
    const isStalled = item.status === 'stalled' || (seeds === 0 && leechers === 0);
    const cls = isStalled ? 'peer-info stalled' : 'peer-info';
    const seedLabel = seeds === 1 ? 'seed' : 'seeds';
    const leechLabel = leechers === 1 ? 'leecher' : 'leechers';
    return `<div class="${cls}">${seeds} ${seedLabel} · ${leechers} ${leechLabel}</div>`;
  }

  private truncate(text: string, max: number): string {
    return text.length > max ? `${text.slice(0, max)}\u2026` : text;
  }

  private formatDate(dateStr: string): string {
    const date = new Date(dateStr);
    if (Number.isNaN(date.getTime())) return '-';
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffDays = Math.floor(diffMs / 86400000);
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Yesterday';
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
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

  handleSeriesClick(slug: string): void {
    // Navigate to the appropriate detail page — slug is already the full path segment
    navigate(`/series/${slug}`);
  }

  handleMovieClick(slug: string): void {
    navigate(`/movies/${slug}`);
  }

  handleImport(id: number): void {
    navigate(`/activity/queue/${id}/import`);
  }

  handleRemove(id: number): void {
    if (confirm('Remove this item from the queue?')) {
      this.removeItemMutation.mutate({ id, removeFromClient: false });
    }
  }

  handleRemoveCompleted(id: number): void {
    if (confirm('Remove this completed item? The torrent will reappear for reimport.')) {
      this.removeTrackedMutation.mutate(id);
    }
  }

  handleClearImported(): void {
    if (
      confirm(
        'Clear all import tracking records? Previously imported torrents will reappear in the queue for reimport.',
      )
    ) {
      this.clearImportedMutation.mutate(undefined);
    }
  }

  handleEditMatch(id: number): void {
    const response = this.queueQuery.data.value as QueueResponse | undefined;
    const item = response?.records?.find((r) => r.id === id);
    if (!item) return;

    const dialog = this.querySelector('queue-match-dialog') as QueueMatchDialog | null;
    if (dialog) {
      this.dialogOpen = true;
      dialog.open(item, () => {
        this.dialogOpen = false;
        this.requestUpdate();
      });
    }
  }

  handleSort(key: QueueSortKey): void {
    if (this.sortKey === key) {
      this.sortDirection = this.sortDirection === 'asc' ? 'desc' : 'asc';
    } else {
      this.sortKey = key;
      this.sortDirection = 'asc';
    }
    savedSortKey = this.sortKey;
    savedSortDirection = this.sortDirection;
    this.requestUpdate();
  }

  handleTabClick(tab: ContentTab): void {
    this.activeTab = tab;
    savedActiveTab = this.activeTab;
    this.requestUpdate();
  }

  private renderTab(tab: ContentTab, label: string, count: number): string {
    const active = this.activeTab === tab;
    return `<button class="content-tab ${active ? 'active' : ''}" onclick="this.closest('queue-page').handleTabClick('${tab}')">${label}<span class="tab-count">${count}</span></button>`;
  }

  private isMovie(item: QueueItem): boolean {
    return item.contentType === 'movie' || (item.movieId != null && item.movieId > 0);
  }

  private isShow(item: QueueItem): boolean {
    if (item.contentType === 'anime') return false;
    if (item.contentType === 'series') return true;
    return item.seriesId != null && item.seriesId > 0;
  }

  private filterByTab(items: QueueItem[]): QueueItem[] {
    switch (this.activeTab) {
      case 'shows':
        return items.filter((i) => this.isShow(i));
      case 'movies':
        return items.filter((i) => this.isMovie(i));
      case 'anime':
        return items.filter((i) => i.contentType === 'anime');
      default:
        return items;
    }
  }
}
