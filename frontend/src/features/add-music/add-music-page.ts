/**
 * Add Music page - search and add new artists
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { type Artist, type ArtistLookupResult, http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

interface RootFolder {
  id: number;
  path: string;
  freeSpace: number;
  contentType: string;
}

interface QualityProfile {
  id: number;
  name: string;
}

@customElement('add-music-page')
export class AddMusicPage extends BaseComponent {
  private searchTerm = signal('');
  private searchResults = signal<ArtistLookupResult[]>([]);
  private isSearching = signal(false);
  private selectedArtist = signal<ArtistLookupResult | null>(null);

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder', 'music'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder?contentType=music'),
  });

  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => http.get<QualityProfile[]>('/qualityprofile'),
  });

  private addArtistMutation = createMutation({
    mutationFn: (artist: Partial<Artist>) => http.post<Artist>('/artist', artist),
    onSuccess: (data) => {
      invalidateQueries(['/artist']);
      showSuccess('Artist added successfully');
      navigate(`/music/${data.titleSlug}`);
    },
    onError: () => {
      showError('Failed to add artist');
    },
  });

  protected onInit(): void {
    this.watch(this.searchTerm);
    this.watch(this.searchResults);
    this.watch(this.isSearching);
    this.watch(this.selectedArtist);
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.qualityProfilesQuery.data);
  }

  protected template(): string {
    const results = this.searchResults.value;
    const isSearching = this.isSearching.value;
    const selected = this.selectedArtist.value;
    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const qualityProfiles = this.qualityProfilesQuery.data.value ?? [];

    return html`
      <div class="add-music-page">
        <h1 class="page-title">Add New Artist</h1>

        <div class="search-section">
          <div class="search-box">
            <svg class="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            <input
              type="text"
              class="search-input"
              placeholder="Search for an artist..."
              value="${escapeHtml(this.searchTerm.value)}"
              oninput="this.closest('add-music-page').handleSearchInput(this.value)"
              onkeydown="if(event.key === 'Enter') this.closest('add-music-page').handleSearch()"
            />
            <button
              class="search-btn"
              onclick="this.closest('add-music-page').handleSearch()"
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
            <p>No artists found</p>
            <p class="hint">Try a different search term</p>
          </div>
        `
            : ''
        }
      </div>

      <style>
        .add-music-page {
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
          height: 80px;
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
          width: 120px;
          flex-shrink: 0;
        }

        .form-poster img {
          width: 100%;
          border-radius: 0.375rem;
        }

        .form-artist-info {
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
      </style>
    `;
  }

  private renderSearchResult(result: ArtistLookupResult, index: number): string {
    const posterImage = result.images?.find((i) => i.coverType === 'poster');
    const meta: string[] = [];
    if (result.artistType) meta.push(result.artistType);
    if (result.area) meta.push(result.area);
    if (result.beginDate) {
      const years = result.endDate
        ? `${result.beginDate.substring(0, 4)}–${result.endDate.substring(0, 4)}`
        : `${result.beginDate.substring(0, 4)}–present`;
      meta.push(years);
    }
    if (result.genres?.length > 0) meta.push(result.genres.slice(0, 3).join(', '));

    return html`
      <div class="result-card" onclick="this.closest('add-music-page').selectArtist(${index})">
        <div class="result-poster">
          ${
            posterImage?.remoteUrl
              ? html`
            <img src="${escapeHtml(posterImage.remoteUrl)}" alt="${escapeHtml(result.name)}" loading="lazy" />
          `
              : html`
            <div class="result-poster-placeholder">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M9 18V5l12-2v13"></path>
              </svg>
            </div>
          `
          }
        </div>
        <div class="result-info">
          <div class="result-title">${escapeHtml(result.name)}</div>
          ${result.disambiguation ? html`<div class="result-disambiguation" style="color: var(--text-muted); font-size: 0.85em;">${escapeHtml(result.disambiguation)}</div>` : ''}
          <div class="result-meta">${escapeHtml(meta.join(' · '))}</div>
          ${result.rating ? html`<div class="result-rating" style="color: var(--color-primary); font-size: 0.85em;">★ ${result.rating.toFixed(1)}</div>` : ''}
        </div>
      </div>
    `;
  }

  private renderAddForm(
    artist: ArtistLookupResult,
    rootFolders: RootFolder[],
    profiles: QualityProfile[],
  ): string {
    const posterImage = artist.images?.find((i) => i.coverType === 'poster');

    return html`
      <div class="add-form">
        <button class="back-btn" onclick="this.closest('add-music-page').clearSelection()">
          &larr; Back to results
        </button>

        <div class="form-header">
          <div class="form-poster">
            ${
              posterImage?.remoteUrl
                ? html`
              <img src="${escapeHtml(posterImage.remoteUrl)}" alt="${escapeHtml(artist.name)}" />
            `
                : ''
            }
          </div>
          <div class="form-artist-info">
            <h2 class="form-title">${escapeHtml(artist.name)}</h2>
            ${artist.disambiguation ? html`<div class="form-disambiguation" style="color: var(--text-muted);">${escapeHtml(artist.disambiguation)}</div>` : ''}
            <div class="form-meta">${escapeHtml(artist.artistType)}${artist.area ? ` · ${escapeHtml(artist.area)}` : ''}${artist.beginDate ? ` · ${artist.beginDate.substring(0, 4)}` : ''}</div>
            <div class="form-overview">${escapeHtml(artist.overview ?? '')}</div>
            ${
              artist.genres.length > 0
                ? `
              <div class="form-genres">
                ${artist.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
              </div>
            `
                : ''
            }
          </div>
        </div>

        <div class="form-grid">
          <div class="form-group">
            <label class="form-label">Root Folder</label>
            ${
              rootFolders.length > 0
                ? html`
              <select class="form-select" id="rootFolder">
                ${rootFolders
                  .map(
                    (folder) => html`
                  <option value="${escapeHtml(folder.path)}">${escapeHtml(folder.path)}</option>
                `,
                  )
                  .join('')}
              </select>
            `
                : html`
              <div style="padding: 0.75rem; background: var(--bg-warning, #332b00); border-radius: 4px; color: var(--text-warning, #ffa500); font-size: 0.9em;">
                No music root folders configured. Add one in <a href="/settings/mediamanagement" style="color: var(--color-primary);">Settings &rarr; Media Management</a> with content type "music".
              </div>
              <select class="form-select" id="rootFolder" style="display:none;"></select>
            `
            }
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
              <span>Monitor this artist</span>
            </label>
          </div>

          <div class="form-group">
            <label class="form-label">Search on Add</label>
            <label class="checkbox-label">
              <input type="checkbox" id="searchOnAdd" checked />
              <span>Start search for albums</span>
            </label>
          </div>
        </div>

        <div class="form-actions">
          <button class="cancel-btn" onclick="this.closest('add-music-page').clearSelection()">
            Cancel
          </button>
          <button class="add-btn" onclick="this.closest('add-music-page').handleAddArtist()">
            Add Artist
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
    this.selectedArtist.set(null);

    try {
      const results = await http.get<ArtistLookupResult[]>('/artist/lookup', {
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

  selectArtist(index: number): void {
    const artist = this.searchResults.value[index];
    if (artist) {
      this.selectedArtist.set(artist);
    }
  }

  clearSelection(): void {
    this.selectedArtist.set(null);
  }

  handleAddArtist(): void {
    const artist = this.selectedArtist.value;
    if (!artist) return;

    const form = this.querySelector('.add-form');
    const rootFolderEl = form?.querySelector('#rootFolder') as HTMLSelectElement | null;
    const qualityProfileEl = form?.querySelector('#qualityProfile') as HTMLSelectElement | null;
    const monitoredEl = form?.querySelector('#monitored') as HTMLInputElement | null;
    const searchOnAddEl = form?.querySelector('#searchOnAdd') as HTMLInputElement | null;

    this.addArtistMutation.mutate({
      name: artist.name,
      musicbrainzId: artist.musicbrainzId,
      qualityProfileId: qualityProfileEl ? Number.parseInt(qualityProfileEl.value, 10) : 0,
      rootFolderPath: rootFolderEl?.value ?? '',
      monitored: monitoredEl?.checked ?? true,
      overview: artist.overview,
      artistType: artist.artistType,
      genres: artist.genres,
      images: artist.images,
      addOptions: {
        searchForAlbums: searchOnAddEl?.checked ?? true,
      },
    } as Partial<Artist>);
  }
}
