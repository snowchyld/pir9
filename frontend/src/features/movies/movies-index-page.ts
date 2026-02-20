/**
 * Movies index page - main grid/table view
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';
import { useMoviesQuery } from '../../core/query';
import {
  movieViewMode,
  movieSortKey,
  movieSortDirection,
  movieFilter,
  searchQuery,
  setMovieViewMode,
  setMovieSort,
  setMovieFilter,
  type ViewMode,
  type MovieSortKey,
} from '../../stores/app.store';
import { navigate } from '../../router';
import type { Movie } from '../../core/http';

@customElement('movies-index-page')
export class MoviesIndexPage extends BaseComponent {
  private moviesQuery = useMoviesQuery();

  protected onInit(): void {
    this.watch(this.moviesQuery.data);
    this.watch(this.moviesQuery.isLoading);
    this.watch(this.moviesQuery.isError);
    this.watch(movieViewMode);
    this.watch(movieSortKey);
    this.watch(movieSortDirection);
    this.watch(movieFilter);
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
    const search = searchQuery.value.toLowerCase();

    // Filter and sort movies
    let filtered = movies;

    if (search) {
      filtered = filtered.filter((m) =>
        m.title.toLowerCase().includes(search) ||
        m.studio?.toLowerCase().includes(search)
      );
    }

    if (filter !== 'all') {
      filtered = filtered.filter((m) => {
        switch (filter) {
          case 'monitored': return m.monitored;
          case 'unmonitored': return !m.monitored;
          case 'released': return m.status === 'released';
          case 'inCinemas': return m.status === 'inCinemas';
          case 'announced': return m.status === 'announced';
          case 'missing': return m.monitored && !m.hasFile;
          default: return true;
        }
      });
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
              value="${filter}"
              onchange="this.closest('movies-index-page').handleFilterChange(event)"
            >
              <option value="all">All</option>
              <option value="monitored">Monitored</option>
              <option value="unmonitored">Unmonitored</option>
              <option value="released">Released</option>
              <option value="inCinemas">In Cinemas</option>
              <option value="announced">Announced</option>
              <option value="missing">Missing</option>
            </select>

            <select
              class="sort-select"
              value="${sortKey}"
              onchange="this.closest('movies-index-page').handleSortChange(event)"
            >
              <option value="sortTitle">Title</option>
              <option value="status">Status</option>
              <option value="studio">Studio</option>
              <option value="added">Added</option>
              <option value="year">Year</option>
              <option value="sizeOnDisk">Size</option>
              <option value="ratings">Rating</option>
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
           onclick="this.closest('movies-index-page').handleMovieClick(${movie.id})">
        <div class="poster-status ${statusClass}"></div>
        ${posterImage
          ? `<img class="poster-image" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(movie.title)}" loading="lazy">`
          : `<div class="poster-placeholder">
              <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                <line x1="7" y1="2" x2="7" y2="22"></line>
                <line x1="17" y1="2" x2="17" y2="22"></line>
                <line x1="2" y1="12" x2="22" y2="12"></line>
              </svg>
            </div>`}
        <div class="poster-info">
          <div class="poster-title">${escapeHtml(movie.title)}</div>
          <div class="poster-year">${movie.year || ''}</div>
        </div>
      </div>
    `;
  }

  private renderTable(movies: Movie[]): string {
    return html`
      <table class="movie-table">
        <thead>
          <tr>
            <th>Title</th>
            <th>Year</th>
            <th>Studio</th>
            <th>Status</th>
            <th>File</th>
            <th>Rating</th>
            <th>Size</th>
          </tr>
        </thead>
        <tbody>
          ${movies.map((m) => html`
            <tr onclick="this.closest('movies-index-page').handleMovieClick(${m.id})">
              <td class="movie-title-cell">${escapeHtml(m.title)}</td>
              <td>${m.year || '-'}</td>
              <td>${escapeHtml(m.studio || '-')}</td>
              <td><span class="status-badge ${m.status}">${m.status}</span></td>
              <td><span class="has-file-badge ${m.hasFile ? 'yes' : 'no'}"></span></td>
              <td>${m.imdbRating ? m.imdbRating.toFixed(1) : '-'}</td>
              <td>${this.formatSize(m.statistics?.sizeOnDisk ?? 0)}</td>
            </tr>
          `).join('')}
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
      posters: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>',
      table: '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>',
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
      case 'sortTitle': return movie.sortTitle || movie.title.toLowerCase();
      case 'status': return movie.status;
      case 'studio': return movie.studio || '';
      case 'added': return movie.added || '';
      case 'year': return movie.year || 0;
      case 'sizeOnDisk': return movie.statistics?.sizeOnDisk ?? 0;
      case 'ratings': return movie.imdbRating ?? 0;
      default: return movie.sortTitle || '';
    }
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
  }

  // Event handlers
  handleMovieClick(id: number): void {
    navigate(`/movies/${id}`);
  }

  handleFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMovieFilter(select.value);
  }

  handleSortChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMovieSort(select.value as MovieSortKey);
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

  handleRetry(): void {
    this.moviesQuery.refetch();
  }
}
