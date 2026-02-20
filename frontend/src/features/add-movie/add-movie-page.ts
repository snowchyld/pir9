/**
 * Add Movie page - search IMDB and add new movies
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http, type Movie, type MovieLookupResult } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

interface RootFolder {
  id: number;
  path: string;
  freeSpace: number;
}

interface QualityProfile {
  id: number;
  name: string;
}

@customElement('add-movie-page')
export class AddMoviePage extends BaseComponent {
  private searchTerm = signal('');
  private searchResults = signal<MovieLookupResult[]>([]);
  private isSearching = signal(false);
  private selectedMovie = signal<MovieLookupResult | null>(null);

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder'),
  });

  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => http.get<QualityProfile[]>('/qualityprofile'),
  });

  private addMovieMutation = createMutation({
    mutationFn: (movie: Partial<Movie>) => http.post<Movie>('/movie', movie),
    onSuccess: (data) => {
      invalidateQueries(['/movie']);
      showSuccess('Movie added successfully');
      navigate(`/movies/${data.titleSlug}`);
    },
    onError: () => {
      showError('Failed to add movie');
    },
  });

  protected onInit(): void {
    this.watch(this.searchTerm);
    this.watch(this.searchResults);
    this.watch(this.isSearching);
    this.watch(this.selectedMovie);
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.qualityProfilesQuery.data);
  }

  protected template(): string {
    const results = this.searchResults.value;
    const isSearching = this.isSearching.value;
    const selected = this.selectedMovie.value;
    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const qualityProfiles = this.qualityProfilesQuery.data.value ?? [];

    return html`
      <div class="add-movie-page">
        <h1 class="page-title">Add New Movie</h1>

        <div class="search-section">
          <div class="search-box">
            <svg class="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            <input
              type="text"
              class="search-input"
              placeholder="Search for a movie..."
              value="${escapeHtml(this.searchTerm.value)}"
              oninput="this.closest('add-movie-page').handleSearchInput(this.value)"
              onkeydown="if(event.key === 'Enter') this.closest('add-movie-page').handleSearch()"
            />
            <button
              class="search-btn"
              onclick="this.closest('add-movie-page').handleSearch()"
              ${isSearching ? 'disabled' : ''}
            >
              ${isSearching ? 'Searching...' : 'Search'}
            </button>
          </div>
        </div>

        ${selected ? this.renderAddForm(selected, rootFolders, qualityProfiles) : ''}

        ${
          !selected && results.length > 0
            ? html`
          <div class="results-section">
            <h2 class="section-title">Search Results</h2>
            <div class="results-grid">
              ${results.map((result, i) => this.renderSearchResult(result, i)).join('')}
            </div>
          </div>
        `
            : ''
        }

        ${
          !selected && results.length === 0 && this.searchTerm.value && !isSearching
            ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            <p>No movies found</p>
            <p class="hint">Try a different search term</p>
          </div>
        `
            : ''
        }

        <div class="import-link">
          <a href="/add/movies/import" onclick="event.preventDefault(); this.closest('add-movie-page').navigateToImport()">
            Looking to import movies from disk?
          </a>
        </div>
      </div>

      <style>
        .add-movie-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .search-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .search-box {
          display: flex;
          align-items: center;
          gap: 0.75rem;
        }

        .search-icon {
          color: var(--text-color-muted);
          flex-shrink: 0;
        }

        .search-input {
          flex: 1;
          padding: 0.75rem;
          font-size: 1rem;
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
          padding: 0.75rem 1.5rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
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

        .results-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
        }

        .results-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 1rem;
        }

        .result-card {
          display: flex;
          gap: 1rem;
          padding: 1rem;
          background-color: var(--bg-card-alt);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          cursor: pointer;
          transition: border-color 0.15s;
        }

        .result-card:hover {
          border-color: var(--color-primary);
        }

        .result-poster {
          width: 80px;
          height: 120px;
          flex-shrink: 0;
          border-radius: 0.25rem;
          overflow: hidden;
          background-color: var(--bg-card);
        }

        .result-poster img {
          width: 100%;
          height: 100%;
          object-fit: cover;
        }

        .result-poster-placeholder {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 100%;
          height: 100%;
          color: var(--text-color-muted);
        }

        .result-info {
          flex: 1;
          min-width: 0;
        }

        .result-title {
          font-weight: 500;
          margin-bottom: 0.25rem;
        }

        .result-meta {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin-bottom: 0.5rem;
        }

        .result-overview {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          display: -webkit-box;
          -webkit-line-clamp: 3;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }

        .add-form {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .form-header {
          display: flex;
          align-items: flex-start;
          gap: 1.5rem;
          margin-bottom: 1.5rem;
          padding-bottom: 1.5rem;
          border-bottom: 1px solid var(--border-color);
        }

        .form-poster {
          width: 150px;
          flex-shrink: 0;
        }

        .form-poster img {
          width: 100%;
          border-radius: 0.375rem;
        }

        .form-movie-info {
          flex: 1;
        }

        .form-title {
          font-size: 1.25rem;
          font-weight: 600;
          margin: 0 0 0.5rem 0;
        }

        .form-meta {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin-bottom: 0.75rem;
        }

        .form-overview {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          line-height: 1.5;
        }

        .form-genres {
          display: flex;
          gap: 0.375rem;
          flex-wrap: wrap;
          margin-top: 0.5rem;
        }

        .genre-tag {
          padding: 0.125rem 0.5rem;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .back-btn {
          padding: 0.375rem 0.75rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          cursor: pointer;
          margin-bottom: 1rem;
        }

        .back-btn:hover {
          background-color: var(--btn-default-bg-hover);
        }

        .form-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
          gap: 1.5rem;
        }

        .form-group {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .form-label {
          font-size: 0.875rem;
          font-weight: 500;
        }

        .form-select, .form-input {
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .form-select:focus, .form-input:focus {
          outline: none;
          border-color: var(--color-primary);
        }

        .checkbox-label {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          cursor: pointer;
        }

        .checkbox-label input[type="checkbox"] {
          width: 16px;
          height: 16px;
          accent-color: var(--color-primary);
        }

        .form-actions {
          display: flex;
          justify-content: flex-end;
          gap: 0.5rem;
          margin-top: 1.5rem;
          padding-top: 1.5rem;
          border-top: 1px solid var(--border-color);
        }

        .cancel-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .cancel-btn:hover {
          background-color: var(--btn-default-bg-hover);
        }

        .add-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--color-success);
          border: 1px solid var(--color-success);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .add-btn:hover {
          opacity: 0.9;
        }

        .empty-state {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 0.5rem;
          padding: 3rem;
          text-align: center;
          color: var(--text-color-muted);
        }

        .empty-state .hint {
          font-size: 0.875rem;
        }

        .import-link {
          text-align: center;
          padding: 1rem;
        }

        .import-link a {
          color: var(--color-primary);
          text-decoration: none;
        }

        .import-link a:hover {
          text-decoration: underline;
        }
      </style>
    `;
  }

  private renderSearchResult(result: MovieLookupResult, index: number): string {
    const posterImage = result.images?.find((i) => i.coverType === 'poster');
    const ratingText = result.ratings?.value ? `${result.ratings.value.toFixed(1)}` : '';

    return html`
      <div class="result-card" onclick="this.closest('add-movie-page').selectMovie(${index})">
        <div class="result-poster">
          ${
            posterImage?.remoteUrl
              ? html`
            <img src="${escapeHtml(posterImage.remoteUrl)}" alt="${escapeHtml(result.title)}" loading="lazy" />
          `
              : html`
            <div class="result-poster-placeholder">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                <line x1="7" y1="2" x2="7" y2="22"></line>
                <line x1="17" y1="2" x2="17" y2="22"></line>
              </svg>
            </div>
          `
          }
        </div>
        <div class="result-info">
          <div class="result-title">${escapeHtml(result.title)}</div>
          <div class="result-meta">${result.year || 'Unknown year'}${ratingText ? ` • ${ratingText}` : ''}${result.runtime ? ` • ${result.runtime} min` : ''}</div>
          <div class="result-overview">${escapeHtml(result.overview ?? '')}</div>
        </div>
      </div>
    `;
  }

  private renderAddForm(
    movie: MovieLookupResult,
    rootFolders: RootFolder[],
    profiles: QualityProfile[],
  ): string {
    const posterImage = movie.images?.find((i) => i.coverType === 'poster');

    return html`
      <div class="add-form">
        <button class="back-btn" onclick="this.closest('add-movie-page').clearSelection()">
          ← Back to results
        </button>

        <div class="form-header">
          <div class="form-poster">
            ${
              posterImage?.remoteUrl
                ? html`
              <img src="${escapeHtml(posterImage.remoteUrl)}" alt="${escapeHtml(movie.title)}" />
            `
                : ''
            }
          </div>
          <div class="form-movie-info">
            <h2 class="form-title">${escapeHtml(movie.title)}</h2>
            <div class="form-meta">
              ${movie.year || 'Unknown year'}
              ${movie.studio ? ` • ${escapeHtml(movie.studio)}` : ''}
              ${movie.runtime ? ` • ${movie.runtime} min` : ''}
              ${movie.certification ? ` • ${escapeHtml(movie.certification)}` : ''}
            </div>
            <div class="form-overview">${escapeHtml(movie.overview ?? '')}</div>
            ${
              movie.genres.length > 0
                ? `
              <div class="form-genres">
                ${movie.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
              </div>
            `
                : ''
            }
          </div>
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label class="form-label">Root Folder</label>
            <select class="form-select" id="rootFolder">
              ${rootFolders
                .map(
                  (folder) => html`
                <option value="${escapeHtml(folder.path)}">${escapeHtml(folder.path)}</option>
              `,
                )
                .join('')}
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">Quality Profile</label>
            <select class="form-select" id="qualityProfile">
              ${profiles
                .map(
                  (profile) => html`
                <option value="${profile.id}">${escapeHtml(profile.name)}</option>
              `,
                )
                .join('')}
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">Monitored</label>
            <label class="checkbox-label">
              <input type="checkbox" id="monitored" checked />
              <span>Monitor this movie</span>
            </label>
          </div>

          <div class="form-group">
            <label class="form-label">Search on Add</label>
            <label class="checkbox-label">
              <input type="checkbox" id="searchOnAdd" checked />
              <span>Start search for movie</span>
            </label>
          </div>
        </div>

        <div class="form-actions">
          <button class="cancel-btn" onclick="this.closest('add-movie-page').clearSelection()">
            Cancel
          </button>
          <button class="add-btn" onclick="this.closest('add-movie-page').handleAddMovie()">
            Add Movie
          </button>
        </div>
      </div>
    `;
  }

  handleSearchInput(value: string): void {
    this.searchTerm.set(value);
  }

  async handleSearch(): Promise<void> {
    const term = this.searchTerm.value.trim();
    if (!term) return;

    this.isSearching.set(true);
    this.selectedMovie.set(null);

    try {
      const results = await http.get<MovieLookupResult[]>('/movie/lookup', {
        params: { term },
      });
      this.searchResults.set(results);
    } catch {
      showError('Search failed');
      this.searchResults.set([]);
    } finally {
      this.isSearching.set(false);
    }
  }

  selectMovie(index: number): void {
    const movie = this.searchResults.value[index];
    if (movie) {
      this.selectedMovie.set(movie);
    }
  }

  clearSelection(): void {
    this.selectedMovie.set(null);
  }

  handleAddMovie(): void {
    const movie = this.selectedMovie.value;
    if (!movie) return;

    const form = this.querySelector('.add-form');
    const rootFolderEl = form?.querySelector('#rootFolder') as HTMLSelectElement | null;
    const qualityProfileEl = form?.querySelector('#qualityProfile') as HTMLSelectElement | null;
    const monitoredEl = form?.querySelector('#monitored') as HTMLInputElement | null;
    const searchOnAddEl = form?.querySelector('#searchOnAdd') as HTMLInputElement | null;

    this.addMovieMutation.mutate({
      title: movie.title,
      imdbId: movie.imdbId,
      qualityProfileId: qualityProfileEl ? parseInt(qualityProfileEl.value, 10) : 0,
      rootFolderPath: rootFolderEl?.value ?? '',
      monitored: monitoredEl?.checked ?? true,
      addOptions: {
        searchForMovie: searchOnAddEl?.checked ?? true,
      },
    } as Partial<Movie>);
  }

  navigateToImport(): void {
    navigate('/add/movies/import');
  }
}
