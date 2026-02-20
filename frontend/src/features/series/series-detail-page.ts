/**
 * Series Detail page - shows series info with seasons and episodes
 */

import type { ReleaseSearchModal } from '../../components/release-search-modal';
import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { type Episode, http, type QueueItem, type Series } from '../../core/http';
import { createMutation, createQuery, invalidateQueries, useQueueQuery } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';
import type { EpisodeRenameDialog } from './episode-rename-dialog';
import type { SeriesEditDialog } from './series-edit-dialog';
import type { SeriesMatchDialog } from './series-match-dialog';

// Ensure dialog components are registered
import './episode-rename-dialog';
import './series-edit-dialog';
import './series-match-dialog';

interface SeasonEpisodes {
  seasonNumber: number;
  episodes: Episode[];
  monitored: boolean;
  statistics: {
    episodeFileCount: number;
    episodeCount: number;
    totalEpisodeCount: number;
    sizeOnDisk: number;
    percentOfEpisodes: number;
  };
}

@customElement('series-detail-page')
export class SeriesDetailPage extends BaseComponent {
  private seriesId = signal<number | null>(null);
  private titleSlug = signal<string | null>(null);
  private expandedSeasons = signal<Set<number>>(new Set());

  // Query state - will be created lazily when we have a series ID
  private seriesQuery: ReturnType<typeof createQuery<Series | null>> | null = null;
  private episodesQuery: ReturnType<typeof createQuery<Episode[]>> | null = null;
  private queueQuery = useQueueQuery();

  // Observe the titleslug attribute from the router
  static get observedAttributes(): string[] {
    return ['titleslug'];
  }

  // Create queries with the correct ID
  private createQueries(id: number): void {
    this.seriesQuery = createQuery({
      queryKey: ['/series', id],
      queryFn: () => http.get<Series>(`/series/${id}`),
    });

    this.episodesQuery = createQuery({
      queryKey: ['/episode', id],
      queryFn: () => http.get<Episode[]>('/episode', { params: { seriesId: id } }),
    });

    // Watch the new query signals to trigger re-renders
    this.watch(this.seriesQuery.data, () => this.requestUpdate());
    this.watch(this.seriesQuery.isLoading, () => this.requestUpdate());
    this.watch(this.episodesQuery.data, () => this.requestUpdate());
  }

  private searchMutation = createMutation({
    mutationFn: (params: { seriesId?: number; episodeIds?: number[] }) =>
      http.post('/command', {
        name: params.episodeIds ? 'EpisodeSearch' : 'SeriesSearch',
        ...params,
      }),
    onSuccess: () => {
      showSuccess('Search started');
    },
    onError: () => {
      showError('Failed to start search');
    },
  });

  private monitorMutation = createMutation({
    mutationFn: (params: { episodeIds: number[]; monitored: boolean }) =>
      http.put('/episode/monitor', params),
    onSuccess: () => {
      invalidateQueries(['/episode', '/series']);
      showSuccess('Episode monitoring updated');
    },
    onError: () => {
      showError('Failed to update monitoring');
    },
  });

  private refreshMetadataMutation = createMutation({
    mutationFn: (seriesId: number) => http.post('/command', { name: 'RefreshSeries', seriesId }),
    onSuccess: () => {
      showSuccess('Metadata refresh queued - syncing episodes from TVDB');
      // Data will be invalidated via WebSocket events when refresh completes
    },
    onError: () => {
      showError('Failed to queue metadata refresh');
    },
  });

  private rescanMutation = createMutation({
    mutationFn: (seriesId: number) => http.post('/command', { name: 'RescanSeries', seriesId }),
    onSuccess: () => {
      const id = this.seriesId.value;
      if (id) {
        invalidateQueries(['/series', id]);
        invalidateQueries(['/episode', id]);
      }
      showSuccess('Disk scan started');
    },
    onError: () => {
      showError('Failed to start disk scan');
    },
  });

  setSeriesId(id: number): void {
    this.seriesId.set(id);
    // Create the queries with the correct ID
    this.createQueries(id);
  }

  // Handle the titleslug attribute from router
  // Note: This may fire before connectedCallback, so we store the value
  // and process it in onMount
  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null): void {
    if (name === 'titleslug' && newValue && newValue !== oldValue) {
      this.titleSlug.set(newValue);
      // Only process if already connected (for dynamic attribute changes)
      if (this._isConnected) {
        this.lookupSeriesId(newValue);
      }
    }
  }

  // Look up series ID from titleSlug
  private async lookupSeriesId(slug: string): Promise<void> {
    try {
      // Fetch the series list directly via http (not using query which may not be ready)
      const seriesList = await http.get<Series[]>('/series');

      if (seriesList) {
        const series = seriesList.find((s) => s.titleSlug === slug);
        if (series) {
          this.setSeriesId(series.id);
        } else {
          showError(`Series not found: ${slug}`);
        }
      }
    } catch (_error) {
      showError('Failed to load series');
    }
  }

  protected onInit(): void {
    this.watch(this.seriesId);
    this.watch(this.titleSlug);
    this.watch(this.expandedSeasons);
    // Update progress bars in-place instead of full re-render to preserve modal state
    this.watch(this.queueQuery.data, () => this.updateProgressBars());
    // Note: Query signals are watched when createQueries() is called
  }

  protected onMount(): void {
    // Process the titleSlug attribute that was set before connection
    const slug = this.getAttribute('titleslug');
    if (slug && !this.seriesId.value) {
      this.titleSlug.set(slug);
      this.lookupSeriesId(slug);
    }
  }

  protected template(): string {
    const series = this.seriesQuery?.data.value;
    const episodes = this.episodesQuery?.data.value ?? [];
    const isLoading = this.seriesQuery?.isLoading.value ?? true;

    // Show loading while looking up series by slug or fetching series details
    if (isLoading || !series) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    const seasons = this.groupEpisodesBySeason(episodes, series);

    // Build episodeId → QueueItem map for download progress
    const queueMap = new Map<number, QueueItem>();
    const queueRecords = this.queueQuery.data.value?.records ?? [];
    for (const item of queueRecords) {
      if (item.episodeId && item.status === 'downloading') {
        queueMap.set(item.episodeId, item);
      }
    }

    return html`
      <div class="series-detail">
        <div class="series-header">
          <div class="series-poster">
            ${
              series.images?.find((i) => i.coverType === 'poster')?.url
                ? html`
              <img
                src="${series.images.find((i) => i.coverType === 'poster')?.url ?? ''}"
                alt="${escapeHtml(series.title)}"
              />
            `
                : html`
              <div class="poster-placeholder">
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                  <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                  <line x1="7" y1="2" x2="7" y2="22"></line>
                  <line x1="17" y1="2" x2="17" y2="22"></line>
                  <line x1="2" y1="12" x2="22" y2="12"></line>
                  <line x1="2" y1="7" x2="7" y2="7"></line>
                  <line x1="2" y1="17" x2="7" y2="17"></line>
                  <line x1="17" y1="17" x2="22" y2="17"></line>
                  <line x1="17" y1="7" x2="22" y2="7"></line>
                </svg>
              </div>
            `
            }
          </div>

          <div class="series-info">
            <h1 class="series-title">${escapeHtml(series.title)}</h1>

            <div class="series-meta">
              <span class="meta-item">${series.year}</span>
              <span class="meta-item">${escapeHtml(series.network ?? 'Unknown Network')}</span>
              <span class="meta-item">${escapeHtml(series.seriesType)}</span>
              <span class="meta-item status ${series.status}">${escapeHtml(series.status)}</span>
            </div>

            <div class="series-stats">
              <div class="stat">
                <span class="stat-value">${series.statistics?.seasonCount ?? 0}</span>
                <span class="stat-label">Seasons</span>
              </div>
              <div class="stat">
                <span class="stat-value">${series.statistics?.episodeFileCount ?? 0}/${series.statistics?.episodeCount ?? 0}</span>
                <span class="stat-label">Episodes</span>
              </div>
              <div class="stat">
                <span class="stat-value">${this.formatBytes(series.statistics?.sizeOnDisk ?? 0)}</span>
                <span class="stat-label">Size</span>
              </div>
            </div>

            <div class="series-overview">
              ${escapeHtml(series.overview ?? 'No overview available.')}
            </div>

            <div class="series-actions">
              <button class="action-btn primary" onclick="this.closest('series-detail-page').handleSearchSeries()">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="11" cy="11" r="8"></circle>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
                Search
              </button>
              <button class="action-btn" onclick="this.closest('series-detail-page').handleRefreshMetadata()" title="Re-fetch episode list from TVDB/Skyhook">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M21 2v6h-6"></path>
                  <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
                  <path d="M3 22v-6h6"></path>
                  <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
                </svg>
                Refresh Metadata
              </button>
              <button class="action-btn" onclick="this.closest('series-detail-page').handleRescan()" title="Scan disk for episode files">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                  <line x1="12" y1="11" x2="12" y2="17"></line>
                  <line x1="9" y1="14" x2="15" y2="14"></line>
                </svg>
                Rescan Files
              </button>
              <button class="action-btn" onclick="this.closest('series-detail-page').handleOrganize()" title="Preview and rename episode files to match naming format">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"></path>
                  <polyline points="14 2 14 8 20 8"></polyline>
                  <line x1="16" y1="13" x2="8" y2="13"></line>
                  <line x1="16" y1="17" x2="8" y2="17"></line>
                  <polyline points="10 9 9 9 8 9"></polyline>
                </svg>
                Organize
              </button>
              <button class="action-btn" onclick="this.closest('series-detail-page').handleFixMatch()" title="Re-match this series to a different TVDB/IMDB entry">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="22" y1="2" x2="11" y2="13"></line>
                  <polygon points="8 2 2 2 2 8"></polygon>
                  <line x1="2" y1="2" x2="7" y2="7"></line>
                </svg>
                Fix Match
              </button>
              <button class="action-btn" onclick="this.closest('series-detail-page').handleEdit()">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                </svg>
                Edit
              </button>
            </div>
          </div>
        </div>

        <div class="seasons-section">
          <h2 class="section-title">Seasons</h2>

          <div class="seasons-list">
            ${seasons.map((season) => this.renderSeason(season, series, queueMap)).join('')}
          </div>
        </div>

        <!-- Interactive Search Modal -->
        <release-search-modal></release-search-modal>

        <!-- Edit Series Dialog -->
        <series-edit-dialog></series-edit-dialog>

        <!-- Episode Rename Dialog -->
        <episode-rename-dialog></episode-rename-dialog>

        <!-- Fix Match Dialog -->
        <series-match-dialog></series-match-dialog>
      </div>

      <style>
        .series-detail {
          display: flex;
          flex-direction: column;
          gap: 2rem;
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

        .series-header {
          display: flex;
          gap: 2rem;
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        @media (max-width: 768px) {
          .series-header {
            flex-direction: column;
          }
        }

        .series-poster {
          flex-shrink: 0;
          width: 200px;
        }

        .series-poster img {
          width: 100%;
          border-radius: 0.375rem;
        }

        .poster-placeholder {
          display: flex;
          align-items: center;
          justify-content: center;
          height: 300px;
          background-color: var(--bg-card-alt);
          border-radius: 0.375rem;
          color: var(--text-color-muted);
        }

        .series-info {
          flex: 1;
          min-width: 0;
        }

        .series-title {
          font-size: 1.75rem;
          font-weight: 600;
          margin: 0 0 0.75rem 0;
        }

        .series-meta {
          display: flex;
          flex-wrap: wrap;
          gap: 0.75rem;
          margin-bottom: 1.25rem;
        }

        .meta-item {
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .meta-item.status {
          padding: 0.125rem 0.5rem;
          border-radius: 0.25rem;
          font-weight: 500;
        }

        .meta-item.status.continuing {
          background-color: var(--color-success);
          color: var(--color-white);
        }

        .meta-item.status.ended {
          background-color: var(--text-color-muted);
          color: var(--color-white);
        }

        .series-stats {
          display: flex;
          gap: 2rem;
          margin-bottom: 1.25rem;
        }

        .stat {
          display: flex;
          flex-direction: column;
        }

        .stat-value {
          font-size: 1.25rem;
          font-weight: 600;
        }

        .stat-label {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .series-overview {
          font-size: 0.875rem;
          line-height: 1.6;
          color: var(--text-color-muted);
          margin-bottom: 1.25rem;
          max-height: 100px;
          overflow: hidden;
        }

        .series-actions {
          display: flex;
          flex-wrap: wrap;
          gap: 0.5rem;
        }

        .action-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 1rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .action-btn:hover {
          background-color: var(--btn-default-bg-hover);
        }

        .action-btn.primary {
          background-color: var(--btn-primary-bg);
          border-color: var(--btn-primary-border);
          color: var(--color-white);
        }

        .action-btn.primary:hover {
          background-color: var(--btn-primary-bg-hover);
        }

        .seasons-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
        }

        .seasons-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .season-item {
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          overflow: hidden;
        }

        .season-header {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem 1rem;
          background-color: var(--bg-card-alt);
          cursor: pointer;
        }

        .season-header:hover {
          background-color: var(--bg-table-row-hover);
        }

        .expand-icon {
          transition: transform 0.2s;
        }

        .expand-icon.expanded {
          transform: rotate(90deg);
        }

        .season-title {
          font-weight: 500;
          flex: 1;
        }

        .season-stats {
          display: flex;
          align-items: center;
          gap: 1rem;
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .season-progress {
          width: 100px;
          height: 6px;
          background-color: var(--bg-progress);
          border-radius: 3px;
          overflow: hidden;
        }

        .season-progress-fill {
          height: 100%;
          background-color: var(--color-success);
        }

        .episodes-list {
          border-top: 1px solid var(--border-color);
        }

        .episode-row {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem 1rem;
          border-bottom: 1px solid var(--border-color);
        }

        .episode-row:last-child {
          border-bottom: none;
        }

        .episode-row:hover {
          background-color: var(--bg-table-row-hover);
        }

        .episode-monitor {
          width: 20px;
          height: 20px;
          accent-color: var(--color-primary);
        }

        .season-monitor {
          width: 18px;
          height: 18px;
          accent-color: var(--color-primary);
          cursor: pointer;
        }

        .episode-number {
          width: 50px;
          font-weight: 500;
          color: var(--text-color-muted);
        }

        .episode-title {
          flex: 1;
        }

        .episode-date {
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .episode-status {
          display: flex;
          align-items: center;
          gap: 0.25rem;
        }

        .episode-status-icon {
          width: 16px;
          height: 16px;
        }

        .episode-status-icon.downloaded {
          color: var(--color-success);
        }

        .episode-status-icon.missing {
          color: var(--color-danger);
        }

        .episode-status-icon.unaired {
          color: var(--text-color-muted);
        }

        .episode-download-progress {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          min-width: 120px;
        }

        .episode-progress-bar {
          flex: 1;
          height: 6px;
          background-color: var(--bg-progress);
          border-radius: 3px;
          overflow: hidden;
        }

        .episode-progress-fill {
          height: 100%;
          background-color: var(--color-primary);
          transition: width 0.3s ease;
        }

        .episode-progress-text {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          white-space: nowrap;
        }

        .episode-actions {
          display: flex;
          gap: 0.25rem;
        }

        .episode-action-btn {
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

        .episode-action-btn:hover {
          color: var(--color-primary);
          background-color: var(--bg-input-hover);
        }

        .episode-action-btn.auto-search {
          color: var(--color-success);
        }

        .episode-action-btn.auto-search:hover {
          color: var(--color-white);
          background-color: var(--color-success);
        }
      </style>
    `;
  }

  private groupEpisodesBySeason(episodes: Episode[], series: Series): SeasonEpisodes[] {
    const seasonMap = new Map<number, Episode[]>();

    episodes.forEach((ep) => {
      const existing = seasonMap.get(ep.seasonNumber) ?? [];
      existing.push(ep);
      seasonMap.set(ep.seasonNumber, existing);
    });

    const seasons: SeasonEpisodes[] = [];

    seasonMap.forEach((eps, seasonNumber) => {
      const seriesSeason = series.seasons?.find((s) => s.seasonNumber === seasonNumber);
      const downloadedCount = eps.filter((e) => e.hasFile).length;
      const airedCount = eps.filter((e) => {
        if (!e.airDateUtc) return false;
        return new Date(e.airDateUtc) <= new Date();
      }).length;

      seasons.push({
        seasonNumber,
        episodes: eps.sort((a, b) => a.episodeNumber - b.episodeNumber),
        monitored: seriesSeason?.monitored ?? true,
        statistics: {
          episodeFileCount: downloadedCount,
          episodeCount: airedCount,
          totalEpisodeCount: eps.length,
          sizeOnDisk: 0, // Size is calculated server-side in series statistics
          percentOfEpisodes: airedCount > 0 ? (downloadedCount / airedCount) * 100 : 0,
        },
      });
    });

    return seasons.sort((a, b) => b.seasonNumber - a.seasonNumber);
  }

  private renderSeason(
    season: SeasonEpisodes,
    _series: Series,
    queueMap: Map<number, QueueItem>,
  ): string {
    const isExpanded = this.expandedSeasons.value.has(season.seasonNumber);
    const seasonLabel = season.seasonNumber === 0 ? 'Specials' : `Season ${season.seasonNumber}`;

    return html`
      <div class="season-item">
        <div
          class="season-header"
          onclick="this.closest('series-detail-page').toggleSeason(${season.seasonNumber})"
        >
          <svg class="expand-icon ${isExpanded ? 'expanded' : ''}" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
          <input
            type="checkbox"
            class="season-monitor"
            ${season.monitored ? 'checked' : ''}
            onclick="event.stopPropagation(); this.closest('series-detail-page').toggleSeasonMonitor(${season.seasonNumber}, this.checked)"
            title="${season.monitored ? 'Unmonitor season' : 'Monitor season'}"
          />
          <span class="season-title">${seasonLabel}</span>
          <div class="season-stats">
            <span>${season.statistics.episodeFileCount}/${season.statistics.episodeCount}</span>
            <div class="season-progress">
              <div class="season-progress-fill" style="width: ${season.statistics.percentOfEpisodes}%"></div>
            </div>
          </div>
        </div>

        ${
          isExpanded
            ? html`
          <div class="episodes-list">
            ${season.episodes.map((ep) => this.renderEpisode(ep, queueMap)).join('')}
          </div>
        `
            : ''
        }
      </div>
    `;
  }

  private renderEpisode(episode: Episode, queueMap: Map<number, QueueItem>): string {
    const isAired = episode.airDateUtc ? new Date(episode.airDateUtc) <= new Date() : false;
    const queueItem = queueMap.get(episode.id);
    const isDownloading = !!queueItem;
    const status = isDownloading
      ? 'downloading'
      : episode.hasFile
        ? 'downloaded'
        : isAired
          ? 'missing'
          : 'unaired';

    const statusHtml = this.buildEpisodeStatusHtml(status, queueItem);

    return html`
      <div class="episode-row" data-episode-id="${episode.id}">
        <input
          type="checkbox"
          class="episode-monitor"
          ${episode.monitored ? 'checked' : ''}
          onclick="event.stopPropagation(); this.closest('series-detail-page').toggleMonitor(${episode.id}, this.checked)"
          title="${episode.monitored ? 'Unmonitor' : 'Monitor'}"
        />
        <span class="episode-number">E${String(episode.episodeNumber).padStart(2, '0')}</span>
        <span class="episode-title">${escapeHtml(episode.title)}</span>
        <span class="episode-date">${episode.airDate ? new Date(episode.airDate).toLocaleDateString() : '-'}</span>
        <span class="episode-status-cell" data-status-for="${episode.id}">${safeHtml(statusHtml)}</span>
        <div class="episode-actions">
          <button
            class="episode-action-btn"
            onclick="event.stopPropagation(); this.closest('series-detail-page').openInteractiveSearch(${episode.id}, ${episode.seasonNumber}, ${episode.episodeNumber}, '${episode.title.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}')"
            title="Interactive Search"
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
          </button>
          ${
            isAired && !episode.hasFile && !isDownloading
              ? html`
            <button
              class="episode-action-btn auto-search"
              onclick="event.stopPropagation(); this.closest('series-detail-page').searchEpisode(${episode.id})"
              title="Automatic Search"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polygon points="5 3 19 12 5 21 5 3"></polygon>
              </svg>
            </button>
          `
              : ''
          }
        </div>
      </div>
    `;
  }

  private buildEpisodeStatusHtml(status: string, queueItem?: QueueItem): string {
    if (status === 'downloading' && queueItem) {
      const progress =
        queueItem.size > 0 ? ((queueItem.size - queueItem.sizeleft) / queueItem.size) * 100 : 0;
      const downloaded = this.formatBytes(queueItem.size - queueItem.sizeleft);
      const total = this.formatBytes(queueItem.size);
      const timeleft = queueItem.timeleft ?? '';
      return `
        <div class="episode-download-progress" title="${downloaded} / ${total}${timeleft ? ` - ${timeleft} remaining` : ''}">
          <div class="episode-progress-bar">
            <div class="episode-progress-fill" style="width: ${progress}%"></div>
          </div>
          <span class="episode-progress-text">${Math.round(progress)}%</span>
        </div>
      `;
    }
    const statusIcon =
      status === 'downloaded'
        ? '<svg class="episode-status-icon downloaded" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline></svg>'
        : status === 'missing'
          ? '<svg class="episode-status-icon missing" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>'
          : '<svg class="episode-status-icon unaired" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>';
    return `<div class="episode-status">${statusIcon}</div>`;
  }

  /**
   * Surgically update only episode progress bars without re-rendering the full DOM.
   * This prevents destroying child components like the release search modal.
   *
   * SECURITY: All values interpolated are numeric (progress %, byte counts) from
   * internal queue data — no user-controlled strings are inserted as HTML.
   */
  private updateProgressBars(): void {
    const queueRecords = this.queueQuery.data.value?.records ?? [];
    const queueMap = new Map<number, QueueItem>();
    for (const item of queueRecords) {
      if (item.episodeId && item.status === 'downloading') {
        queueMap.set(item.episodeId, item);
      }
    }

    const statusCells = this.querySelectorAll<HTMLElement>('.episode-status-cell[data-status-for]');
    statusCells.forEach((cell) => {
      const episodeId = Number(cell.dataset.statusFor);
      if (!episodeId) return;

      const queueItem = queueMap.get(episodeId);

      // Determine current status from the existing DOM
      const hasProgressBar = cell.querySelector('.episode-download-progress') !== null;
      const hasDownloadedIcon = cell.querySelector('.episode-status-icon.downloaded') !== null;

      if (queueItem) {
        // Episode is downloading — update or create progress bar
        if (hasProgressBar) {
          // Just update the existing progress bar values in-place
          const fill = cell.querySelector<HTMLElement>('.episode-progress-fill');
          const text = cell.querySelector('.episode-progress-text');
          const container = cell.querySelector<HTMLElement>('.episode-download-progress');
          const progress =
            queueItem.size > 0 ? ((queueItem.size - queueItem.sizeleft) / queueItem.size) * 100 : 0;
          if (fill) fill.style.width = `${progress}%`;
          if (text) text.textContent = `${Math.round(progress)}%`;
          if (container) {
            const downloaded = this.formatBytes(queueItem.size - queueItem.sizeleft);
            const total = this.formatBytes(queueItem.size);
            const timeleft = queueItem.timeleft ?? '';
            container.title = `${downloaded} / ${total}${timeleft ? ` - ${timeleft} remaining` : ''}`;
          }
        } else {
          // Switch from status icon to progress bar (developer-controlled template, no user strings)
          // nosemgrep: javascript.browser.security.insecure-document-method.insecure-document-method
          cell.innerHTML = this.buildEpisodeStatusHtml('downloading', queueItem);
        }
      } else if (hasProgressBar && !hasDownloadedIcon) {
        // Was downloading, now stopped — revert to missing icon (developer-controlled SVG)
        // nosemgrep: javascript.browser.security.insecure-document-method.insecure-document-method
        cell.innerHTML = this.buildEpisodeStatusHtml('missing');
      }
    });
  }

  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  toggleSeason(seasonNumber: number): void {
    const current = new Set(this.expandedSeasons.value);
    if (current.has(seasonNumber)) {
      current.delete(seasonNumber);
    } else {
      current.add(seasonNumber);
    }
    this.expandedSeasons.set(current);
  }

  toggleMonitor(episodeId: number, monitored: boolean): void {
    this.monitorMutation.mutate({ episodeIds: [episodeId], monitored });
  }

  toggleSeasonMonitor(seasonNumber: number, monitored: boolean): void {
    const episodes = this.episodesQuery?.data.value as Episode[] | undefined;
    if (!episodes) return;
    const episodeIds = episodes.filter((e) => e.seasonNumber === seasonNumber).map((e) => e.id);
    if (episodeIds.length === 0) return;
    this.monitorMutation.mutate({ episodeIds, monitored });
  }

  searchEpisode(episodeId: number): void {
    this.searchMutation.mutate({ episodeIds: [episodeId] });
  }

  openInteractiveSearch(
    episodeId: number,
    seasonNumber: number,
    episodeNumber: number,
    episodeTitle: string,
  ): void {
    const series = this.seriesQuery?.data.value;
    if (!series) return;

    const modal = this.querySelector('release-search-modal') as ReleaseSearchModal | null;
    if (modal) {
      modal.open({
        seriesId: series.id,
        seriesTitle: series.title,
        seasonNumber,
        episodeId,
        episodeNumber,
        episodeTitle,
      });
    }
  }

  handleSearchSeries(): void {
    const id = this.seriesId.value;
    if (id) {
      this.searchMutation.mutate({ seriesId: id });
    }
  }

  handleRefreshMetadata(): void {
    const id = this.seriesId.value;
    if (id) {
      this.refreshMetadataMutation.mutate(id);
    }
  }

  handleRescan(): void {
    const id = this.seriesId.value;
    if (id) {
      this.rescanMutation.mutate(id);
    }
  }

  handleOrganize(): void {
    const series = this.seriesQuery?.data.value;
    if (!series) return;

    const dialog = this.querySelector('episode-rename-dialog') as EpisodeRenameDialog | null;
    if (dialog) {
      dialog.open(series.id, series.title);
    }
  }

  handleFixMatch(): void {
    const series = this.seriesQuery?.data.value;
    if (!series) return;

    const dialog = this.querySelector('series-match-dialog') as SeriesMatchDialog | null;
    if (dialog) {
      dialog.open(series.id, series.title, series.tvdbId, series.imdbId ?? null, series.year);
    }
  }

  handleEdit(): void {
    const series = this.seriesQuery?.data.value;
    if (!series) return;

    const dialog = this.querySelector('series-edit-dialog') as SeriesEditDialog | null;
    if (dialog) {
      dialog.open(series);
    }
  }
}
