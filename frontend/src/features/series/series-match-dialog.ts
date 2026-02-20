/**
 * Series match dialog - search for and re-match a series to the correct TVDB/IMDB entry.
 * Used when a series was auto-matched to the wrong show (e.g., "Revenge (2017)" instead of "Revenge (2011)").
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http, type Series } from '../../core/http';
import { createMutation, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

interface LookupResult {
  tvdbId: number;
  title: string;
  overview: string | null;
  network: string | null;
  year: number;
  status: string;
  imdbId: string | null;
  images: Array<{ coverType: string; url: string }>;
}

interface CurrentMatchInfo {
  tvdbId: number;
  imdbId: string | null;
  title: string;
  year: number;
}

@customElement('series-match-dialog')
export class SeriesMatchDialog extends BaseComponent {
  private isOpen = signal(false);
  private seriesId = signal<number | null>(null);
  private seriesTitle = signal('');
  private searchTerm = signal('');
  private searchResults = signal<LookupResult[]>([]);
  private isSearching = signal(false);
  private currentMatch = signal<CurrentMatchInfo | null>(null);

  private rematchMutation = createMutation({
    mutationFn: (params: { seriesId: number; tvdbId: number; imdbId?: string }) =>
      http.post<Series>(`/series/${params.seriesId}/rematch`, {
        tvdbId: params.tvdbId,
        imdbId: params.imdbId ?? null,
      }),
    onSuccess: (updated: Series) => {
      const id = this.seriesId.value;
      if (id) {
        invalidateQueries(['/series', id]);
        invalidateQueries(['/episode', id]);
        invalidateQueries(['/series']);
      }
      showSuccess('Series re-matched — metadata refreshed');
      this.close();
      // Navigate to the new slug since the title/year likely changed
      if (updated?.titleSlug) {
        navigate(`/series/${updated.titleSlug}`, { replace: true });
      }
    },
    onError: () => {
      showError('Failed to re-match series');
    },
  });

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.searchTerm);
    this.watch(this.searchResults);
    this.watch(this.isSearching);
    this.watch(this.currentMatch);
    this.watch(this.rematchMutation.isLoading);
  }

  open(
    seriesId: number,
    seriesTitle: string,
    tvdbId: number,
    imdbId: string | null,
    year: number,
  ): void {
    this.seriesId.set(seriesId);
    this.seriesTitle.set(seriesTitle);
    this.searchTerm.set(seriesTitle);
    this.currentMatch.set({ tvdbId, imdbId, title: seriesTitle, year });
    this.searchResults.set([]);
    this.isOpen.set(true);
    // Auto-search with current title
    this.doSearch();
  }

  close(): void {
    this.isOpen.set(false);
    this.searchResults.set([]);
    this.searchTerm.set('');
    this.currentMatch.set(null);
  }

  handleSearchInput(value: string): void {
    this.searchTerm.set(value);
  }

  async doSearch(): Promise<void> {
    const term = this.searchTerm.value.trim();
    if (!term) return;

    this.isSearching.set(true);
    try {
      const results = await http.get<LookupResult[]>('/series/lookup', {
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

  selectMatch(tvdbId: number): void {
    const id = this.seriesId.value;
    if (!id) return;

    const result = this.searchResults.value.find((r) => r.tvdbId === tvdbId);
    if (!result) return;

    this.rematchMutation.mutate({
      seriesId: id,
      tvdbId: result.tvdbId,
      imdbId: result.imdbId ?? undefined,
    });
  }

  private getPoster(result: LookupResult): string | null {
    return result.images?.find((i) => i.coverType === 'poster')?.url ?? null;
  }

  protected template(): string {
    if (!this.isOpen.value) return '';

    const results = this.searchResults.value;
    const searching = this.isSearching.value;
    const current = this.currentMatch.value;
    const title = this.seriesTitle.value;
    const term = this.searchTerm.value;
    const isRematching = this.rematchMutation.isLoading.value;

    return html`
      <div class="match-backdrop" onclick="if(event.target===this)this.querySelector('series-match-dialog')?.close()">
        <div class="match-dialog" role="dialog" aria-modal="true">
          <div class="match-header">
            <h2>Fix Match - ${escapeHtml(title)}</h2>
            <button class="close-btn" onclick="this.closest('series-match-dialog').close()" aria-label="Close">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="match-body">
            ${
              current
                ? html`
              <div class="current-match-info">
                <span class="current-label">Current match:</span>
                <span class="current-value">TVDB: ${current.tvdbId}</span>
                ${current.imdbId ? html`<span class="current-value">IMDB: ${escapeHtml(current.imdbId)}</span>` : ''}
                <span class="current-value">Year: ${current.year}</span>
              </div>
            `
                : ''
            }

            <div class="search-bar">
              <input
                type="text"
                class="search-input"
                value="${escapeHtml(term)}"
                placeholder="Search for series..."
                oninput="this.closest('series-match-dialog').handleSearchInput(this.value)"
                onkeydown="if(event.key==='Enter')this.closest('series-match-dialog').doSearch()"
              />
              <button
                class="search-btn"
                onclick="this.closest('series-match-dialog').doSearch()"
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
                  <span>Searching...</span>
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
                      const isCurrent = current?.tvdbId === r.tvdbId;
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
                            ${r.network ? html`<span class="result-network">${escapeHtml(r.network)}</span>` : ''}
                            <span class="result-status">${escapeHtml(r.status)}</span>
                            <span class="result-tvdb">TVDB: ${r.tvdbId}</span>
                            ${r.imdbId ? html`<span class="result-imdb">${escapeHtml(r.imdbId)}</span>` : ''}
                          </div>
                          ${r.overview ? html`<p class="result-overview">${escapeHtml(r.overview.substring(0, 200))}${r.overview.length > 200 ? '...' : ''}</p>` : ''}
                        </div>
                        <div class="result-action">
                          <button
                            class="select-btn ${isCurrent ? 'current' : 'primary'}"
                            onclick="this.closest('series-match-dialog').selectMatch(${r.tvdbId})"
                            ${isRematching || isCurrent ? 'disabled' : ''}
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
          position: fixed;
          inset: 0;
          z-index: 1000;
          display: flex;
          align-items: center;
          justify-content: center;
          background-color: rgba(0, 0, 0, 0.6);
        }

        .match-dialog {
          width: min(900px, 95vw);
          max-height: 85vh;
          display: flex;
          flex-direction: column;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
        }

        .match-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem 1.25rem;
          border-bottom: 1px solid var(--border-color);
        }

        .match-header h2 {
          margin: 0;
          font-size: 1.125rem;
          font-weight: 600;
        }

        .close-btn {
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

        .close-btn:hover {
          color: var(--text-color);
          background-color: var(--bg-input-hover);
        }

        .match-body {
          flex: 1;
          overflow-y: auto;
          padding: 1rem 1.25rem;
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .current-match-info {
          display: flex;
          flex-wrap: wrap;
          align-items: center;
          gap: 0.75rem;
          padding: 0.625rem 0.875rem;
          background-color: var(--bg-card-alt);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          font-size: 0.8125rem;
        }

        .current-label {
          font-weight: 600;
          color: var(--text-color-muted);
        }

        .current-value {
          color: var(--text-color-muted);
          font-family: monospace;
          font-size: 0.75rem;
          padding: 0.125rem 0.375rem;
          background-color: var(--bg-card);
          border-radius: 0.25rem;
        }

        .search-bar {
          display: flex;
          gap: 0.5rem;
        }

        .search-input {
          flex: 1;
          padding: 0.5rem 0.75rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
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
          opacity: 0.5;
          cursor: not-allowed;
        }

        .results-area {
          flex: 1;
          min-height: 200px;
        }

        .results-loading {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1rem;
          padding: 3rem;
          color: var(--text-color-muted);
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

        .results-empty {
          display: flex;
          justify-content: center;
          padding: 2rem;
          color: var(--text-color-muted);
        }

        .results-empty p {
          margin: 0;
        }

        .results-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .result-card {
          display: flex;
          gap: 0.75rem;
          padding: 0.75rem;
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          transition: border-color 0.15s;
        }

        .result-card.current {
          border-color: var(--color-primary);
          background-color: color-mix(in srgb, var(--color-primary) 5%, transparent);
        }

        .result-card:hover:not(.current) {
          border-color: var(--text-color-muted);
        }

        .result-poster {
          flex-shrink: 0;
          width: 60px;
        }

        .result-poster img {
          width: 100%;
          border-radius: 0.25rem;
        }

        .poster-placeholder-sm {
          display: flex;
          align-items: center;
          justify-content: center;
          height: 90px;
          background-color: var(--bg-card-alt);
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          font-size: 0.625rem;
          text-align: center;
        }

        .result-info {
          flex: 1;
          min-width: 0;
        }

        .result-title-row {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          flex-wrap: wrap;
          margin-bottom: 0.25rem;
        }

        .result-title {
          font-weight: 600;
          font-size: 0.9375rem;
        }

        .result-year {
          color: var(--text-color-muted);
          font-size: 0.875rem;
        }

        .current-badge {
          padding: 0.0625rem 0.375rem;
          background-color: var(--color-primary);
          color: var(--color-white);
          border-radius: 0.25rem;
          font-size: 0.6875rem;
          font-weight: 600;
        }

        .result-meta {
          display: flex;
          flex-wrap: wrap;
          gap: 0.5rem;
          margin-bottom: 0.375rem;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .result-network {
          font-weight: 500;
        }

        .result-tvdb,
        .result-imdb {
          font-family: monospace;
          font-size: 0.6875rem;
          padding: 0 0.25rem;
          background-color: var(--bg-card-alt);
          border-radius: 0.125rem;
        }

        .result-overview {
          margin: 0;
          font-size: 0.75rem;
          line-height: 1.4;
          color: var(--text-color-muted);
          max-height: 40px;
          overflow: hidden;
        }

        .result-action {
          display: flex;
          align-items: center;
          flex-shrink: 0;
        }

        .select-btn {
          padding: 0.375rem 0.875rem;
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          font-size: 0.8125rem;
          cursor: pointer;
          white-space: nowrap;
        }

        .select-btn.primary {
          background-color: var(--btn-primary-bg);
          border-color: var(--btn-primary-border);
          color: var(--color-white);
        }

        .select-btn.primary:hover:not(:disabled) {
          background-color: var(--btn-primary-bg-hover);
        }

        .select-btn.current {
          background-color: var(--bg-card-alt);
          color: var(--text-color-muted);
        }

        .select-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
      </style>
    `;
  }
}
