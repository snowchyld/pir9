/**
 * Movies index page - main grid/table view
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { getRootFolder, http, type Movie } from '../../core/http';
import { useMoviesQuery } from '../../core/query';
import { navigate } from '../../router';
import {
  type MovieSortKey,
  movieFilter,
  movieRootFolderFilter,
  movieSortDirection,
  movieSortKey,
  movieViewMode,
  searchQuery,
  setMovieFilter,
  setMovieRootFolderFilter,
  setMovieSort,
  setMovieViewMode,
  showError,
  showInfo,
  type ViewMode,
} from '../../stores/app.store';

@customElement('movies-index-page')
export class MoviesIndexPage extends BaseComponent {
  private moviesQuery = useMoviesQuery();
  private refreshCommandId: number | null = null;

  protected onInit(): void {
    this.watch(this.moviesQuery.data);
    this.watch(this.moviesQuery.isLoading);
    this.watch(this.moviesQuery.isError);
    this.watch(movieViewMode);
    this.watch(movieSortKey);
    this.watch(movieSortDirection);
    this.watch(movieFilter);
    this.watch(movieRootFolderFilter);
    this.watch(searchQuery);
  }

  protected template(): string {
    const movies = this.moviesQuery.data.value ?? [];
    const isLoading = this.moviesQuery.isLoading.value;
    const isError = this.moviesQuery.isError.value;
    const viewMode = movieViewMode.value;
    const sortKey = movieSortKey.value;
    const sortDir = movieSortDirection.value;
    const filter = movieFilter.value;
    const rootFolderFilter = movieRootFolderFilter.value;
    const search = searchQuery.value.toLowerCase();

    // Collect unique root folders (before filtering)
    const rootFolders = [
      ...new Set(movies.map((m) => m.rootFolderPath || getRootFolder(m.path))),
    ].sort();

    // Filter and sort movies
    let filtered = movies;

    if (search) {
      filtered = filtered.filter(
        (m) => m.title.toLowerCase().includes(search) || m.studio?.toLowerCase().includes(search),
      );
    }

    if (filter !== 'all') {
      filtered = filtered.filter((m) => {
        switch (filter) {
          case 'monitored':
            return m.monitored;
          case 'unmonitored':
            return !m.monitored;
          case 'released':
            return m.status === 'released';
          case 'inCinemas':
            return m.status === 'inCinemas';
          case 'announced':
            return m.status === 'announced';
          case 'missing':
            return m.monitored && !m.hasFile;
          default:
            return true;
        }
      });
    }

    if (rootFolderFilter !== 'all') {
      filtered = filtered.filter(
        (m) => (m.rootFolderPath || getRootFolder(m.path)) === rootFolderFilter,
      );
    }

    filtered = [...filtered].sort((a, b) => {
      let comparison = 0;
      const aVal = this.getSortValue(a, sortKey);
      const bVal = this.getSortValue(b, sortKey);

      if (aVal < bVal) comparison = -1;
      if (aVal > bVal) comparison = 1;

      return sortDir === 'descending' ? -comparison : comparison;
    });

    return html`
      <div class="movies-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Movies</h1>
            <span class="movie-count">${filtered.length} movies</span>
          </div>

          <div class="toolbar-right">
            <select
              class="filter-select"
              onchange="this.closest('movies-index-page').handleFilterChange(event)"
            >
              <option value="all" ${filter === 'all' ? 'selected' : ''}>All</option>
              <option value="monitored" ${filter === 'monitored' ? 'selected' : ''}>Monitored</option>
              <option value="unmonitored" ${filter === 'unmonitored' ? 'selected' : ''}>Unmonitored</option>
              <option value="released" ${filter === 'released' ? 'selected' : ''}>Released</option>
              <option value="inCinemas" ${filter === 'inCinemas' ? 'selected' : ''}>In Cinemas</option>
              <option value="announced" ${filter === 'announced' ? 'selected' : ''}>Announced</option>
              <option value="missing" ${filter === 'missing' ? 'selected' : ''}>Missing</option>
            </select>

            <!-- Root folder filter dropdown -->
            ${
              rootFolders.length > 1
                ? html`
            <select
              class="filter-select"
              onchange="this.closest('movies-index-page').handleRootFolderFilterChange(event)"
            >
              <option value="all" ${rootFolderFilter === 'all' ? 'selected' : ''}>All Folders</option>
              ${rootFolders.map((f) => html`<option value="${escapeHtml(f)}" ${rootFolderFilter === f ? 'selected' : ''}>${escapeHtml(f)}</option>`).join('')}
            </select>
            `
                : ''
            }

            <select
              class="sort-select"
              onchange="this.closest('movies-index-page').handleSortChange(event)"
            >
              <option value="sortTitle" ${sortKey === 'sortTitle' ? 'selected' : ''}>Title</option>
              <option value="status" ${sortKey === 'status' ? 'selected' : ''}>Status</option>
              <option value="studio" ${sortKey === 'studio' ? 'selected' : ''}>Studio</option>
              <option value="added" ${sortKey === 'added' ? 'selected' : ''}>Added</option>
              <option value="year" ${sortKey === 'year' ? 'selected' : ''}>Year</option>
              <option value="sizeOnDisk" ${sortKey === 'sizeOnDisk' ? 'selected' : ''}>Size</option>
              <option value="ratings" ${sortKey === 'ratings' ? 'selected' : ''}>Rating</option>
            </select>

            <button
              class="sort-dir-btn"
              onclick="this.closest('movies-index-page').handleSortDirToggle()"
              title="${sortDir === 'ascending' ? 'Ascending' : 'Descending'}"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                   class="${sortDir === 'descending' ? 'rotate-180' : ''}">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </button>

            ${
              this.refreshCommandId
                ? html`<button
                  class="refresh-all-btn stop"
                  onclick="this.closest('movies-index-page').handleStopRefresh()"
                  title="Stop refreshing movie metadata"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <rect x="6" y="6" width="12" height="12" rx="1"></rect>
                  </svg>
                  <span>Stop Refresh</span>
                </button>`
                : html`<button
                  class="refresh-all-btn"
                  onclick="this.closest('movies-index-page').handleRefreshAll()"
                  title="Refresh all movie metadata from IMDB"
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="23 4 23 10 17 10"></polyline>
                    <polyline points="1 20 1 14 7 14"></polyline>
                    <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
                  </svg>
                  <span>Refresh All</span>
                </button>`
            }

            <div class="view-modes">
              ${this.renderViewModeButton('posters', 'Posters')}
              ${this.renderViewModeButton('table', 'Table')}
            </div>

            <button
              class="add-movie-btn"
              onclick="this.closest('movies-index-page').handleAddMovie()"
              title="Add Movie"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
              Add Movie
            </button>
          </div>
        </div>

        ${isLoading ? this.renderLoading() : ''}
        ${isError ? this.renderError() : ''}
        ${!isLoading && !isError ? this.renderContent(filtered, viewMode) : ''}
      </div>

      <style>
        .movies-page {
          display: flex;
          flex-direction: column;
          gap: 1.25rem;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from { opacity: 0; transform: translateY(12px); }
          to { opacity: 1; transform: translateY(0); }
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          flex-wrap: wrap;
          gap: 1rem;
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
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
          background: linear-gradient(135deg, var(--text-color) 0%, var(--pir9-blue) 100%);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
          background-clip: text;
        }

        .movie-count {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          padding: 0.25rem 0.625rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
        }

        .toolbar-right {
          display: flex;
          align-items: center;
          gap: 0.625rem;
        }

        .filter-select,
        .sort-select {
          padding: 0.5rem 0.875rem;
          background-color: var(--bg-input);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          font-size: 0.875rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .filter-select:hover,
        .sort-select:hover {
          border-color: var(--border-glass);
          background-color: var(--bg-input-hover);
        }

        .sort-dir-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.5rem;
          background-color: var(--bg-input);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .sort-dir-btn:hover {
          background-color: var(--bg-input-hover);
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .sort-dir-btn svg.rotate-180 { transform: rotate(180deg); }

        .refresh-all-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.875rem;
          background: var(--btn-primary-bg);
          color: var(--color-white);
          border: none;
          border-radius: 0.5rem;
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .refresh-all-btn:hover {
          background: var(--btn-primary-bg-hover);
          box-shadow: var(--glow-primary);
          transform: translateY(-1px);
        }

        .refresh-all-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
          transform: none;
        }

        .refresh-all-btn.loading svg {
          animation: spin 1s linear infinite;
        }

        .refresh-all-btn.stop {
          background: var(--color-danger, #dc3545);
        }

        .refresh-all-btn.stop:hover {
          background: var(--color-danger-hover, #c82333);
          box-shadow: 0 0 12px rgba(220, 53, 69, 0.4);
        }

        .view-modes {
          display: flex;
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          overflow: hidden;
          background: var(--bg-input);
        }

        .view-mode-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.5rem 0.625rem;
          background-color: transparent;
          color: var(--text-color-muted);
          border: none;
          cursor: pointer;
          transition: all var(--transition-fast) var(--ease-out-expo);
        }

        .view-mode-btn:not(:last-child) {
          border-right: 1px solid var(--border-input);
        }

        .view-mode-btn.active {
          background: linear-gradient(135deg, var(--color-primary), var(--pir9-blue));
          color: var(--color-white);
          box-shadow: 0 2px 8px rgba(93, 156, 236, 0.4);
        }

        .add-movie-btn {
          display: flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 0.875rem;
          background: linear-gradient(135deg, var(--color-primary), var(--pir9-blue));
          color: var(--color-white);
          border: none;
          border-radius: 0.5rem;
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
          box-shadow: 0 2px 8px rgba(93, 156, 236, 0.3);
        }

        .add-movie-btn:hover {
          box-shadow: 0 4px 16px rgba(93, 156, 236, 0.5);
          transform: translateY(-1px);
        }

        .loading-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 1rem;
          padding: 6rem 2rem;
        }

        .loading-spinner {
          width: 48px;
          height: 48px;
          border: 3px solid var(--border-glass);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        @keyframes spin { to { transform: rotate(360deg); } }

        .error-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1.25rem;
          padding: 6rem 2rem;
          text-align: center;
        }

        .empty-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1.25rem;
          padding: 6rem 2rem;
          text-align: center;
        }

        .poster-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
          gap: 1.25rem;
        }

        .poster-card {
          position: relative;
          border-radius: 0.875rem;
          overflow: hidden;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          box-shadow: var(--shadow-card);
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .poster-card:hover {
          transform: translateY(-6px) scale(1.02);
          box-shadow: var(--shadow-card-hover), 0 0 30px rgba(93, 156, 236, 0.15);
          border-color: rgba(93, 156, 236, 0.3);
        }

        .poster-image {
          width: 100%;
          aspect-ratio: 2/3;
          object-fit: cover;
          background-color: var(--bg-card-center);
        }

        .poster-placeholder {
          width: 100%;
          aspect-ratio: 2/3;
          display: flex;
          align-items: center;
          justify-content: center;
          background: linear-gradient(135deg, var(--bg-card-center), var(--bg-card));
          color: var(--text-color-muted);
        }

        .poster-info {
          padding: 0.75rem;
          background: linear-gradient(to top, rgba(0,0,0,0.6), transparent);
          position: absolute;
          bottom: 0;
          left: 0;
          right: 0;
        }

        .poster-title {
          font-size: 0.875rem;
          font-weight: 600;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          color: #fff;
          text-shadow: 0 1px 3px rgba(0,0,0,0.5);
        }

        .poster-year {
          font-size: 0.75rem;
          color: rgba(255,255,255,0.8);
        }

        .poster-status {
          position: absolute;
          top: 0.5rem;
          right: 0.5rem;
          width: 10px;
          height: 10px;
          border-radius: 50%;
          z-index: 2;
          box-shadow: 0 0 8px currentColor;
        }

        .poster-status.released { background-color: var(--color-success); color: var(--color-success); }
        .poster-status.inCinemas { background-color: var(--pir9-blue); color: var(--pir9-blue); }
        .poster-status.announced { background-color: var(--color-warning, #f0ad4e); color: var(--color-warning, #f0ad4e); }
        .poster-status.tba,
        .poster-status.deleted { background-color: var(--color-gray-600); color: var(--color-gray-600); }

        .movie-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          overflow: hidden;
        }

        .movie-table th,
        .movie-table td {
          padding: 0.875rem 1rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color-light);
        }

        .movie-table th {
          font-weight: 600;
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-color-muted);
          background: var(--bg-card-center);
        }

        .movie-table th.sortable {
          cursor: pointer;
          user-select: none;
          transition: color var(--transition-fast);
        }

        .movie-table th.sortable:hover {
          color: var(--pir9-blue);
          background: var(--bg-input-hover);
        }

        .movie-table th.sortable.sorted {
          color: var(--pir9-blue);
        }

        .movie-table th.sortable svg {
          display: inline-block;
          vertical-align: middle;
          margin-left: 0.25rem;
        }

        .movie-table tr {
          cursor: pointer;
          transition: background var(--transition-fast);
        }

        .movie-table tr:hover {
          background-color: var(--bg-hover);
        }

        .movie-title-cell {
          font-weight: 500;
          color: var(--text-color);
        }

        .status-badge {
          display: inline-block;
          padding: 0.2rem 0.5rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
        }

        .status-badge.released { background: rgba(39, 174, 96, 0.15); color: var(--color-success); }
        .status-badge.inCinemas { background: rgba(93, 156, 236, 0.15); color: var(--pir9-blue); }
        .status-badge.announced { background: rgba(240, 173, 78, 0.15); color: var(--color-warning, #f0ad4e); }

        .has-file-badge {
          display: inline-block;
          width: 8px;
          height: 8px;
          border-radius: 50%;
        }
        .has-file-badge.yes { background: var(--color-success); }
        .has-file-badge.no { background: var(--color-danger); }
      </style>
    `;
  }

  private renderContent(movies: Movie[], viewMode: ViewMode): string {
    if (movies.length === 0) {
      return this.renderEmpty();
    }

    if (viewMode === 'table') {
      return this.renderTable(movies);
    }

    return this.renderPosterGrid(movies);
  }

  private renderPosterGrid(movies: Movie[]): string {
    return html`
      <div class="poster-grid">
        ${movies.map((m) => this.renderPosterCard(m)).join('')}
      </div>
    `;
  }

  private renderPosterCard(movie: Movie): string {
    const posterImage = movie.images?.find((i) => i.coverType === 'poster');
    const statusClass = movie.monitored ? movie.status : 'unmonitored';

    return html`
      <div class="poster-card"
           onclick="this.closest('movies-index-page').handleMovieClick('${escapeHtml(movie.titleSlug)}')">
        <div class="poster-status ${statusClass}"></div>
        ${
          posterImage
            ? `<img class="poster-image" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(movie.title)}" loading="lazy">`
            : `<div class="poster-placeholder">
              <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                <line x1="7" y1="2" x2="7" y2="22"></line>
                <line x1="17" y1="2" x2="17" y2="22"></line>
                <line x1="2" y1="12" x2="22" y2="12"></line>
              </svg>
            </div>`
        }
        <div class="poster-info">
          <div class="poster-title">${escapeHtml(movie.title)}</div>
          <div class="poster-year">${movie.year || ''}</div>
        </div>
      </div>
    `;
  }

  private renderTable(movies: Movie[]): string {
    const sortKey = movieSortKey.value;
    const sortDir = movieSortDirection.value;
    const sortIcon =
      sortDir === 'ascending'
        ? '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
        : '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>';

    return html`
      <table class="movie-table">
        <thead>
          <tr>
            <th class="sortable ${sortKey === 'sortTitle' ? 'sorted' : ''}" onclick="this.closest('movies-index-page').handleColumnSort('sortTitle')">
              Title ${sortKey === 'sortTitle' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'year' ? 'sorted' : ''}" onclick="this.closest('movies-index-page').handleColumnSort('year')">
              Year ${sortKey === 'year' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'studio' ? 'sorted' : ''}" onclick="this.closest('movies-index-page').handleColumnSort('studio')">
              Studio ${sortKey === 'studio' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'status' ? 'sorted' : ''}" onclick="this.closest('movies-index-page').handleColumnSort('status')">
              Status ${sortKey === 'status' ? safeHtml(sortIcon) : ''}
            </th>
            <th>File</th>
            <th class="sortable ${sortKey === 'ratings' ? 'sorted' : ''}" onclick="this.closest('movies-index-page').handleColumnSort('ratings')">
              Rating ${sortKey === 'ratings' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'sizeOnDisk' ? 'sorted' : ''}" onclick="this.closest('movies-index-page').handleColumnSort('sizeOnDisk')">
              Size ${sortKey === 'sizeOnDisk' ? safeHtml(sortIcon) : ''}
            </th>
          </tr>
        </thead>
        <tbody>
          ${movies
            .map(
              (m) => html`
            <tr onclick="this.closest('movies-index-page').handleMovieClick('${escapeHtml(m.titleSlug)}')">
              <td class="movie-title-cell">${escapeHtml(m.title)}</td>
              <td>${m.year || '-'}</td>
              <td>${escapeHtml(m.studio || '-')}</td>
              <td><span class="status-badge ${m.status}">${m.status}</span></td>
              <td><span class="has-file-badge ${m.hasFile ? 'yes' : 'no'}"></span></td>
              <td>${this.formatRating(m)}</td>
              <td>${this.formatSize(m.statistics?.sizeOnDisk ?? 0)}</td>
            </tr>
          `,
            )
            .join('')}
        </tbody>
      </table>
    `;
  }

  private renderLoading(): string {
    return html`
      <div class="loading-container">
        <div class="loading-spinner"></div>
        <span>Loading movies...</span>
      </div>
    `;
  }

  private renderError(): string {
    return html`
      <div class="error-container">
        <span>Failed to load movies</span>
        <button onclick="this.closest('movies-index-page').handleRetry()">Retry</button>
      </div>
    `;
  }

  private renderEmpty(): string {
    return html`
      <div class="empty-container">
        <svg width="72" height="72" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" style="color: var(--text-color-dim)">
          <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
          <line x1="7" y1="2" x2="7" y2="22"></line>
          <line x1="17" y1="2" x2="17" y2="22"></line>
          <line x1="2" y1="12" x2="22" y2="12"></line>
        </svg>
        <p style="color: var(--text-color-muted)">No movies found. Add a movie to get started.</p>
        <button
          class="add-movie-btn"
          onclick="this.closest('movies-index-page').handleAddMovie()"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
          Add Movie
        </button>
      </div>
    `;
  }

  private renderViewModeButton(mode: ViewMode, label: string): string {
    const active = movieViewMode.value === mode;
    const icons: Record<string, string> = {
      posters:
        '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>',
      table:
        '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>',
    };

    return html`
      <button
        class="view-mode-btn ${active ? 'active' : ''}"
        onclick="this.closest('movies-index-page').handleViewModeChange('${mode}')"
        title="${label}"
      >
        ${icons[mode] || ''}
      </button>
    `;
  }

  private getSortValue(movie: Movie, key: MovieSortKey): string | number {
    switch (key) {
      case 'sortTitle':
        return movie.sortTitle || movie.title.toLowerCase();
      case 'status':
        return movie.status;
      case 'studio':
        return movie.studio || '';
      case 'added':
        return movie.added || '';
      case 'year':
        return movie.year || 0;
      case 'sizeOnDisk':
        return movie.statistics?.sizeOnDisk ?? 0;
      case 'ratings':
        return movie.ratings?.value ?? movie.imdbRating ?? 0;
      default:
        return movie.sortTitle || '';
    }
  }

  private formatRating(movie: Movie): string {
    const rating = movie.ratings?.value ?? movie.imdbRating;
    if (!rating) return '-';
    return rating.toFixed(1);
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
  }

  // Event handlers
  handleMovieClick(titleSlug: string): void {
    navigate(`/movies/${titleSlug}`);
  }

  handleFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMovieFilter(select.value);
  }

  handleRootFolderFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMovieRootFolderFilter(select.value);
  }

  handleSortChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMovieSort(select.value as MovieSortKey);
  }

  handleColumnSort(key: string): void {
    if (movieSortKey.value === key) {
      const current = movieSortDirection.value;
      movieSortDirection.set(current === 'ascending' ? 'descending' : 'ascending');
    } else {
      setMovieSort(key as MovieSortKey);
      movieSortDirection.set('ascending');
    }
  }

  handleSortDirToggle(): void {
    setMovieSort(movieSortKey.value);
  }

  handleViewModeChange(mode: string): void {
    setMovieViewMode(mode as ViewMode);
  }

  handleAddMovie(): void {
    navigate('/add/movies');
  }

  async handleRefreshAll(): Promise<void> {
    try {
      const filter = movieFilter.value;
      const rootFolderFilter = movieRootFolderFilter.value;
      const hasActiveFilter = filter !== 'all' || rootFolderFilter !== 'all';

      let result: { id: number };

      if (hasActiveFilter) {
        const allMovies = this.moviesQuery.data.value ?? [];
        let filtered = allMovies;

        if (filter !== 'all') {
          filtered = filtered.filter((m) => {
            switch (filter) {
              case 'monitored':
                return m.monitored;
              case 'unmonitored':
                return !m.monitored;
              case 'released':
                return m.status === 'released';
              case 'inCinemas':
                return m.status === 'inCinemas';
              case 'announced':
                return m.status === 'announced';
              case 'missing':
                return m.monitored && !m.hasFile;
              default:
                return true;
            }
          });
        }
        if (rootFolderFilter !== 'all') {
          filtered = filtered.filter(
            (m) => (m.rootFolderPath || getRootFolder(m.path)) === rootFolderFilter,
          );
        }

        const movieIds = filtered.map((m) => m.id);
        result = await http.post<{ id: number }>('/command', { name: 'RefreshMovies', movieIds });
        showInfo(`Refreshing ${movieIds.length} movies...`, 'Refresh Started');
      } else {
        result = await http.post<{ id: number }>('/command', { name: 'RefreshMovies' });
        showInfo('Refreshing all movie metadata...', 'Refresh Started');
      }

      this.refreshCommandId = result.id;
      this.requestUpdate();
    } catch (error) {
      console.error('[MoviesIndex] Failed to refresh all movies:', error);
      showError('Failed to start refresh command', 'Refresh Failed');
    }
  }

  async handleStopRefresh(): Promise<void> {
    if (!this.refreshCommandId) return;

    try {
      await http.delete(`/command/${this.refreshCommandId}`);
      showInfo('Stopping movie refresh...', 'Refresh Stopping');
    } catch (error) {
      console.error('[MoviesIndex] Failed to stop refresh:', error);
      showError('Failed to stop refresh command', 'Stop Failed');
    } finally {
      this.refreshCommandId = null;
      this.requestUpdate();
      // Refetch to show whatever was updated so far
      this.moviesQuery.refetch();
    }
  }

  handleRetry(): void {
    this.moviesQuery.refetch();
  }
}
