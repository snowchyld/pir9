/**
 * Movie match dialog - search IMDB for movies and re-match to a different entry.
 * Used when a movie was matched to the wrong IMDB entry.
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http, type Movie, type MovieLookupResult } from '../../core/http';
import { createMutation, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

@customElement('movie-match-dialog')
export class MovieMatchDialog extends BaseComponent {
  private isOpen = signal(false);
  private movieId = signal<number | null>(null);
  private movieTitle = signal('');
  private currentImdbId = signal<string | null>(null);
  private searchTerm = signal('');
  private searchResults = signal<MovieLookupResult[]>([]);
  private isSearching = signal(false);

  private rematchMutation = createMutation({
    mutationFn: (params: { movieId: number; imdbId: string }) =>
      http.post<Movie>(`/movie/${params.movieId}/rematch`, {
        imdbId: params.imdbId,
      }),
    onSuccess: () => {
      const id = this.movieId.value;
      if (id) {
        invalidateQueries(['/movie', id]);
        invalidateQueries(['/movie']);
      }
      showSuccess('Movie re-matched — metadata refreshed');
      this.close();
    },
    onError: () => {
      showError('Failed to re-match movie');
    },
  });

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.searchTerm);
    this.watch(this.searchResults);
    this.watch(this.isSearching);
    this.watch(this.currentImdbId);
    this.watch(this.rematchMutation.isLoading);
  }

  open(movieId: number, movieTitle: string, imdbId: string | null): void {
    this.movieId.set(movieId);
    this.movieTitle.set(movieTitle);
    this.currentImdbId.set(imdbId);
    this.searchTerm.set(movieTitle);
    this.searchResults.set([]);
    this.isOpen.set(true);
    this.doSearch();
  }

  close(): void {
    this.isOpen.set(false);
    this.searchResults.set([]);
    this.searchTerm.set('');
    this.currentImdbId.set(null);
  }

  handleSearchInput(value: string): void {
    this.searchTerm.set(value);
  }

  async doSearch(): Promise<void> {
    const term = this.searchTerm.value.trim();
    if (!term) return;

    this.isSearching.set(true);
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

  selectMatch(imdbId: string): void {
    const id = this.movieId.value;
    if (!id) return;

    this.rematchMutation.mutate({ movieId: id, imdbId });
  }

  private getPoster(result: MovieLookupResult): string | null {
    return result.images?.find((i) => i.coverType === 'poster')?.url ?? null;
  }

  protected template(): string {
    if (!this.isOpen.value) return '';

    const results = this.searchResults.value;
    const searching = this.isSearching.value;
    const currentImdb = this.currentImdbId.value;
    const title = this.movieTitle.value;
    const term = this.searchTerm.value;
    const isRematching = this.rematchMutation.isLoading.value;

    return html`
      <div class="match-backdrop" onclick="if(event.target===this)this.querySelector('movie-match-dialog')?.close()">
        <div class="match-dialog" role="dialog" aria-modal="true">
          <div class="match-header">
            <h2>Fix Match - ${escapeHtml(title)}</h2>
            <button class="close-btn" onclick="this.closest('movie-match-dialog').close()" aria-label="Close">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="match-body">
            ${
              currentImdb
                ? html`
              <div class="current-match-info">
                <span class="current-label">Current match:</span>
                <span class="current-value">IMDB: ${escapeHtml(currentImdb)}</span>
              </div>
            `
                : html`
              <div class="current-match-info">
                <span class="current-label">No IMDB match</span>
              </div>
            `
            }

            <div class="search-bar">
              <input
                type="text"
                class="search-input"
                value="${escapeHtml(term)}"
                placeholder="Search for movie..."
                oninput="this.closest('movie-match-dialog').handleSearchInput(this.value)"
                onkeydown="if(event.key==='Enter')this.closest('movie-match-dialog').doSearch()"
              />
              <button
                class="search-btn"
                onclick="this.closest('movie-match-dialog').doSearch()"
                ${searching ? 'disabled' : ''}
              >
                ${searching ? 'Searching...' : 'Search'}
              </button>
            </div>

            <div class="results-area">
              ${
                searching
                  ? html`
                <div class="results-loading">
                  <div class="loading-spinner"></div>
                  <span>Searching IMDB...</span>
                </div>
              `
                  : results.length === 0
                    ? html`
                <div class="results-empty">
                  <p>No results. Try a different search term.</p>
                </div>
              `
                    : html`
                <div class="results-list">
                  ${results
                    .map((r) => {
                      const poster = this.getPoster(r);
                      const isCurrent = currentImdb === r.imdbId;
                      return html`
                      <div class="result-card ${isCurrent ? 'current' : ''}">
                        <div class="result-poster">
                          ${
                            poster
                              ? html`<img src="${poster}" alt="${escapeHtml(r.title)}" loading="lazy" />`
                              : html`<div class="poster-placeholder-sm">No Poster</div>`
                          }
                        </div>
                        <div class="result-info">
                          <div class="result-title-row">
                            <span class="result-title">${escapeHtml(r.title)}</span>
                            <span class="result-year">(${r.year})</span>
                            ${isCurrent ? html`<span class="current-badge">Current Match</span>` : ''}
                          </div>
                          <div class="result-meta">
                            ${r.runtime ? html`<span>${r.runtime} min</span>` : ''}
                            ${r.ratings?.value ? html`<span>Rating: ${r.ratings.value.toFixed(1)}</span>` : ''}
                            ${r.imdbId ? html`<span class="result-imdb">${escapeHtml(r.imdbId)}</span>` : ''}
                          </div>
                          ${r.genres?.length ? html`<div class="result-genres">${r.genres.slice(0, 4).join(', ')}</div>` : ''}
                        </div>
                        <div class="result-action">
                          <button
                            class="select-btn ${isCurrent ? 'current' : 'primary'}"
                            onclick="this.closest('movie-match-dialog').selectMatch('${r.imdbId ?? ''}')"
                            ${isRematching || isCurrent || !r.imdbId ? 'disabled' : ''}
                          >
                            ${isCurrent ? 'Matched' : isRematching ? 'Matching...' : 'Select'}
                          </button>
                        </div>
                      </div>
                    `;
                    })
                    .join('')}
                </div>
              `
              }
            </div>
          </div>
        </div>
      </div>

      <style>
        .match-backdrop {
          position: fixed; inset: 0; z-index: 1000;
          display: flex; align-items: center; justify-content: center;
          background-color: rgba(0, 0, 0, 0.6);
        }
        .match-dialog {
          width: min(900px, 95vw); max-height: 85vh;
          display: flex; flex-direction: column;
          background-color: var(--bg-card); border: 1px solid var(--border-color);
          border-radius: 0.5rem; box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
        }
        .match-header {
          display: flex; align-items: center; justify-content: space-between;
          padding: 1rem 1.25rem; border-bottom: 1px solid var(--border-color);
        }
        .match-header h2 { margin: 0; font-size: 1.125rem; font-weight: 600; }
        .close-btn {
          display: flex; align-items: center; justify-content: center;
          padding: 0.25rem; background: transparent; border: none;
          border-radius: 0.25rem; color: var(--text-color-muted); cursor: pointer;
        }
        .close-btn:hover { color: var(--text-color); background-color: var(--bg-input-hover); }
        .match-body {
          flex: 1; overflow-y: auto; padding: 1rem 1.25rem;
          display: flex; flex-direction: column; gap: 1rem;
        }
        .current-match-info {
          display: flex; flex-wrap: wrap; align-items: center; gap: 0.75rem;
          padding: 0.625rem 0.875rem; background-color: var(--bg-card-alt);
          border: 1px solid var(--border-color); border-radius: 0.375rem; font-size: 0.8125rem;
        }
        .current-label { font-weight: 600; color: var(--text-color-muted); }
        .current-value {
          color: var(--text-color-muted); font-family: monospace; font-size: 0.75rem;
          padding: 0.125rem 0.375rem; background-color: var(--bg-card); border-radius: 0.25rem;
        }
        .search-bar { display: flex; gap: 0.5rem; }
        .search-input {
          flex: 1; padding: 0.5rem 0.75rem; background-color: var(--bg-input);
          border: 1px solid var(--border-color); border-radius: 0.25rem;
          color: var(--text-color); font-size: 0.875rem;
        }
        .search-input:focus { outline: none; border-color: var(--color-primary); }
        .search-btn {
          padding: 0.5rem 1rem; background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border); border-radius: 0.25rem;
          color: var(--color-white); font-size: 0.875rem; cursor: pointer; white-space: nowrap;
        }
        .search-btn:hover:not(:disabled) { background-color: var(--btn-primary-bg-hover); }
        .search-btn:disabled { opacity: 0.5; cursor: not-allowed; }
        .results-area { flex: 1; min-height: 200px; }
        .results-loading {
          display: flex; flex-direction: column; align-items: center; gap: 1rem;
          padding: 3rem; color: var(--text-color-muted);
        }
        .loading-spinner {
          width: 32px; height: 32px; border: 3px solid var(--border-color);
          border-top-color: var(--color-primary); border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }
        @keyframes spin { to { transform: rotate(360deg); } }
        .results-empty { display: flex; justify-content: center; padding: 2rem; color: var(--text-color-muted); }
        .results-empty p { margin: 0; }
        .results-list { display: flex; flex-direction: column; gap: 0.5rem; }
        .result-card {
          display: flex; gap: 0.75rem; padding: 0.75rem;
          border: 1px solid var(--border-color); border-radius: 0.375rem;
          transition: border-color 0.15s;
        }
        .result-card.current {
          border-color: var(--color-primary);
          background-color: color-mix(in srgb, var(--color-primary) 5%, transparent);
        }
        .result-card:hover:not(.current) { border-color: var(--text-color-muted); }
        .result-poster { flex-shrink: 0; width: 60px; }
        .result-poster img { width: 100%; border-radius: 0.25rem; }
        .poster-placeholder-sm {
          display: flex; align-items: center; justify-content: center;
          height: 90px; background-color: var(--bg-card-alt); border-radius: 0.25rem;
          color: var(--text-color-muted); font-size: 0.625rem; text-align: center;
        }
        .result-info { flex: 1; min-width: 0; }
        .result-title-row { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; margin-bottom: 0.25rem; }
        .result-title { font-weight: 600; font-size: 0.9375rem; }
        .result-year { color: var(--text-color-muted); font-size: 0.875rem; }
        .current-badge {
          padding: 0.0625rem 0.375rem; background-color: var(--color-primary);
          color: var(--color-white); border-radius: 0.25rem; font-size: 0.6875rem; font-weight: 600;
        }
        .result-meta {
          display: flex; flex-wrap: wrap; gap: 0.5rem; margin-bottom: 0.375rem;
          font-size: 0.75rem; color: var(--text-color-muted);
        }
        .result-imdb {
          font-family: monospace; font-size: 0.6875rem; padding: 0 0.25rem;
          background-color: var(--bg-card-alt); border-radius: 0.125rem;
        }
        .result-genres { font-size: 0.75rem; color: var(--text-color-muted); }
        .result-action { display: flex; align-items: center; flex-shrink: 0; }
        .select-btn {
          padding: 0.375rem 0.875rem; border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem; font-size: 0.8125rem; cursor: pointer; white-space: nowrap;
        }
        .select-btn.primary {
          background-color: var(--btn-primary-bg); border-color: var(--btn-primary-border);
          color: var(--color-white);
        }
        .select-btn.primary:hover:not(:disabled) { background-color: var(--btn-primary-bg-hover); }
        .select-btn.current { background-color: var(--bg-card-alt); color: var(--text-color-muted); }
        .select-btn:disabled { opacity: 0.5; cursor: not-allowed; }
      </style>
    `;
  }
}
