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
  movieCount: number;
  peopleCount: number;
  creditsCount: number;
  dbSizeBytes: number;
  lastSync: string | null;
  lastBasicsSync: string | null;
  lastEpisodesSync: string | null;
  lastRatingsSync: string | null;
}

interface SyncStatus {
  isRunning: boolean;
  titleBasics: SyncDataset | null;
  titleEpisodes: SyncDataset | null;
  titleRatings: SyncDataset | null;
  nameBasics: SyncDataset | null;
  titlePrincipals: SyncDataset | null;
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
  downloadProgress?: number;
  downloadSizeBytes?: number;
  downloadBytesDone?: number;
  currentPhase?: string;
}

interface DatasetInfo {
  name: string;
  remoteSize: number | null;
  localSize: number | null;
  localAge: string | null;
  cached: boolean;
}

interface MbStats {
  artistCount: number;
  releaseGroupCount: number;
  releaseCount: number;
  coverArtCount: number;
  labelCount: number;
  recordingCount: number;
  workCount: number;
  areaCount: number;
  seriesCount: number;
  eventCount: number;
  instrumentCount: number;
  placeCount: number;
  lastSync: string | null;
  dbSizeBytes: number;
}

interface MbSyncStatus {
  isRunning: boolean;
  artists: SyncDataset | null;
  releaseGroups: SyncDataset | null;
  releases: SyncDataset | null;
  labels: SyncDataset | null;
  recordings: SyncDataset | null;
  works: SyncDataset | null;
  areas: SyncDataset | null;
  series: SyncDataset | null;
  events: SyncDataset | null;
  instruments: SyncDataset | null;
  places: SyncDataset | null;
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

  // MusicBrainz queries and mutations
  private mbStatsQuery = createQuery({
    queryKey: ['/musicbrainz/stats'],
    queryFn: () => http.get<MbStats>('/musicbrainz/stats'),
  });

  private mbSyncMutation = createMutation({
    mutationFn: () => http.post('/musicbrainz/sync', {}),
    onSuccess: () => {
      showSuccess('MusicBrainz sync started in background');
      this.startMbAutoRefresh();
      setTimeout(() => {
        invalidateQueries(['/musicbrainz/stats']);
        invalidateQueries(['/musicbrainz/sync/status']);
      }, 1000);
    },
    onError: () => {
      showError('Failed to start MusicBrainz sync');
    },
  });

  private mbSyncStatusQuery = createQuery({
    queryKey: ['/musicbrainz/sync/status'],
    queryFn: () => http.get<MbSyncStatus>('/musicbrainz/sync/status'),
  });

  private mbCancelMutation = createMutation({
    mutationFn: () => http.post('/musicbrainz/sync/cancel', {}),
    onSuccess: () => {
      this.stopMbAutoRefresh();
      invalidateQueries(['/musicbrainz/sync/status']);
      showSuccess('MusicBrainz sync cancelled');
    },
    onError: () => {
      showError('Failed to cancel MusicBrainz sync');
    },
  });

  // IMDB dataset info
  private imdbDatasets = signal<DatasetInfo[]>([]);
  private imdbDatasetsLoading = signal(false);

  // MusicBrainz dataset info
  private mbDatasets = signal<DatasetInfo[]>([]);
  private mbDatasetsLoading = signal(false);

  private mbRefreshInterval: number | null = null;

  private startMbAutoRefresh(): void {
    if (this.mbRefreshInterval) return;
    this.mbRefreshInterval = window.setInterval(() => {
      invalidateQueries(['/musicbrainz/sync/status']);
      invalidateQueries(['/musicbrainz/stats']);

      // Check if sync is still running
      const status = this.mbSyncStatusQuery.data.value;
      if (status && !this.isMbSyncRunning(status)) {
        this.stopMbAutoRefresh();
        showSuccess('MusicBrainz sync completed');
      }
    }, 5000);
  }

  private stopMbAutoRefresh(): void {
    if (this.mbRefreshInterval) {
      clearInterval(this.mbRefreshInterval);
      this.mbRefreshInterval = null;
    }
  }

  protected onInit(): void {
    this.watch(this.statsQuery.data);
    this.watch(this.statsQuery.isLoading);
    this.watch(this.syncStatusQuery.data);
    this.watch(this.searchResults);
    this.watch(this.isSearching);
    this.watch(this.searchTerm);
    this.watch(this.mbStatsQuery.data);
    this.watch(this.mbSyncStatusQuery.data);
    this.watch(this.imdbDatasets);
    this.watch(this.imdbDatasetsLoading);
    this.watch(this.mbDatasets);
    this.watch(this.mbDatasetsLoading);

    // Check if a sync is already running and start auto-refresh (deferred to allow queries to load)
    setTimeout(() => {
      this.checkAndStartAutoRefresh();
      this.checkMbAutoRefresh();
    }, 2000);

    // Auto-load dataset sizes (server caches remote sizes, so this is fast)
    this.handleRefreshImdbDatasets();
    this.handleRefreshMbDatasets();
  }

  disconnectedCallback(): void {
    super.disconnectedCallback?.();
    this.stopAutoRefresh();
    this.stopMbAutoRefresh();
  }

  private checkMbAutoRefresh(): void {
    const status = this.mbSyncStatusQuery.data.value;
    if (this.isMbSyncRunning(status)) {
      this.startMbAutoRefresh();
    }
  }

  private isMbSyncRunning(status: MbSyncStatus | undefined): boolean {
    if (!status) return false;
    if (status.isRunning) return true;
    const allDatasets = [
      status.artists,
      status.releaseGroups,
      status.releases,
      status.labels,
      status.recordings,
      status.works,
      status.areas,
      status.series,
      status.events,
      status.instruments,
      status.places,
    ];
    return allDatasets.some((d) => d?.status === 'running');
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
      status.isRunning ||
      status.titleBasics?.status === 'running' ||
      status.titleEpisodes?.status === 'running' ||
      status.titleRatings?.status === 'running' ||
      status.nameBasics?.status === 'running' ||
      status.titlePrincipals?.status === 'running'
    );
  }

  private startAutoRefresh(): void {
    if (this.refreshInterval) return; // Already running

    this.refreshInterval = window.setInterval(() => {
      invalidateQueries(['/imdb/sync/status']);
      invalidateQueries(['/imdb/stats']);

      // Check if sync is still running
      const status = this.syncStatusQuery.data.value;
      if (status && !this.isSyncRunning(status)) {
        this.stopAutoRefresh();
        showSuccess('IMDB sync completed');
      }
    }, 5000);
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
        <h2 class="section-title">IMDB Database</h2>

        <div class="stats-grid">
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.seriesCount ?? 0)}</div>
            <div class="stat-label">Series</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.episodeCount ?? 0)}</div>
            <div class="stat-label">Episodes</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.movieCount ?? 0)}</div>
            <div class="stat-label">Movies</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.peopleCount ?? 0)}</div>
            <div class="stat-label">People</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">${this.formatNumber(stats?.creditsCount ?? 0)}</div>
            <div class="stat-label">Credits</div>
          </div>
          <div class="stat-card">
            <div class="stat-value">${this.formatSize(stats?.dbSizeBytes ?? 0)}</div>
            <div class="stat-label">DB Size</div>
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
            onclick="this.closest('imdb-settings').handleDownloadAllImdb()"
          >
            Download All
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

        ${this.renderImdbDatasetTable()}
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
        <h2 class="section-title">MusicBrainz</h2>

        <p class="section-description">
          MusicBrainz provides artist, album, and track metadata for the Music library.
          Sync downloads JSON dumps (~3 GB) and imports into the local database for offline querying.
        </p>

        ${this.renderMbStats()}
        ${this.renderMbSyncControls()}

        ${this.renderMbDatasetTable()}
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

        .download-progress-bar {
          height: 6px;
          background-color: var(--border-color);
          border-radius: 3px;
          margin-top: 0.375rem;
          overflow: hidden;
        }

        .download-progress-fill {
          height: 100%;
          background-color: var(--color-primary);
          border-radius: 3px;
          transition: width 0.3s ease;
        }

        .dataset-section {
          margin-top: 1.5rem;
          padding-top: 1rem;
          border-top: 1px solid var(--border-color);
        }

        .dataset-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.8125rem;
        }

        .dataset-table th,
        .dataset-table td {
          padding: 0.5rem 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .dataset-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.025em;
        }

        .dataset-table tbody tr:hover {
          background-color: var(--bg-input);
        }

        .dataset-name-cell {
          font-family: monospace;
          font-size: 0.8125rem;
        }

        .dataset-actions {
          display: flex;
          gap: 0.375rem;
        }

        .cache-badge {
          display: inline-block;
          padding: 0.0625rem 0.375rem;
          border-radius: 0.25rem;
          font-size: 0.6875rem;
          font-weight: 500;
        }

        .cache-yes {
          background-color: var(--color-success-bg);
          color: var(--color-success);
        }

        .cache-no {
          background-color: var(--bg-input);
          color: var(--text-color-muted);
        }

        .small-btn {
          padding: 0.25rem 0.5rem;
          font-size: 0.75rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          cursor: pointer;
          white-space: nowrap;
        }

        .small-btn:hover:not(:disabled) {
          background-color: var(--btn-default-bg-hover);
        }

        .small-btn:disabled {
          opacity: 0.4;
          cursor: not-allowed;
        }
      </style>
    `;
  }

  private renderSyncStatus(status: SyncStatus | undefined): string {
    if (!status) return '';

    const hasAnyData =
      status.titleBasics ||
      status.titleEpisodes ||
      status.titleRatings ||
      status.nameBasics ||
      status.titlePrincipals;
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
        ${this.renderDatasetStatus('People', status.nameBasics)}
        ${this.renderDatasetStatus('Credits', status.titlePrincipals)}
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

    const isDownloading = dataset.currentPhase === 'downloading';
    const downloadPct = dataset.downloadProgress ?? 0;

    return html`
      <div class="sync-dataset">
        <div class="sync-dataset-name">
          ${name}
          <span class="sync-dataset-status ${statusClass}">${dataset.status}</span>
        </div>
        ${
          isDownloading
            ? html`
          <div class="sync-dataset-stats">
            Downloading ${escapeHtml(dataset.datasetName)}... ${downloadPct.toFixed(1)}%
            ${dataset.downloadSizeBytes ? ` (${this.formatBytes(dataset.downloadBytesDone ?? 0)} / ${this.formatBytes(dataset.downloadSizeBytes)})` : ''}
          </div>
          <div class="download-progress-bar">
            <div class="download-progress-fill" style="width: ${Math.min(downloadPct, 100)}%"></div>
          </div>
        `
            : html`
          <div class="sync-dataset-stats">
            ${dataset.currentPhase === 'parsing' && dataset.status === 'running' ? 'Parsing... ' : ''}${this.formatNumber(dataset.rowsProcessed)} rows processed |
            ${this.formatNumber(dataset.rowsInserted)} inserted |
            ${this.formatNumber(dataset.rowsUpdated)} updated
            ${dataset.completedAt ? ` | Completed: ${this.formatDate(dataset.completedAt)}` : ''}
          </div>
        `
        }
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

  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const val = bytes / 1024 ** i;
    return `${val.toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
  }

  private formatSize(bytes: number | null): string {
    if (bytes === null || bytes === undefined) return '--';
    if (bytes === 0) return '0 B';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    const val = bytes / 1024 ** i;
    return `${val.toFixed(i > 0 ? 1 : 0)} ${units[i]}`;
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

  // MusicBrainz handlers
  handleStartMbSync(): void {
    this.mbSyncMutation.mutate(undefined);
  }

  handleCancelMbSync(): void {
    this.mbCancelMutation.mutate(undefined);
  }

  handleRefreshMbStatus(): void {
    invalidateQueries(['/musicbrainz/stats']);
    invalidateQueries(['/musicbrainz/sync/status']);
    showSuccess('MusicBrainz status refreshed');
  }

  // Dataset handlers

  async handleRefreshImdbDatasets(): Promise<void> {
    this.imdbDatasetsLoading.set(true);
    try {
      const data = await http.get<DatasetInfo[]>('/imdb/datasets');
      this.imdbDatasets.set(data);
    } catch {
      showError('Failed to fetch IMDB dataset info');
    } finally {
      this.imdbDatasetsLoading.set(false);
    }
  }

  async handleRefreshMbDatasets(): Promise<void> {
    this.mbDatasetsLoading.set(true);
    try {
      const data = await http.get<DatasetInfo[]>('/musicbrainz/datasets');
      this.mbDatasets.set(data);
    } catch {
      showError('Failed to fetch MusicBrainz dataset info');
    } finally {
      this.mbDatasetsLoading.set(false);
    }
  }

  async handleDownloadImdbDataset(name: string): Promise<void> {
    try {
      await http.post('/imdb/download', { datasets: [name] });
      showSuccess(`Downloading ${name}...`);
      this.startAutoRefresh();
    } catch {
      showError(`Failed to start download for ${name}`);
    }
  }

  async handleProcessImdbDataset(name: string): Promise<void> {
    try {
      await http.post('/imdb/process', { datasets: [name] });
      showSuccess(`Processing ${name}...`);
      this.startAutoRefresh();
    } catch {
      showError(`Failed to start processing for ${name}`);
    }
  }

  async handleDownloadAllImdb(): Promise<void> {
    try {
      await http.post('/imdb/download', { datasets: [] });
      showSuccess('Downloading all IMDB datasets...');
      this.startAutoRefresh();
    } catch {
      showError('Failed to start IMDB download');
    }
  }

  async handleDownloadMbDataset(name: string): Promise<void> {
    try {
      await http.post('/musicbrainz/download', { datasets: [name] });
      showSuccess(`Downloading ${name}...`);
      this.startMbAutoRefresh();
    } catch {
      showError(`Failed to start download for ${name}`);
    }
  }

  async handleProcessMbDataset(name: string): Promise<void> {
    try {
      await http.post('/musicbrainz/process', { datasets: [name] });
      showSuccess(`Processing ${name}...`);
      this.startMbAutoRefresh();
    } catch {
      showError(`Failed to start processing for ${name}`);
    }
  }

  async handleDownloadAllMb(): Promise<void> {
    try {
      await http.post('/musicbrainz/download', { datasets: [] });
      showSuccess('Downloading all MusicBrainz datasets...');
      this.startMbAutoRefresh();
    } catch {
      showError('Failed to start MusicBrainz download');
    }
  }

  private renderMbStats(): string {
    const stats = this.mbStatsQuery.data.value;
    if (!stats) {
      return '<div class="stats-loading">Loading MusicBrainz stats...</div>';
    }

    return html`
      <div class="sync-info">
        <h3 class="subsection-title">Database Statistics</h3>
        <div class="stats-grid">
          <div class="stat-card">
            <span class="stat-value">${(stats.artistCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Artists</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.releaseGroupCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Albums</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.releaseCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Releases</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.recordingCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Recordings</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.labelCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Labels</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.workCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Works</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.areaCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Areas</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.eventCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Events</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.instrumentCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Instruments</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.placeCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Places</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.seriesCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Series</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${(stats.coverArtCount ?? 0).toLocaleString()}</span>
            <span class="stat-label">Cover Art</span>
          </div>
          <div class="stat-card">
            <span class="stat-value">${this.formatSize(stats.dbSizeBytes ?? 0)}</span>
            <span class="stat-label">DB Size</span>
          </div>
        </div>
        ${
          stats.lastSync
            ? html`
          <div class="sync-times" style="margin-top: 0.5rem;">
            <div class="sync-time">
              <span class="sync-label">Last Sync:</span>
              <span class="sync-value">${this.formatDate(stats.lastSync)}</span>
            </div>
          </div>
        `
            : ''
        }
      </div>
    `;
  }

  private renderMbSyncControls(): string {
    const syncStatus = this.mbSyncStatusQuery.data.value;
    const isRunning = this.isMbSyncRunning(syncStatus);

    return html`
      <div class="button-group" style="margin-top: 1rem;">
        <button
          class="primary-btn"
          onclick="this.closest('imdb-settings').handleStartMbSync()"
          ${this.mbSyncMutation.isLoading.value || isRunning ? 'disabled' : ''}
        >
          ${this.mbSyncMutation.isLoading.value ? 'Starting...' : isRunning ? 'Sync Running...' : 'Start MusicBrainz Sync'}
        </button>

        <button
          class="secondary-btn"
          onclick="this.closest('imdb-settings').handleRefreshMbStatus()"
        >
          Refresh Status
        </button>

        ${
          isRunning
            ? html`
          <button
            class="danger-btn"
            onclick="this.closest('imdb-settings').handleCancelMbSync()"
            ${this.mbCancelMutation.isLoading.value ? 'disabled' : ''}
          >
            ${this.mbCancelMutation.isLoading.value ? 'Cancelling...' : 'Cancel Sync'}
          </button>
        `
            : ''
        }
      </div>

      ${this.renderMbSyncStatus(syncStatus)}
    `;
  }

  private renderMbSyncStatus(status: MbSyncStatus | undefined): string {
    if (!status) return '';

    const datasets: [string, SyncDataset | null][] = [
      ['Instruments', status.instruments ?? null],
      ['Areas', status.areas ?? null],
      ['Series', status.series ?? null],
      ['Events', status.events ?? null],
      ['Places', status.places ?? null],
      ['Labels', status.labels ?? null],
      ['Artists', status.artists ?? null],
      ['Works', status.works ?? null],
      ['Recordings', status.recordings ?? null],
      ['Release Groups', status.releaseGroups ?? null],
      ['Releases', status.releases ?? null],
    ];

    const hasAnyData = datasets.some(([, d]) => d !== null);
    if (!hasAnyData) return '';

    return html`
      <div class="sync-status">
        <h4 class="sync-status-title">MusicBrainz Sync Status</h4>
        ${datasets
          .filter(([, d]) => d !== null)
          .map(([name, d]) => this.renderDatasetStatus(name, d))
          .join('')}
      </div>
    `;
  }

  private renderImdbDatasetTable(): string {
    const datasets = this.imdbDatasets.value;
    const loading = this.imdbDatasetsLoading.value;

    return html`
      <div class="dataset-section">
        <h3 class="subsection-title">IMDB Datasets</h3>
        <div class="button-group" style="margin-bottom: 0.75rem;">
          <button
            class="secondary-btn"
            onclick="this.closest('imdb-settings').handleRefreshImdbDatasets()"
            ${loading ? 'disabled' : ''}
          >
            ${loading ? 'Loading...' : 'Refresh Sizes'}
          </button>
          ${
            datasets.length > 0
              ? html`
            <button
              class="secondary-btn"
              onclick="this.closest('imdb-settings').handleDownloadAllImdb()"
            >
              Download All
            </button>
          `
              : ''
          }
        </div>
        ${
          datasets.length > 0
            ? html`
          <table class="dataset-table">
            <thead>
              <tr>
                <th>Dataset</th>
                <th>Remote Size</th>
                <th>Cached</th>
                <th>Cache Age</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              ${datasets
                .map(
                  (ds) => html`
                <tr>
                  <td class="dataset-name-cell">${escapeHtml(ds.name)}</td>
                  <td>${this.formatSize(ds.remoteSize)}</td>
                  <td>
                    <span class="cache-badge ${ds.cached ? 'cache-yes' : 'cache-no'}">
                      ${ds.cached ? 'Yes' : 'No'}
                    </span>
                  </td>
                  <td>${ds.localAge ?? '--'}</td>
                  <td class="dataset-actions">
                    <button
                      class="small-btn"
                      onclick="this.closest('imdb-settings').handleDownloadImdbDataset('${escapeHtml(ds.name)}')"
                    >Download</button>
                    <button
                      class="small-btn"
                      onclick="this.closest('imdb-settings').handleProcessImdbDataset('${escapeHtml(ds.name)}')"
                      ${ds.cached ? '' : 'disabled'}
                    >Process</button>
                  </td>
                </tr>
              `,
                )
                .join('')}
            </tbody>
          </table>
        `
            : ''
        }
      </div>
    `;
  }

  private renderMbDatasetTable(): string {
    const datasets = this.mbDatasets.value;
    const loading = this.mbDatasetsLoading.value;

    return html`
      <div class="dataset-section">
        <h3 class="subsection-title">MusicBrainz Datasets</h3>
        <div class="button-group" style="margin-bottom: 0.75rem;">
          <button
            class="secondary-btn"
            onclick="this.closest('imdb-settings').handleRefreshMbDatasets()"
            ${loading ? 'disabled' : ''}
          >
            ${loading ? 'Loading...' : 'Refresh Sizes'}
          </button>
          ${
            datasets.length > 0
              ? html`
            <button
              class="secondary-btn"
              onclick="this.closest('imdb-settings').handleDownloadAllMb()"
            >
              Download All
            </button>
          `
              : ''
          }
        </div>
        ${
          datasets.length > 0
            ? html`
          <table class="dataset-table">
            <thead>
              <tr>
                <th>Dataset</th>
                <th>Remote Size</th>
                <th>Cached</th>
                <th>Cache Age</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              ${datasets
                .map(
                  (ds) => html`
                <tr>
                  <td class="dataset-name-cell">${escapeHtml(ds.name)}</td>
                  <td>${this.formatSize(ds.remoteSize)}</td>
                  <td>
                    <span class="cache-badge ${ds.cached ? 'cache-yes' : 'cache-no'}">
                      ${ds.cached ? 'Yes' : 'No'}
                    </span>
                  </td>
                  <td>${ds.localAge ?? '--'}</td>
                  <td class="dataset-actions">
                    <button
                      class="small-btn"
                      onclick="this.closest('imdb-settings').handleDownloadMbDataset('${escapeHtml(ds.name)}')"
                    >Download</button>
                    <button
                      class="small-btn"
                      onclick="this.closest('imdb-settings').handleProcessMbDataset('${escapeHtml(ds.name)}')"
                      ${ds.cached ? '' : 'disabled'}
                    >Process</button>
                  </td>
                </tr>
              `,
                )
                .join('')}
            </tbody>
          </table>
        `
            : ''
        }
      </div>
    `;
  }
}
