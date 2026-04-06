/**
 * Add Audiobook page - search and add audiobooks
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { type Audiobook, type AudiobookLookupResult, http } from '../../core/http';
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

@customElement('add-audiobook-page')
export class AddAudiobookPage extends BaseComponent {
  private searchTerm = signal('');
  private lookupResults = signal<AudiobookLookupResult[]>([]);
  private selectedResult = signal<AudiobookLookupResult | null>(null);
  private isSearching = signal(false);
  private hasSearched = signal(false);

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder', 'audiobook'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder?contentType=audiobook'),
  });

  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => http.get<QualityProfile[]>('/qualityprofile'),
  });

  private addAudiobookMutation = createMutation({
    mutationFn: (audiobook: Partial<Audiobook>) => http.post<Audiobook>('/audiobook', audiobook),
    onSuccess: (data) => {
      invalidateQueries(['/audiobook']);
      showSuccess('Audiobook added successfully');
      navigate(`/audiobooks/${data.titleSlug}`);
    },
    onError: () => {
      showError('Failed to add audiobook');
    },
  });

  protected onInit(): void {
    this.watch(this.searchTerm);
    this.watch(this.lookupResults);
    this.watch(this.selectedResult);
    this.watch(this.isSearching);
    this.watch(this.hasSearched);
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.qualityProfilesQuery.data);
  }

  protected template(): string {
    const results = this.lookupResults.value;
    const selected = this.selectedResult.value;
    const isSearching = this.isSearching.value;
    const hasSearched = this.hasSearched.value;
    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const qualityProfiles = this.qualityProfilesQuery.data.value ?? [];
    const noRootFolders = rootFolders.length === 0;

    return html`
      <div class="add-audiobook-page">
        <h1 class="page-title">Add New Audiobook</h1>

        ${
          noRootFolders
            ? html`
          <div class="warning-banner">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path>
              <line x1="12" y1="9" x2="12" y2="13"></line>
              <line x1="12" y1="17" x2="12.01" y2="17"></line>
            </svg>
            <span>No audiobook root folders configured. Add a root folder with content type "audiobook" in Settings &gt; Media Management.</span>
          </div>
        `
            : ''
        }

        <div class="search-section">
          <div class="search-box">
            <svg class="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M4 11a7 7 0 0 1 14 0 7 7 0 0 1-14 0z"></path>
              <path d="M21 21l-4.35-4.35"></path>
            </svg>
            <input
              type="text"
              class="search-input"
              placeholder="Search by title, author, or ISBN..."
              value="${escapeHtml(this.searchTerm.value)}"
              oninput="this.closest('add-audiobook-page').handleSearchInput(this.value)"
              onkeydown="if(event.key === 'Enter') this.closest('add-audiobook-page').handleSearch()"
            />
            <button
              class="search-btn"
              onclick="this.closest('add-audiobook-page').handleSearch()"
              ${isSearching ? 'disabled' : ''}
            >
              ${isSearching ? 'Searching...' : 'Search'}
            </button>
          </div>
        </div>

        ${selected ? this.renderSelectedPreview(selected, rootFolders, qualityProfiles) : ''}

        ${!selected && hasSearched && results.length > 0 ? this.renderSearchResults(results) : ''}

        ${
          !selected && hasSearched && results.length === 0 && !isSearching
            ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"></path>
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"></path>
            </svg>
            <p>No audiobooks found</p>
            <p class="hint">Try a different search term</p>
          </div>
        `
            : ''
        }
      </div>

      <style>
        .add-audiobook-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .warning-banner {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 1rem 1.25rem;
          background: rgba(220, 53, 69, 0.1);
          border: 1px solid rgba(220, 53, 69, 0.3);
          border-radius: 0.5rem;
          color: var(--color-danger);
          font-size: 0.875rem;
        }

        .search-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .search-box { display: flex; align-items: center; gap: 0.75rem; }
        .search-icon { color: var(--text-color-muted); flex-shrink: 0; }

        .search-input {
          flex: 1; padding: 0.75rem; font-size: 1rem;
          background-color: var(--bg-input); border: 1px solid var(--border-input);
          border-radius: 0.25rem; color: var(--text-color);
        }
        .search-input:focus { outline: none; border-color: var(--color-primary); }

        .search-btn {
          padding: 0.75rem 1.5rem;
          background-color: var(--btn-primary-bg); border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem; color: var(--color-white);
          font-size: 0.875rem; font-weight: 500; cursor: pointer; white-space: nowrap;
        }
        .search-btn:hover:not(:disabled) { background-color: var(--btn-primary-bg-hover); }
        .search-btn:disabled { opacity: 0.6; cursor: not-allowed; }

        .results-list { display: flex; flex-direction: column; gap: 0.75rem; }

        .result-card {
          display: flex; align-items: flex-start; gap: 1rem; padding: 1rem;
          background-color: var(--bg-card); border: 1px solid var(--border-color);
          border-radius: 0.5rem; cursor: pointer;
          transition: all var(--transition-normal);
        }
        .result-card:hover { border-color: var(--pir9-blue); background-color: var(--bg-card-hover); }

        .result-cover { width: 60px; flex-shrink: 0; }
        .result-cover img { width: 100%; border-radius: 0.25rem; }
        .result-cover-placeholder {
          width: 60px; height: 90px; display: flex; align-items: center; justify-content: center;
          background: var(--bg-card-center); border-radius: 0.25rem; color: var(--text-color-muted);
        }

        .result-info { flex: 1; }
        .result-title { font-size: 1rem; font-weight: 600; margin: 0 0 0.25rem 0; }
        .result-meta { font-size: 0.875rem; color: var(--text-color-muted); }

        .preview-card {
          padding: 1.5rem; background-color: var(--bg-card);
          border: 1px solid var(--border-color); border-radius: 0.5rem;
        }

        .preview-header {
          display: flex; align-items: flex-start; gap: 1.5rem;
          margin-bottom: 1.5rem; padding-bottom: 1.5rem;
          border-bottom: 1px solid var(--border-color);
        }

        .preview-poster { width: 120px; flex-shrink: 0; }
        .preview-poster img { width: 100%; border-radius: 0.375rem; }
        .preview-poster-placeholder {
          width: 120px; height: 180px; display: flex; align-items: center; justify-content: center;
          background: var(--bg-card-center); border-radius: 0.375rem; color: var(--text-color-muted);
        }

        .preview-info { flex: 1; }
        .preview-title { font-size: 1.25rem; font-weight: 600; margin: 0 0 0.5rem 0; }
        .preview-meta { font-size: 0.875rem; color: var(--text-color-muted); margin-bottom: 0.75rem; }
        .preview-overview { font-size: 0.875rem; color: var(--text-color-muted); line-height: 1.5; }

        .preview-genres { display: flex; gap: 0.375rem; flex-wrap: wrap; margin-top: 0.5rem; }
        .genre-tag {
          padding: 0.125rem 0.5rem; background: var(--bg-card-center);
          border: 1px solid var(--border-glass); border-radius: 9999px;
          font-size: 0.75rem; color: var(--text-color-muted);
        }

        .form-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 1.5rem; }
        .form-group { display: flex; flex-direction: column; gap: 0.5rem; }
        .form-label { font-size: 0.875rem; font-weight: 500; }

        .form-select {
          padding: 0.5rem 0.75rem; font-size: 0.875rem;
          background-color: var(--bg-input); border: 1px solid var(--border-input);
          border-radius: 0.25rem; color: var(--text-color);
        }
        .form-select:focus { outline: none; border-color: var(--color-primary); }

        .checkbox-label { display: flex; align-items: center; gap: 0.5rem; cursor: pointer; }
        .checkbox-label input[type="checkbox"] { width: 16px; height: 16px; accent-color: var(--color-primary); }

        .form-actions {
          display: flex; justify-content: flex-end; gap: 0.5rem;
          margin-top: 1.5rem; padding-top: 1.5rem; border-top: 1px solid var(--border-color);
        }

        .cancel-btn {
          padding: 0.625rem 1.25rem; background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border); border-radius: 0.25rem;
          color: var(--text-color); font-size: 0.875rem; cursor: pointer;
        }
        .cancel-btn:hover { background-color: var(--btn-default-bg-hover); }

        .add-btn {
          padding: 0.625rem 1.25rem; background-color: var(--color-success);
          border: 1px solid var(--color-success); border-radius: 0.25rem;
          color: var(--color-white); font-size: 0.875rem; font-weight: 500; cursor: pointer;
        }
        .add-btn:hover { opacity: 0.9; }

        .empty-state {
          display: flex; flex-direction: column; align-items: center; gap: 0.5rem;
          padding: 3rem; text-align: center; color: var(--text-color-muted);
        }
        .empty-state .hint { font-size: 0.875rem; }
      </style>
    `;
  }

  private renderSearchResults(results: AudiobookLookupResult[]): string {
    return html`
      <div class="results-list">
        ${results
          .map(
            (r) => html`
          <div class="result-card" onclick="this.closest('add-audiobook-page').handleSelectResult(${results.indexOf(r)})">
            <div class="result-cover">
              ${
                r.imageUrl
                  ? `<img src="${escapeHtml(r.imageUrl)}" alt="${escapeHtml(r.title)}" />`
                  : `<div class="result-cover-placeholder">
                    <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"></path>
                      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"></path>
                    </svg>
                  </div>`
              }
            </div>
            <div class="result-info">
              <h3 class="result-title">${escapeHtml(r.title)}</h3>
              <div class="result-meta">
                ${r.author ? `by ${escapeHtml(r.author)}` : ''}
                ${r.publisher ? ` - ${escapeHtml(r.publisher)}` : ''}
              </div>
            </div>
          </div>
        `,
          )
          .join('')}
      </div>
    `;
  }

  private renderSelectedPreview(
    result: AudiobookLookupResult,
    rootFolders: RootFolder[],
    profiles: QualityProfile[],
  ): string {
    return html`
      <div class="preview-card">
        <div class="preview-header">
          <div class="preview-poster">
            ${
              result.imageUrl
                ? html`<img src="${escapeHtml(result.imageUrl)}" alt="${escapeHtml(result.title)}" />`
                : html`<div class="preview-poster-placeholder">
                  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"></path>
                    <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"></path>
                  </svg>
                </div>`
            }
          </div>
          <div class="preview-info">
            <h2 class="preview-title">${escapeHtml(result.title)}</h2>
            <div class="preview-meta">
              ${result.author ? `by ${escapeHtml(result.author)}` : ''}
              ${result.publisher ? ` - ${escapeHtml(result.publisher)}` : ''}
            </div>
            <div class="preview-overview">${escapeHtml(result.overview ?? '')}</div>
            ${
              result.genres.length > 0
                ? `
              <div class="preview-genres">
                ${result.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
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
              <span>Monitor this audiobook</span>
            </label>
          </div>
        </div>

        <div class="form-actions">
          <button class="cancel-btn" onclick="this.closest('add-audiobook-page').handleReset()">
            Cancel
          </button>
          <button class="add-btn" onclick="this.closest('add-audiobook-page').handleAddAudiobook()">
            Add Audiobook
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
    this.hasSearched.set(false);
    this.lookupResults.set([]);
    this.selectedResult.set(null);

    try {
      const results = await http.get<AudiobookLookupResult[]>('/audiobook/lookup', {
        params: { term },
      });
      this.lookupResults.set(results ?? []);
    } catch {
      showError('Failed to search for audiobooks');
    } finally {
      this.isSearching.set(false);
      this.hasSearched.set(true);
    }
  }

  handleSelectResult(index: number): void {
    const results = this.lookupResults.value;
    if (index >= 0 && index < results.length) {
      this.selectedResult.set(results[index]);
    }
  }

  handleReset(): void {
    this.selectedResult.set(null);
  }

  handleAddAudiobook(): void {
    const result = this.selectedResult.value;
    if (!result) return;

    const card = this.querySelector('.preview-card');
    const rootFolderEl = card?.querySelector('#rootFolder') as HTMLSelectElement | null;
    const qualityProfileEl = card?.querySelector('#qualityProfile') as HTMLSelectElement | null;
    const monitoredEl = card?.querySelector('#monitored') as HTMLInputElement | null;

    this.addAudiobookMutation.mutate({
      title: result.title,
      author: result.author,
      narrator: result.narrator,
      overview: result.overview,
      publisher: result.publisher,
      isbn: result.isbn,
      genres: result.genres,
      imageUrl: result.imageUrl,
      qualityProfileId: qualityProfileEl ? Number.parseInt(qualityProfileEl.value, 10) : 0,
      rootFolderPath: rootFolderEl?.value ?? '',
      monitored: monitoredEl?.checked ?? true,
    } as Partial<Audiobook>);
  }
}
