/**
 * Cutoff Unmet page - episodes below quality cutoff
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

interface CutoffEpisode {
  id: number;
  seriesId: number;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  airDate: string;
  episodeFile: {
    quality: {
      quality: {
        name: string;
      };
    };
  };
  series: {
    id: number;
    title: string;
    titleSlug: string;
  };
}

interface CutoffResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: CutoffEpisode[];
}

type SortKey = 'seriesTitle' | 'episodeNumber' | 'title' | 'airDateUtc';

@customElement('cutoff-unmet-page')
export class CutoffUnmetPage extends BaseComponent {
  private page = signal(1);
  private pageSize = 25;
  private sortKey = signal<SortKey>('airDateUtc');
  private sortDirection = signal<'ascending' | 'descending'>('descending');

  private cutoffQuery = createQuery({
    queryKey: [
      '/wanted/cutoff',
      this.page.value,
      this.pageSize,
      this.sortKey.value,
      this.sortDirection.value,
    ],
    queryFn: () =>
      http.get<CutoffResponse>('/wanted/cutoff', {
        params: {
          page: this.page.value,
          pageSize: this.pageSize,
          monitored: true,
          sortKey: this.sortKey.value,
          sortDirection: this.sortDirection.value,
        },
      }),
  });

  private searchMutation = createMutation({
    mutationFn: (episodeIds: number[]) =>
      http.post('/command', { name: 'EpisodeSearch', episodeIds }),
    onSuccess: () => {
      showSuccess('Search started');
    },
    onError: () => {
      showError('Failed to start search');
    },
  });

  protected onInit(): void {
    this.watch(this.page);
    this.watch(this.sortKey);
    this.watch(this.sortDirection);
    this.watch(this.cutoffQuery.data);
    this.watch(this.cutoffQuery.isLoading);
    this.watch(this.cutoffQuery.isError);
  }

  protected template(): string {
    const response = this.cutoffQuery.data.value;
    const episodes = response?.records ?? [];
    const totalRecords = response?.totalRecords ?? 0;
    const currentPage = this.page.value;
    const totalPages = Math.ceil(totalRecords / this.pageSize);
    const isLoading = this.cutoffQuery.isLoading.value;
    const isError = this.cutoffQuery.isError.value;

    return html`
      <div class="cutoff-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Cutoff Unmet</h1>
            <span class="item-count">${totalRecords} episodes</span>
          </div>

          <div class="toolbar-right">
            ${
              episodes.length > 0
                ? html`
              <button
                class="search-all-btn"
                onclick="this.closest('cutoff-unmet-page').handleSearchAll()"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="11" cy="11" r="8"></circle>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
                Search All
              </button>
            `
                : ''
            }
            <button
              class="refresh-btn"
              onclick="this.closest('cutoff-unmet-page').handleRefresh()"
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

        <div class="cutoff-content">
          ${isLoading ? this.renderLoading() : ''}
          ${isError ? this.renderError() : ''}
          ${!isLoading && !isError ? this.renderContent(episodes) : ''}
        </div>

        ${totalPages > 1 ? this.renderPagination(currentPage, totalPages) : ''}
      </div>

      <style>
        .cutoff-page {
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

        .search-all-btn {
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

        .search-all-btn:hover {
          background-color: var(--btn-primary-bg-hover);
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

        .cutoff-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .cutoff-table th,
        .cutoff-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .cutoff-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          white-space: nowrap;
          background-color: var(--bg-card-alt);
        }

        .cutoff-table th.sortable {
          cursor: pointer;
          user-select: none;
        }

        .cutoff-table th.sortable:hover {
          color: var(--text-color);
        }

        .cutoff-table th.sorted {
          color: var(--color-primary);
        }

        .sort-icon {
          vertical-align: middle;
          margin-left: 0.25rem;
        }

        .cutoff-table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

        .title-link {
          color: var(--link-color);
          text-decoration: none;
        }

        .title-link:hover {
          color: var(--link-hover);
        }

        .episode-number {
          color: var(--text-color-muted);
        }

        .quality-badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          background-color: var(--color-warning);
          color: var(--color-white);
          border-radius: 0.25rem;
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
          color: var(--color-primary);
          background-color: var(--bg-input-hover);
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
        <p>Failed to load cutoff unmet episodes</p>
        <button class="refresh-btn" onclick="this.closest('cutoff-unmet-page').handleRefresh()">
          Retry
        </button>
      </div>
    `;
  }

  private renderContent(episodes: CutoffEpisode[]): string {
    if (episodes.length === 0) {
      return html`
        <div class="empty-container">
          <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
            <polyline points="22 4 12 14.01 9 11.01"></polyline>
          </svg>
          <p>No episodes below cutoff</p>
        </div>
      `;
    }

    const th = (label: string, key: SortKey): string => {
      const isSorted = this.sortKey.value === key;
      const icon = isSorted
        ? this.sortDirection.value === 'ascending'
          ? '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
          : '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>'
        : '';
      return `<th class="sortable ${isSorted ? 'sorted' : ''}" onclick="this.closest('cutoff-unmet-page').handleSort('${key}')">${label}${icon}</th>`;
    };

    return html`
      <table class="cutoff-table">
        <thead>
          <tr>
            ${th('Series', 'seriesTitle')}
            ${th('Episode', 'episodeNumber')}
            ${th('Title', 'title')}
            <th>Current Quality</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          ${episodes.map((ep) => this.renderRow(ep)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderRow(episode: CutoffEpisode): string {
    const quality = episode.episodeFile?.quality?.quality?.name ?? 'Unknown';

    return html`
      <tr>
        <td>
          <a
            class="title-link"
            href="/series/${episode.series.titleSlug}"
            onclick="event.preventDefault(); this.closest('cutoff-unmet-page').handleSeriesClick('${episode.series.titleSlug}')"
          >
            ${escapeHtml(episode.series.title)}
          </a>
        </td>
        <td>
          <span class="episode-number">
            S${String(episode.seasonNumber).padStart(2, '0')}E${String(episode.episodeNumber).padStart(2, '0')}
          </span>
        </td>
        <td>${escapeHtml(episode.title)}</td>
        <td>
          <span class="quality-badge">${escapeHtml(quality)}</span>
        </td>
        <td>
          <button
            class="action-btn"
            onclick="this.closest('cutoff-unmet-page').handleSearch(${episode.id})"
            title="Search for episode"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
          </button>
        </td>
      </tr>
    `;
  }

  private renderPagination(currentPage: number, totalPages: number): string {
    return html`
      <div class="pagination">
        <button
          class="page-btn"
          ?disabled="${currentPage === 1}"
          onclick="this.closest('cutoff-unmet-page').goToPage(${currentPage - 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
        <span class="page-btn active">${currentPage} / ${totalPages}</span>
        <button
          class="page-btn"
          ?disabled="${currentPage === totalPages}"
          onclick="this.closest('cutoff-unmet-page').goToPage(${currentPage + 1})"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
        </button>
      </div>
    `;
  }

  handleRefresh(): void {
    this.cutoffQuery.refetch();
  }

  handleSeriesClick(titleSlug: string): void {
    navigate(`/series/${titleSlug}`);
  }

  handleSearch(episodeId: number): void {
    this.searchMutation.mutate([episodeId]);
  }

  handleSearchAll(): void {
    const episodes = this.cutoffQuery.data.value?.records ?? [];
    const episodeIds = episodes.map((e) => e.id);
    if (episodeIds.length > 0) {
      this.searchMutation.mutate(episodeIds);
    }
  }

  handleSort(key: SortKey): void {
    if (this.sortKey.value === key) {
      this.sortDirection.set(this.sortDirection.value === 'ascending' ? 'descending' : 'ascending');
    } else {
      this.sortKey.set(key);
      this.sortDirection.set('ascending');
    }
    this.page.set(1);
    this.cutoffQuery.refetch();
  }

  goToPage(page: number): void {
    this.page.set(page);
    this.cutoffQuery.refetch();
  }
}
