/**
 * Unified Wanted page with content-type tabs:
 * Series | Movies | Anime | Music (stub) | Podcasts (stub)
 *
 * Series and Anime tabs have a sub-toggle for Missing vs Cutoff Unmet.
 * Movies tab shows missing movies only.
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

// --- Types ---

interface MissingEpisode {
  id: number;
  seriesId: number;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  airDate: string;
  airDateUtc: string;
  monitored: boolean;
  series: {
    id: number;
    title: string;
    titleSlug: string;
    seriesType: number;
  };
}

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
    seriesType: number;
  };
}

interface EpisodeResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: MissingEpisode[];
}

interface CutoffResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: CutoffEpisode[];
}

interface MissingMovie {
  id: number;
  title: string;
  sortTitle: string;
  tmdbId: number;
  imdbId: string | null;
  year: number;
  monitored: boolean;
  hasFile: boolean;
  titleSlug: string;
  path: string;
  status: number;
  added: string;
}

interface MovieResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: MissingMovie[];
}

type ContentTab = 'series' | 'movies' | 'anime' | 'music' | 'podcasts';
type SubTab = 'missing' | 'cutoff';
type SortKey = 'seriesTitle' | 'episodeNumber' | 'title' | 'airDateUtc';
type MovieSortKey = 'title' | 'year' | 'added';

// Module-level state to survive navigation
let savedContentTab: ContentTab = 'series';
let savedSubTab: SubTab = 'missing';

@customElement('wanted-page')
export class WantedPage extends BaseComponent {
  private contentTab: ContentTab = savedContentTab;
  private subTab: SubTab = savedSubTab;

  private page = signal(1);
  private pageSize = 25;
  private sortKey = signal<SortKey>('airDateUtc');
  private sortDirection = signal<'ascending' | 'descending'>('descending');
  private movieSortKey = signal<MovieSortKey>('title');
  private movieSortDirection = signal<'ascending' | 'descending'>('ascending');

  // Episode queries (missing + cutoff) with content type filter
  private missingQuery = createQuery({
    queryKey: [
      '/wanted/missing',
      this.page.value,
      this.pageSize,
      this.sortKey.value,
      this.sortDirection.value,
      this.contentTab,
    ],
    queryFn: () =>
      http.get<EpisodeResponse>('/wanted/missing', {
        params: {
          page: this.page.value,
          pageSize: this.pageSize,
          monitored: true,
          sortKey: this.sortKey.value,
          sortDirection: this.sortDirection.value,
          contentType: this.contentTab === 'series' || this.contentTab === 'anime'
            ? this.contentTab
            : undefined,
        },
      }),
  });

  private cutoffQuery = createQuery({
    queryKey: [
      '/wanted/cutoff',
      this.page.value,
      this.pageSize,
      this.sortKey.value,
      this.sortDirection.value,
      this.contentTab,
    ],
    queryFn: () =>
      http.get<CutoffResponse>('/wanted/cutoff', {
        params: {
          page: this.page.value,
          pageSize: this.pageSize,
          monitored: true,
          sortKey: this.sortKey.value,
          sortDirection: this.sortDirection.value,
          contentType: this.contentTab === 'series' || this.contentTab === 'anime'
            ? this.contentTab
            : undefined,
        },
      }),
  });

  // Movie missing query
  private movieQuery = createQuery({
    queryKey: [
      '/wanted/missing/movies',
      this.page.value,
      this.pageSize,
      this.movieSortKey.value,
      this.movieSortDirection.value,
    ],
    queryFn: () =>
      http.get<MovieResponse>('/wanted/missing/movies', {
        params: {
          page: this.page.value,
          pageSize: this.pageSize,
          monitored: true,
          sortKey: this.movieSortKey.value,
          sortDirection: this.movieSortDirection.value,
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
    this.watch(this.movieSortKey);
    this.watch(this.movieSortDirection);
    this.watch(this.missingQuery.data);
    this.watch(this.missingQuery.isLoading);
    this.watch(this.missingQuery.isError);
    this.watch(this.cutoffQuery.data);
    this.watch(this.cutoffQuery.isLoading);
    this.watch(this.cutoffQuery.isError);
    this.watch(this.movieQuery.data);
    this.watch(this.movieQuery.isLoading);
    this.watch(this.movieQuery.isError);
  }

  protected template(): string {
    return html`
      <div class="wanted-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Wanted</h1>
            ${safeHtml(this.renderTotalCount())}
          </div>
          <div class="toolbar-right">
            ${safeHtml(this.renderToolbarActions())}
          </div>
        </div>

        <!-- Content type tabs -->
        <div class="content-tabs">
          ${safeHtml(this.renderContentTab('series', 'Series', this.getSeriesCount()))}
          ${safeHtml(this.renderContentTab('movies', 'Movies', this.getMovieCount()))}
          ${safeHtml(this.renderContentTab('anime', 'Anime', this.getAnimeCount()))}
          ${safeHtml(this.renderContentTab('music', 'Music', -1))}
          ${safeHtml(this.renderContentTab('podcasts', 'Podcasts', -1))}
        </div>

        <!-- Sub-tabs for episode-based content -->
        ${safeHtml(this.renderSubTabs())}

        <!-- Tab content -->
        <div class="tab-content">
          ${safeHtml(this.renderTabContent())}
        </div>
      </div>

      <style>${this.getStyles()}</style>
    `;
  }

  // --- Count helpers ---

  private getSeriesCount(): number {
    if (this.contentTab !== 'series') return -1;
    if (this.subTab === 'missing') {
      return this.missingQuery.data.value?.totalRecords ?? 0;
    }
    return this.cutoffQuery.data.value?.totalRecords ?? 0;
  }

  private getMovieCount(): number {
    if (this.contentTab !== 'movies') return -1;
    return this.movieQuery.data.value?.totalRecords ?? 0;
  }

  private getAnimeCount(): number {
    if (this.contentTab !== 'anime') return -1;
    if (this.subTab === 'missing') {
      return this.missingQuery.data.value?.totalRecords ?? 0;
    }
    return this.cutoffQuery.data.value?.totalRecords ?? 0;
  }

  private renderTotalCount(): string {
    let count = 0;
    if (this.contentTab === 'movies') {
      count = this.movieQuery.data.value?.totalRecords ?? 0;
    } else if (this.contentTab === 'series' || this.contentTab === 'anime') {
      count = this.subTab === 'missing'
        ? (this.missingQuery.data.value?.totalRecords ?? 0)
        : (this.cutoffQuery.data.value?.totalRecords ?? 0);
    }
    if (this.contentTab === 'music' || this.contentTab === 'podcasts') return '';
    const label = this.contentTab === 'movies' ? 'movies' : 'episodes';
    return `<span class="item-count">${count} ${label}</span>`;
  }

  // --- Content tabs ---

  private renderContentTab(tab: ContentTab, label: string, count: number): string {
    const active = this.contentTab === tab;
    const countBadge = count >= 0
      ? `<span class="tab-count">${count}</span>`
      : '<span class="tab-count tab-count-stub">--</span>';
    return `<button class="content-tab ${active ? 'active' : ''}"
      onclick="this.closest('wanted-page').handleContentTabClick('${tab}')">${label}${countBadge}</button>`;
  }

  private renderSubTabs(): string {
    if (this.contentTab !== 'series' && this.contentTab !== 'anime') return '';
    return html`
      <div class="sub-tabs">
        <button class="sub-tab ${this.subTab === 'missing' ? 'active' : ''}"
          onclick="this.closest('wanted-page').handleSubTabClick('missing')">Missing</button>
        <button class="sub-tab ${this.subTab === 'cutoff' ? 'active' : ''}"
          onclick="this.closest('wanted-page').handleSubTabClick('cutoff')">Cutoff Unmet</button>
      </div>
    `;
  }

  // --- Toolbar actions ---

  private renderToolbarActions(): string {
    if (this.contentTab === 'music' || this.contentTab === 'podcasts') return '';

    const hasEpisodes = this.contentTab !== 'movies' && (
      this.subTab === 'missing'
        ? (this.missingQuery.data.value?.records?.length ?? 0) > 0
        : (this.cutoffQuery.data.value?.records?.length ?? 0) > 0
    );

    const searchAllBtn = hasEpisodes ? html`
      <button class="search-all-btn"
        onclick="this.closest('wanted-page').handleSearchAll()">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8"></circle>
          <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
        </svg>
        Search All
      </button>
    ` : '';

    return html`
      ${searchAllBtn}
      <button class="refresh-btn"
        onclick="this.closest('wanted-page').handleRefresh()" title="Refresh">
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="23 4 23 10 17 10"></polyline>
          <polyline points="1 20 1 14 7 14"></polyline>
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
        </svg>
      </button>
    `;
  }

  // --- Tab content ---

  private renderTabContent(): string {
    if (this.contentTab === 'music') return this.renderStub('Music');
    if (this.contentTab === 'podcasts') return this.renderStub('Podcasts');
    if (this.contentTab === 'movies') return this.renderMoviesContent();

    // Series or Anime — episode-based
    if (this.subTab === 'missing') return this.renderMissingContent();
    return this.renderCutoffContent();
  }

  private renderStub(label: string): string {
    return html`
      <div class="empty-container">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor"
          stroke-width="1.5" color="var(--text-color-muted)">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="12" y1="8" x2="12" y2="12"></line>
          <line x1="12" y1="16" x2="12.01" y2="16"></line>
        </svg>
        <p>${label} wanted tracking coming soon</p>
      </div>
    `;
  }

  // --- Missing episodes ---

  private renderMissingContent(): string {
    const isLoading = this.missingQuery.isLoading.value;
    const isError = this.missingQuery.isError.value;
    if (isLoading) return this.renderLoading();
    if (isError) return this.renderError('missing episodes');

    const response = this.missingQuery.data.value;
    const episodes = response?.records ?? [];
    const totalPages = Math.ceil((response?.totalRecords ?? 0) / this.pageSize);

    if (episodes.length === 0) return this.renderEmpty('No missing episodes');

    return html`
      ${this.renderEpisodeTable(episodes, 'missing')}
      ${totalPages > 1 ? this.renderPagination(this.page.value, totalPages) : ''}
    `;
  }

  // --- Cutoff unmet ---

  private renderCutoffContent(): string {
    const isLoading = this.cutoffQuery.isLoading.value;
    const isError = this.cutoffQuery.isError.value;
    if (isLoading) return this.renderLoading();
    if (isError) return this.renderError('cutoff unmet episodes');

    const response = this.cutoffQuery.data.value;
    const episodes = (response?.records ?? []) as unknown as CutoffEpisode[];
    const totalPages = Math.ceil((response?.totalRecords ?? 0) / this.pageSize);

    if (episodes.length === 0) return this.renderEmpty('No episodes below cutoff');

    return html`
      ${this.renderCutoffTable(episodes)}
      ${totalPages > 1 ? this.renderPagination(this.page.value, totalPages) : ''}
    `;
  }

  // --- Movies missing ---

  private renderMoviesContent(): string {
    const isLoading = this.movieQuery.isLoading.value;
    const isError = this.movieQuery.isError.value;
    if (isLoading) return this.renderLoading();
    if (isError) return this.renderError('missing movies');

    const response = this.movieQuery.data.value;
    const movies = response?.records ?? [];
    const totalPages = Math.ceil((response?.totalRecords ?? 0) / this.pageSize);

    if (movies.length === 0) return this.renderEmpty('No missing movies');

    return html`
      ${this.renderMovieTable(movies)}
      ${totalPages > 1 ? this.renderPagination(this.page.value, totalPages) : ''}
    `;
  }

  // --- Tables ---

  private renderEpisodeTable(episodes: MissingEpisode[], _mode: string): string {
    const th = (label: string, key: SortKey): string => {
      const isSorted = this.sortKey.value === key;
      const icon = isSorted
        ? this.sortDirection.value === 'ascending'
          ? '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
          : '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>'
        : '';
      return `<th class="sortable ${isSorted ? 'sorted' : ''}" onclick="this.closest('wanted-page').handleSort('${key}')">${label}${icon}</th>`;
    };

    return html`
      <table class="wanted-table">
        <thead>
          <tr>
            ${th('Series', 'seriesTitle')}
            ${th('Episode', 'episodeNumber')}
            ${th('Title', 'title')}
            ${th('Air Date', 'airDateUtc')}
            <th></th>
          </tr>
        </thead>
        <tbody>
          ${episodes.map((ep) => this.renderEpisodeRow(ep)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderEpisodeRow(episode: MissingEpisode): string {
    const airDate = episode.airDate ? new Date(episode.airDate) : null;
    const seriesPath = this.contentTab === 'anime'
      ? `/anime`
      : `/series/${episode.series.titleSlug}`;

    return html`
      <tr>
        <td>
          <a class="title-link" href="${seriesPath}"
            onclick="event.preventDefault(); this.closest('wanted-page').handleSeriesClick('${episode.series.titleSlug}')">
            ${escapeHtml(episode.series.title)}
          </a>
        </td>
        <td><span class="episode-number">S${String(episode.seasonNumber).padStart(2, '0')}E${String(episode.episodeNumber).padStart(2, '0')}</span></td>
        <td>${escapeHtml(episode.title)}</td>
        <td class="date-cell">${airDate ? airDate.toLocaleDateString() : '-'}</td>
        <td>
          <button class="action-btn" onclick="this.closest('wanted-page').handleSearch(${episode.id})" title="Search for episode">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
          </button>
        </td>
      </tr>
    `;
  }

  private renderCutoffTable(episodes: CutoffEpisode[]): string {
    const th = (label: string, key: SortKey): string => {
      const isSorted = this.sortKey.value === key;
      const icon = isSorted
        ? this.sortDirection.value === 'ascending'
          ? '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
          : '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>'
        : '';
      return `<th class="sortable ${isSorted ? 'sorted' : ''}" onclick="this.closest('wanted-page').handleSort('${key}')">${label}${icon}</th>`;
    };

    return html`
      <table class="wanted-table">
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
          ${episodes.map((ep) => this.renderCutoffRow(ep)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderCutoffRow(episode: CutoffEpisode): string {
    const quality = episode.episodeFile?.quality?.quality?.name ?? 'Unknown';

    return html`
      <tr>
        <td>
          <a class="title-link" href="/series/${episode.series.titleSlug}"
            onclick="event.preventDefault(); this.closest('wanted-page').handleSeriesClick('${episode.series.titleSlug}')">
            ${escapeHtml(episode.series.title)}
          </a>
        </td>
        <td><span class="episode-number">S${String(episode.seasonNumber).padStart(2, '0')}E${String(episode.episodeNumber).padStart(2, '0')}</span></td>
        <td>${escapeHtml(episode.title)}</td>
        <td><span class="quality-badge">${escapeHtml(quality)}</span></td>
        <td>
          <button class="action-btn" onclick="this.closest('wanted-page').handleSearch(${episode.id})" title="Search for episode">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
          </button>
        </td>
      </tr>
    `;
  }

  private renderMovieTable(movies: MissingMovie[]): string {
    const th = (label: string, key: MovieSortKey): string => {
      const isSorted = this.movieSortKey.value === key;
      const icon = isSorted
        ? this.movieSortDirection.value === 'ascending'
          ? '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
          : '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>'
        : '';
      return `<th class="sortable ${isSorted ? 'sorted' : ''}" onclick="this.closest('wanted-page').handleMovieSort('${key}')">${label}${icon}</th>`;
    };

    return html`
      <table class="wanted-table">
        <thead>
          <tr>
            ${th('Title', 'title')}
            ${th('Year', 'year')}
            ${th('Added', 'added')}
          </tr>
        </thead>
        <tbody>
          ${movies.map((m) => this.renderMovieRow(m)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderMovieRow(movie: MissingMovie): string {
    const added = new Date(movie.added);

    return html`
      <tr>
        <td>
          <a class="title-link" href="/movies/${movie.titleSlug}"
            onclick="event.preventDefault(); this.closest('wanted-page').handleMovieClick('${movie.titleSlug}')">
            ${escapeHtml(movie.title)}
          </a>
        </td>
        <td class="date-cell">${movie.year > 0 ? movie.year : '-'}</td>
        <td class="date-cell">${added.toLocaleDateString()}</td>
      </tr>
    `;
  }

  // --- Shared renderers ---

  private renderLoading(): string {
    return html`<div class="loading-container"><div class="loading-spinner"></div></div>`;
  }

  private renderError(what: string): string {
    return html`
      <div class="error-container">
        <p>Failed to load ${what}</p>
        <button class="refresh-btn" onclick="this.closest('wanted-page').handleRefresh()">Retry</button>
      </div>
    `;
  }

  private renderEmpty(message: string): string {
    return html`
      <div class="empty-container">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor"
          stroke-width="1.5" color="var(--text-color-muted)">
          <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
          <polyline points="22 4 12 14.01 9 11.01"></polyline>
        </svg>
        <p>${message}</p>
      </div>
    `;
  }

  private renderPagination(currentPage: number, totalPages: number): string {
    return html`
      <div class="pagination">
        <button class="page-btn" ${currentPage === 1 ? 'disabled' : ''}
          onclick="this.closest('wanted-page').goToPage(${currentPage - 1})">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
        <span class="page-btn active">${currentPage} / ${totalPages}</span>
        <button class="page-btn" ${currentPage === totalPages ? 'disabled' : ''}
          onclick="this.closest('wanted-page').goToPage(${currentPage + 1})">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
        </button>
      </div>
    `;
  }

  // --- Event handlers ---

  handleContentTabClick(tab: ContentTab): void {
    if (this.contentTab === tab) return;
    this.contentTab = tab;
    savedContentTab = tab;
    this.page.set(1);
    this.refetchActiveQuery();
    this.requestUpdate();
  }

  handleSubTabClick(tab: SubTab): void {
    if (this.subTab === tab) return;
    this.subTab = tab;
    savedSubTab = tab;
    this.page.set(1);
    this.refetchActiveQuery();
    this.requestUpdate();
  }

  handleRefresh(): void {
    this.refetchActiveQuery();
  }

  handleSeriesClick(titleSlug: string): void {
    navigate(`/series/${titleSlug}`);
  }

  handleMovieClick(titleSlug: string): void {
    navigate(`/movies/${titleSlug}`);
  }

  handleSearch(episodeId: number): void {
    this.searchMutation.mutate([episodeId]);
  }

  handleSearchAll(): void {
    if (this.contentTab === 'movies') return;
    const episodes = this.subTab === 'missing'
      ? (this.missingQuery.data.value?.records ?? [])
      : (this.cutoffQuery.data.value?.records ?? []);
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
    this.refetchActiveQuery();
  }

  handleMovieSort(key: MovieSortKey): void {
    if (this.movieSortKey.value === key) {
      this.movieSortDirection.set(this.movieSortDirection.value === 'ascending' ? 'descending' : 'ascending');
    } else {
      this.movieSortKey.set(key);
      this.movieSortDirection.set('ascending');
    }
    this.page.set(1);
    this.movieQuery.refetch();
  }

  goToPage(page: number): void {
    this.page.set(page);
    this.refetchActiveQuery();
  }

  private refetchActiveQuery(): void {
    if (this.contentTab === 'movies') {
      this.movieQuery.refetch();
    } else if (this.contentTab === 'series' || this.contentTab === 'anime') {
      if (this.subTab === 'missing') {
        this.missingQuery.refetch();
      } else {
        this.cutoffQuery.refetch();
      }
    }
  }

  // --- Styles ---

  private getStyles(): string {
    return `
      .wanted-page {
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

      /* Content tabs */
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
        background: var(--bg-card-alt);
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

      .tab-count-stub {
        opacity: 0.5;
      }

      /* Sub-tabs (Missing / Cutoff) */
      .sub-tabs {
        display: flex;
        gap: 0.25rem;
      }

      .sub-tab {
        padding: 0.375rem 0.875rem;
        background: var(--bg-card);
        border: 1px solid var(--border-glass);
        border-radius: 0.375rem;
        color: var(--text-color-muted);
        font-size: 0.8125rem;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.15s ease;
      }

      .sub-tab:hover {
        color: var(--text-color);
        border-color: var(--text-color-muted);
      }

      .sub-tab.active {
        background: var(--color-primary);
        border-color: var(--color-primary);
        color: var(--color-white);
      }

      /* Loading / Error / Empty */
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

      /* Table */
      .wanted-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 0.875rem;
      }

      .wanted-table th,
      .wanted-table td {
        padding: 0.75rem;
        text-align: left;
        border-bottom: 1px solid var(--border-color);
      }

      .wanted-table th {
        font-weight: 600;
        color: var(--text-color-muted);
        white-space: nowrap;
        background-color: var(--bg-card-alt);
      }

      .wanted-table th.sortable {
        cursor: pointer;
        user-select: none;
      }

      .wanted-table th.sortable:hover {
        color: var(--text-color);
      }

      .wanted-table th.sorted {
        color: var(--color-primary);
      }

      .sort-icon {
        vertical-align: middle;
        margin-left: 0.25rem;
      }

      .wanted-table tbody tr:hover td {
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
        font-size: 0.875rem;
      }

      .date-cell {
        white-space: nowrap;
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
    `;
  }
}
