/**
 * Movie Detail page - shows movie info with file status
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';
import { createQuery, invalidateQueries } from '../../core/query';
import { http, type Movie } from '../../core/http';
import { navigate } from '../../router';
import { showSuccess, showError, showInfo } from '../../stores/app.store';
import { signal } from '../../core/reactive';

@customElement('movie-detail-page')
export class MovieDetailPage extends BaseComponent {
  private movieId = signal<number | null>(null);

  private movieQuery: ReturnType<typeof createQuery<Movie | null>> | null = null;

  static get observedAttributes(): string[] {
    return ['id'];
  }

  private createQueries(id: number): void {
    this.movieQuery = createQuery({
      queryKey: ['/movie', id],
      queryFn: () => http.get<Movie>(`/movie/${id}`),
    });

    this.watch(this.movieQuery.data, () => this.requestUpdate());
    this.watch(this.movieQuery.isLoading, () => this.requestUpdate());
  }

  protected onInit(): void {
    // Get movie ID from attribute (set by router)
    const idAttr = this.getAttribute('id');
    if (idAttr) {
      const id = parseInt(idAttr, 10);
      if (!isNaN(id)) {
        this.movieId.set(id);
        this.createQueries(id);
      }
    }
  }

  attributeChangedCallback(name: string, _oldValue: string | null, newValue: string | null): void {
    if (name === 'id' && newValue) {
      const id = parseInt(newValue, 10);
      if (!isNaN(id) && id !== this.movieId.value) {
        this.movieId.set(id);
        this.createQueries(id);
      }
    }
  }

  protected template(): string {
    const movie = this.movieQuery?.data.value;
    const isLoading = this.movieQuery?.isLoading.value ?? true;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
          <span>Loading movie...</span>
        </div>
        ${this.styles()}
      `;
    }

    if (!movie) {
      return html`
        <div class="error-container">
          <p>Movie not found</p>
          <button class="back-btn" onclick="this.closest('movie-detail-page').handleBack()">Back to Movies</button>
        </div>
        ${this.styles()}
      `;
    }

    const posterImage = movie.images?.find((i) => i.coverType === 'poster');
    const fanartImage = movie.images?.find((i) => i.coverType === 'fanart');

    return html`
      <div class="movie-detail">
        <!-- Header with fanart background -->
        <div class="detail-header" style="${fanartImage ? `background-image: linear-gradient(to bottom, rgba(0,0,0,0.3), var(--bg-page)), url('${fanartImage.url}')` : ''}">
          <button class="back-btn" onclick="this.closest('movie-detail-page').handleBack()">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"></polyline>
            </svg>
            Movies
          </button>

          <div class="header-content">
            <div class="poster-container">
              ${posterImage
                ? `<img class="detail-poster" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(movie.title)}">`
                : `<div class="detail-poster-placeholder">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                      <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                      <line x1="7" y1="2" x2="7" y2="22"></line>
                      <line x1="17" y1="2" x2="17" y2="22"></line>
                      <line x1="2" y1="12" x2="22" y2="12"></line>
                    </svg>
                  </div>`}
            </div>

            <div class="header-info">
              <h1 class="movie-title">${escapeHtml(movie.title)}</h1>
              <div class="meta-row">
                <span class="status-badge ${movie.status}">${movie.status}</span>
                ${movie.year ? `<span class="meta-item">${movie.year}</span>` : ''}
                ${movie.runtime ? `<span class="meta-item">${movie.runtime} min</span>` : ''}
                ${movie.certification ? `<span class="meta-item">${escapeHtml(movie.certification)}</span>` : ''}
                ${movie.studio ? `<span class="meta-item">${escapeHtml(movie.studio)}</span>` : ''}
              </div>
              ${movie.genres.length > 0 ? `
                <div class="genres">
                  ${movie.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
                </div>
              ` : ''}
              ${movie.overview ? `<p class="overview">${escapeHtml(movie.overview)}</p>` : ''}

              <div class="stats-row">
                ${movie.imdbRating ? `
                  <div class="stat">
                    <span class="stat-value">${movie.imdbRating.toFixed(1)}</span>
                    <span class="stat-label">IMDB</span>
                  </div>
                ` : ''}
                ${movie.imdbVotes ? `
                  <div class="stat">
                    <span class="stat-value">${this.formatNumber(movie.imdbVotes)}</span>
                    <span class="stat-label">Votes</span>
                  </div>
                ` : ''}
                <div class="stat">
                  <span class="stat-value">${this.formatSize(movie.statistics?.sizeOnDisk ?? 0)}</span>
                  <span class="stat-label">Size</span>
                </div>
                <div class="stat">
                  <span class="stat-value file-status ${movie.hasFile ? 'yes' : 'no'}">${movie.hasFile ? 'Yes' : 'No'}</span>
                  <span class="stat-label">File</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Info panel -->
        <div class="info-panel">
          <div class="info-grid">
            <div class="info-item">
              <span class="info-label">Path</span>
              <span class="info-value">${escapeHtml(movie.path)}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Quality Profile</span>
              <span class="info-value">${movie.qualityProfileId}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Monitored</span>
              <span class="info-value">${movie.monitored ? 'Yes' : 'No'}</span>
            </div>
            ${movie.releaseDate ? `
              <div class="info-item">
                <span class="info-label">Release Date</span>
                <span class="info-value">${movie.releaseDate}</span>
              </div>
            ` : ''}
            ${movie.imdbId ? `
              <div class="info-item">
                <span class="info-label">IMDB</span>
                <span class="info-value">${escapeHtml(movie.imdbId)}</span>
              </div>
            ` : ''}
            <div class="info-item">
              <span class="info-label">Added</span>
              <span class="info-value">${new Date(movie.added).toLocaleDateString()}</span>
            </div>
          </div>
        </div>

        <!-- Actions -->
        <div class="actions-panel">
          <button class="action-btn" onclick="this.closest('movie-detail-page').handleRefreshMetadata()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 2v6h-6"></path>
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
              <path d="M3 22v-6h6"></path>
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
            </svg>
            Refresh Metadata
          </button>
          <button class="action-btn danger" onclick="this.closest('movie-detail-page').handleDelete()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
            Delete
          </button>
        </div>
      </div>

      ${this.styles()}
    `;
  }

  private styles(): string {
    return html`
      <style>
        .movie-detail {
          display: flex;
          flex-direction: column;
          gap: 1.25rem;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from { opacity: 0; transform: translateY(12px); }
          to { opacity: 1; transform: translateY(0); }
        }

        .loading-container {
          display: flex;
          flex-direction: column;
          align-items: center;
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

        .detail-header {
          padding: 1.5rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          background-size: cover;
          background-position: center;
        }

        .back-btn {
          display: inline-flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 0.75rem;
          background: var(--bg-card);
          color: var(--text-color);
          border: 1px solid var(--border-glass);
          border-radius: 0.5rem;
          cursor: pointer;
          font-size: 0.875rem;
          margin-bottom: 1rem;
          transition: all var(--transition-normal);
        }

        .back-btn:hover {
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .header-content {
          display: flex;
          gap: 1.5rem;
        }

        .detail-poster {
          width: 180px;
          aspect-ratio: 2/3;
          object-fit: cover;
          border-radius: 0.5rem;
          box-shadow: 0 4px 20px rgba(0,0,0,0.3);
          flex-shrink: 0;
        }

        .detail-poster-placeholder {
          width: 180px;
          aspect-ratio: 2/3;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--bg-card-center);
          border-radius: 0.5rem;
          color: var(--text-color-muted);
          flex-shrink: 0;
        }

        .header-info {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .movie-title {
          font-size: 1.75rem;
          font-weight: 700;
          margin: 0;
        }

        .meta-row {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          flex-wrap: wrap;
        }

        .meta-item {
          color: var(--text-color-muted);
          font-size: 0.875rem;
        }

        .status-badge {
          display: inline-block;
          padding: 0.2rem 0.625rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
        }
        .status-badge.released { background: rgba(39, 174, 96, 0.15); color: var(--color-success); }
        .status-badge.inCinemas { background: rgba(93, 156, 236, 0.15); color: var(--pir9-blue); }
        .status-badge.announced { background: rgba(240, 173, 78, 0.15); color: var(--color-warning, #f0ad4e); }

        .genres {
          display: flex;
          gap: 0.375rem;
          flex-wrap: wrap;
        }

        .genre-tag {
          padding: 0.125rem 0.5rem;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .overview {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          line-height: 1.5;
          margin: 0;
        }

        .stats-row {
          display: flex;
          gap: 1.5rem;
          margin-top: 0.5rem;
        }

        .stat {
          display: flex;
          flex-direction: column;
          gap: 0.125rem;
        }

        .stat-value {
          font-size: 1.125rem;
          font-weight: 600;
        }

        .stat-label {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }

        .file-status.yes { color: var(--color-success); }
        .file-status.no { color: var(--color-danger); }

        .info-panel {
          padding: 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .info-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
          gap: 1rem;
        }

        .info-item {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
        }

        .info-label {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }

        .info-value {
          font-size: 0.875rem;
          word-break: break-all;
        }

        .actions-panel {
          display: flex;
          gap: 0.75rem;
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .action-btn {
          display: flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 0.875rem;
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          background: var(--bg-input);
          color: var(--text-color);
          cursor: pointer;
          font-size: 0.875rem;
          transition: all var(--transition-normal);
        }

        .action-btn:hover {
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .action-btn.danger:hover {
          border-color: var(--color-danger);
          color: var(--color-danger);
        }

        .error-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1rem;
          padding: 6rem 2rem;
          text-align: center;
        }

        @media (max-width: 640px) {
          .header-content {
            flex-direction: column;
            align-items: center;
            text-align: center;
          }

          .meta-row, .genres, .stats-row {
            justify-content: center;
          }
        }
      </style>
    `;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
  }

  private formatNumber(num: number): string {
    if (num >= 1000000) return `${(num / 1000000).toFixed(1)}M`;
    if (num >= 1000) return `${(num / 1000).toFixed(1)}K`;
    return num.toString();
  }

  // Event handlers
  handleBack(): void {
    navigate('/movies');
  }

  async handleRefreshMetadata(): Promise<void> {
    const id = this.movieId.value;
    if (!id) return;

    try {
      await http.post('/command', { name: 'RefreshMovies', movieId: id });
      showInfo('Refreshing movie metadata from IMDB...');

      // Refetch after a short delay to show updated data
      setTimeout(() => {
        this.movieQuery?.refetch();
        showSuccess('Movie metadata updated');
      }, 5000);
    } catch {
      showError('Failed to queue metadata refresh');
    }
  }

  async handleDelete(): Promise<void> {
    const movie = this.movieQuery?.data.value;
    if (!movie) return;

    if (!confirm(`Are you sure you want to delete "${movie.title}"?`)) return;

    try {
      await http.delete(`/movie/${movie.id}`, { params: { deleteFiles: false } });
      showSuccess(`Deleted "${movie.title}"`);
      invalidateQueries(['/movie']);
      navigate('/movies');
    } catch {
      showError('Failed to delete movie');
    }
  }
}
