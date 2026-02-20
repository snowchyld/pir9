/**
 * IMDB Settings page
 * Provides sync controls, stats display, and search functionality for IMDB data
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

interface ImdbStats {
  seriesCount: number;
  episodeCount: number;
  lastBasicsSync: string | null;
  lastEpisodesSync: string | null;
  lastRatingsSync: string | null;
}

interface SyncStatus {
  titleBasics: SyncDataset | null;
  titleEpisodes: SyncDataset | null;
  titleRatings: SyncDataset | null;
}

interface SyncDataset {
  datasetName: string;
  rowsProcessed: number;
  rowsInserted: number;
  rowsUpdated: number;
  startedAt: string;
  completedAt: string | null;
  status: string;
  errorMessage: string | null;
}

interface ImdbSeries {
  imdbId: number;
  imdbIdFormatted: string;
  title: string;
  originalTitle: string | null;
  startYear: number | null;
  endYear: number | null;
  runtimeMinutes: number | null;
  genres: string[];
  isAdult: boolean;
  titleType: string;
  rating: number | null;
  votes: number | null;
  imdbUrl: string;
  isOngoing: boolean;
}

@customElement('imdb-settings')
export class ImdbSettings extends BaseComponent {
  // Queries
  private statsQuery = createQuery({
    queryKey: ['/imdb/stats'],
    queryFn: () => http.get<ImdbStats>('/imdb/stats'),
  });

  private syncStatusQuery = createQuery({
    queryKey: ['/imdb/sync/status'],
    queryFn: () => http.get<SyncStatus>('/imdb/sync/status'),
  });

  // Search state
  private searchTerm = signal('');
  private searchResults = signal<ImdbSeries[]>([]);
  private isSearching = signal(false);

  // Auto-refresh interval for running syncs
  private refreshInterval: number | null = null;

  // Mutations
  private syncMutation = createMutation({
    mutationFn: () => http.post('/imdb/sync', { datasets: [] }),
    onSuccess: () => {
      showSuccess('IMDB sync started in background');
      // Start auto-refresh and do initial refresh
      this.startAutoRefresh();
      setTimeout(() => {
        invalidateQueries(['/imdb/sync/status']);
        invalidateQueries(['/imdb/stats']);
      }, 1000);
    },
    onError: () => {
      showError('Failed to start IMDB sync');
    },
  });

  private backfillMutation = createMutation({
    mutationFn: (limit: number) => http.post('/imdb/backfill-air-dates', { limit }),
    onSuccess: () => {
      showSuccess('Air date backfill started in background');
    },
    onError: () => {
      showError('Failed to start air date backfill');
    },
  });

  private cancelStaleMutation = createMutation({
    mutationFn: () => http.post<{ cancelled: number }>('/imdb/sync/cancel-stale', {}),
    onSuccess: (data) => {
      this.stopAutoRefresh();
      invalidateQueries(['/imdb/sync/status']);
      showSuccess(`Cancelled ${data?.cancelled ?? 0} stale sync(s)`);
    },
    onError: () => {
      showError('Failed to cancel stale syncs');
    },
  });

  protected onInit(): void {
    this.watch(this.statsQuery.data);
    this.watch(this.statsQuery.isLoading);
    this.watch(this.syncStatusQuery.data);
    this.watch(this.searchResults);
    this.watch(this.isSearching);
    this.watch(this.searchTerm);

    // Check if a sync is already running and start auto-refresh
    this.checkAndStartAutoRefresh();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback?.();
    this.stopAutoRefresh();
  }

  private checkAndStartAutoRefresh(): void {
    const status = this.syncStatusQuery.data.value;
    if (this.isSyncRunning(status)) {
      this.startAutoRefresh();
    }
  }

  private isSyncRunning(status: SyncStatus | undefined): boolean {
    if (!status) return false;
    return (
      status.titleBasics?.status === 'running' ||
      status.titleEpisodes?.status === 'running' ||
      status.titleRatings?.status === 'running'
    );
  }

  private startAutoRefresh(): void {
    if (this.refreshInterval) return; // Already running

    this.refreshInterval = window.setInterval(() => {
      invalidateQueries(['/imdb/sync/status']);
      invalidateQueries(['/imdb/stats']);

      // Check if sync is still running
      const status = this.syncStatusQuery.data.value;
      if (!this.isSyncRunning(status)) {
        this.stopAutoRefresh();
        showSuccess('IMDB sync completed');
      }
    }, 5000); // Refresh every 5 seconds
  }

  private stopAutoRefresh(): void {
    if (this.refreshInterval) {
      clearInterval(this.refreshInterval);
      this.refreshInterval = null;
    }
  }

  protected template(): string {
    const stats = this.statsQuery.data.value;
    const syncStatus = this.syncStatusQuery.data.value;
    const isLoading = this.statsQuery.isLoading.value;
    const results = this.searchResults.value;
    const searching = this.isSearching.value;
    const term = this.searchTerm.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="settings-section">
        <h2 class="section-title">IMDB Database Statistics</h2>

        <div class="stats-grid">
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.seriesCount ?? 0)}</div>
            <div class="stat-label">Series</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.episodeCount ?? 0)}</div>
            <div class="stat-label">Episodes</div>
          </div>
        </div>

        <div class="sync-info">
          <h3 class="subsection-title">Last Sync Times</h3>
          <div class="sync-times">
            <div class="sync-time">
              <span class="sync-label">Title Basics:</span>
              <span class="sync-value">${this.formatDate(stats?.lastBasicsSync)}</span>
            </div>
            <div class="sync-time">
              <span class="sync-label">Episodes:</span>
              <span class="sync-value">${this.formatDate(stats?.lastEpisodesSync)}</span>
            </div>
            <div class="sync-time">
              <span class="sync-label">Ratings:</span>
              <span class="sync-value">${this.formatDate(stats?.lastRatingsSync)}</span>
            </div>
          </div>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Sync Controls</h2>

        <p class="section-description">
          Sync downloads IMDB's public datasets (title.basics, title.episode, title.ratings)
          and imports TV series data. This process can take 10-30 minutes depending on your connection.
        </p>

        <div class="button-group">
          <button
            class="primary-btn"
            onclick="this.closest('imdb-settings').handleStartSync()"
            ${this.syncMutation.isLoading.value ? 'disabled' : ''}
          >
            ${this.syncMutation.isLoading.value ? 'Starting...' : 'Start Full Sync'}
          </button>

          <button
            class="secondary-btn"
            onclick="this.closest('imdb-settings').handleRefreshStatus()"
          >
            Refresh Status
          </button>

          ${
            this.isSyncRunning(syncStatus)
              ? html`
            <button
              class="danger-btn"
              onclick="this.closest('imdb-settings').handleCancelStale()"
              ${this.cancelStaleMutation.isLoading.value ? 'disabled' : ''}
            >
              ${this.cancelStaleMutation.isLoading.value ? 'Cancelling...' : 'Cancel Stale Sync'}
            </button>
          `
              : ''
          }
        </div>

        ${this.renderSyncStatus(syncStatus)}
      </div>

      <div class="settings-section">
        <h2 class="section-title">TVMaze Air Dates</h2>

        <p class="section-description">
          IMDB doesn't include episode air dates. Use TVMaze to backfill air dates for series in the database.
        </p>

        <div class="button-group">
          <button
            class="secondary-btn"
            onclick="this.closest('imdb-settings').handleBackfillAirDates()"
            ${this.backfillMutation.isLoading.value ? 'disabled' : ''}
          >
            ${this.backfillMutation.isLoading.value ? 'Starting...' : 'Backfill Air Dates (100 series)'}
          </button>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Search IMDB</h2>

        <div class="search-container">
          <div class="search-input-group">
            <input
              type="text"
              class="form-input search-input"
              placeholder="Search series by title..."
              value="${escapeHtml(term)}"
              oninput="this.closest('imdb-settings').handleSearchInput(this.value)"
              onkeydown="if(event.key==='Enter') this.closest('imdb-settings').handleSearch()"
            />
            <button
              class="search-btn"
              onclick="this.closest('imdb-settings').handleSearch()"
              ${searching ? 'disabled' : ''}
            >
              ${searching ? 'Searching...' : 'Search'}
            </button>
          </div>
        </div>

        ${this.renderSearchResults(results, searching)}
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
          margin-bottom: 2rem;
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .subsection-title {
          font-size: 0.9375rem;
          font-weight: 600;
          margin: 1.5rem 0 0.75rem 0;
          color: var(--text-color);
        }

        .section-description {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin: 0 0 1.25rem 0;
          line-height: 1.5;
        }

        .stats-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
          gap: 1rem;
        }

        .stat-card {
          background-color: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          padding: 1.25rem;
          text-align: center;
        }

        .stat-value {
          font-size: 1.75rem;
          font-weight: 700;
          color: var(--color-primary);
        }

        .stat-label {
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          margin-top: 0.25rem;
        }

        .sync-times {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .sync-time {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          font-size: 0.875rem;
        }

        .sync-label {
          color: var(--text-color-muted);
          min-width: 100px;
        }

        .sync-value {
          color: var(--text-color);
          font-family: monospace;
        }

        .button-group {
          display: flex;
          gap: 0.75rem;
          flex-wrap: wrap;
        }

        .primary-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .primary-btn:hover:not(:disabled) {
          background-color: var(--btn-primary-bg-hover);
        }

        .primary-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .secondary-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .secondary-btn:hover:not(:disabled) {
          background-color: var(--btn-default-bg-hover);
        }

        .secondary-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .danger-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--color-danger, #dc3545);
          border: 1px solid var(--color-danger, #dc3545);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .danger-btn:hover:not(:disabled) {
          background-color: var(--color-danger-hover, #c82333);
        }

        .danger-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .sync-status {
          margin-top: 1.5rem;
          padding: 1rem;
          background-color: var(--bg-input);
          border-radius: 0.375rem;
          border: 1px solid var(--border-color);
        }

        .sync-status-title {
          font-size: 0.875rem;
          font-weight: 600;
          margin: 0 0 0.75rem 0;
        }

        .sync-dataset {
          margin-bottom: 0.75rem;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .sync-dataset:last-child {
          margin-bottom: 0;
          padding-bottom: 0;
          border-bottom: none;
        }

        .sync-dataset-name {
          font-weight: 500;
          font-size: 0.8125rem;
          margin-bottom: 0.25rem;
        }

        .sync-dataset-stats {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .sync-dataset-status {
          display: inline-block;
          padding: 0.125rem 0.5rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 500;
          margin-left: 0.5rem;
        }

        .status-completed {
          background-color: var(--color-success-bg);
          color: var(--color-success);
        }

        .status-running {
          background-color: var(--color-warning-bg);
          color: var(--color-warning);
        }

        .status-error {
          background-color: var(--color-danger-bg);
          color: var(--color-danger);
        }

        .search-container {
          margin-bottom: 1rem;
        }

        .search-input-group {
          display: flex;
          gap: 0.5rem;
          max-width: 500px;
        }

        .search-input {
          flex: 1;
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .search-input:focus {
          outline: none;
          border-color: var(--color-primary);
        }

        .search-btn {
          padding: 0.5rem 1rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          cursor: pointer;
          white-space: nowrap;
        }

        .search-btn:hover:not(:disabled) {
          background-color: var(--btn-primary-bg-hover);
        }

        .search-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .search-results {
          margin-top: 1rem;
        }

        .search-results-count {
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          margin-bottom: 0.75rem;
        }

        .result-card {
          display: flex;
          gap: 1rem;
          padding: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          margin-bottom: 0.5rem;
        }

        .result-card:hover {
          border-color: var(--color-primary);
        }

        .result-info {
          flex: 1;
          min-width: 0;
        }

        .result-title {
          font-weight: 600;
          font-size: 0.9375rem;
          margin-bottom: 0.25rem;
        }

        .result-title a {
          color: var(--text-color);
          text-decoration: none;
        }

        .result-title a:hover {
          color: var(--color-primary);
        }

        .result-meta {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          display: flex;
          flex-wrap: wrap;
          gap: 0.75rem;
        }

        .result-rating {
          display: flex;
          align-items: center;
          gap: 0.25rem;
        }

        .result-rating svg {
          width: 12px;
          height: 12px;
          fill: #f5c518;
        }

        .result-genres {
          display: flex;
          gap: 0.375rem;
          margin-top: 0.5rem;
          flex-wrap: wrap;
        }

        .genre-tag {
          padding: 0.125rem 0.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          font-size: 0.6875rem;
          color: var(--text-color-muted);
        }

        .no-results {
          text-align: center;
          padding: 2rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  private renderSyncStatus(status: SyncStatus | undefined): string {
    if (!status) return '';

    const hasAnyData = status.titleBasics || status.titleEpisodes || status.titleRatings;
    if (!hasAnyData) {
      return html`
        <div class="sync-status">
          <h4 class="sync-status-title">Sync Status</h4>
          <p class="section-description" style="margin: 0;">No sync has been performed yet.</p>
        </div>
      `;
    }

    return html`
      <div class="sync-status">
        <h4 class="sync-status-title">Sync Status</h4>
        ${this.renderDatasetStatus('Title Basics', status.titleBasics)}
        ${this.renderDatasetStatus('Episodes', status.titleEpisodes)}
        ${this.renderDatasetStatus('Ratings', status.titleRatings)}
      </div>
    `;
  }

  private renderDatasetStatus(name: string, dataset: SyncDataset | null): string {
    if (!dataset) return '';

    const statusClass =
      dataset.status === 'completed'
        ? 'status-completed'
        : dataset.status === 'running'
          ? 'status-running'
          : 'status-error';

    return html`
      <div class="sync-dataset">
        <div class="sync-dataset-name">
          ${name}
          <span class="sync-dataset-status ${statusClass}">${dataset.status}</span>
        </div>
        <div class="sync-dataset-stats">
          ${this.formatNumber(dataset.rowsProcessed)} rows processed |
          ${this.formatNumber(dataset.rowsInserted)} inserted |
          ${this.formatNumber(dataset.rowsUpdated)} updated
          ${dataset.completedAt ? ` | Completed: ${this.formatDate(dataset.completedAt)}` : ''}
        </div>
        ${dataset.errorMessage ? html`<div class="sync-dataset-stats" style="color: var(--color-danger);">Error: ${escapeHtml(dataset.errorMessage)}</div>` : ''}
      </div>
    `;
  }

  private renderSearchResults(results: ImdbSeries[], searching: boolean): string {
    if (searching) {
      return html`
        <div class="search-results">
          <div class="loading-container" style="padding: 2rem;">
            <div class="loading-spinner"></div>
          </div>
        </div>
      `;
    }

    if (results.length === 0) {
      return '';
    }

    return html`
      <div class="search-results">
        <div class="search-results-count">${results.length} result${results.length !== 1 ? 's' : ''} found</div>
        ${results.map((series) => this.renderSeriesCard(series)).join('')}
      </div>
    `;
  }

  private renderSeriesCard(series: ImdbSeries): string {
    const years = series.startYear
      ? series.endYear
        ? `${series.startYear}-${series.endYear}`
        : `${series.startYear}-`
      : 'Unknown';

    return html`
      <div class="result-card">
        <div class="result-info">
          <div class="result-title">
            <a href="${series.imdbUrl}" target="_blank" rel="noopener noreferrer">
              ${escapeHtml(series.title)}
            </a>
          </div>
          <div class="result-meta">
            <span>${years}</span>
            <span>${escapeHtml(series.imdbIdFormatted)}</span>
            ${
              series.rating
                ? html`
              <span class="result-rating">
                <svg viewBox="0 0 24 24"><path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/></svg>
                ${series.rating.toFixed(1)}
              </span>
            `
                : ''
            }
            ${series.votes ? html`<span>(${this.formatNumber(series.votes)} votes)</span>` : ''}
          </div>
          ${
            series.genres.length > 0
              ? html`
            <div class="result-genres">
              ${series.genres.map((g) => html`<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
            </div>
          `
              : ''
          }
        </div>
      </div>
    `;
  }

  private formatNumber(n: number): string {
    return n.toLocaleString();
  }

  private formatDate(dateStr: string | null | undefined): string {
    if (!dateStr) return 'Never';
    try {
      const date = new Date(dateStr);
      return date.toLocaleString();
    } catch {
      return dateStr;
    }
  }

  handleStartSync(): void {
    this.syncMutation.mutate(undefined);
  }

  handleRefreshStatus(): void {
    invalidateQueries(['/imdb/sync/status']);
    invalidateQueries(['/imdb/stats']);
    showSuccess('Status refreshed');
  }

  handleBackfillAirDates(): void {
    this.backfillMutation.mutate(100);
  }

  handleCancelStale(): void {
    this.cancelStaleMutation.mutate(undefined);
  }

  handleSearchInput(value: string): void {
    this.searchTerm.set(value);
  }

  async handleSearch(): Promise<void> {
    const term = this.searchTerm.value.trim();
    if (!term) {
      this.searchResults.set([]);
      return;
    }

    this.isSearching.set(true);
    try {
      const results = await http.get<ImdbSeries[]>(
        `/imdb/search?term=${encodeURIComponent(term)}&limit=25`,
      );
      this.searchResults.set(results);
    } catch (_e) {
      showError('Search failed');
      this.searchResults.set([]);
    } finally {
      this.isSearching.set(false);
    }
  }
}
