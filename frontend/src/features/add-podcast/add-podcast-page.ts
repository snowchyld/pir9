/**
 * Add Podcast page - add by RSS feed URL
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http, type Podcast, type PodcastLookupResult } from '../../core/http';
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

@customElement('add-podcast-page')
export class AddPodcastPage extends BaseComponent {
  private feedUrl = signal('');
  private lookupResult = signal<PodcastLookupResult | null>(null);
  private isValidating = signal(false);
  private isValidated = signal(false);

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder', 'podcast'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder?contentType=podcast'),
  });

  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => http.get<QualityProfile[]>('/qualityprofile'),
  });

  private addPodcastMutation = createMutation({
    mutationFn: (podcast: Partial<Podcast>) => http.post<Podcast>('/podcast', podcast),
    onSuccess: (data) => {
      invalidateQueries(['/podcast']);
      showSuccess('Podcast added successfully');
      navigate(`/podcasts/${data.titleSlug}`);
    },
    onError: () => {
      showError('Failed to add podcast');
    },
  });

  protected onInit(): void {
    this.watch(this.feedUrl);
    this.watch(this.lookupResult);
    this.watch(this.isValidating);
    this.watch(this.isValidated);
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.qualityProfilesQuery.data);
  }

  protected template(): string {
    const result = this.lookupResult.value;
    const isValidating = this.isValidating.value;
    const isValidated = this.isValidated.value;
    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const qualityProfiles = this.qualityProfilesQuery.data.value ?? [];

    return html`
      <div class="add-podcast-page">
        <h1 class="page-title">Add New Podcast</h1>

        <div class="search-section">
          <div class="search-box">
            <svg class="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M4 11a7 7 0 0 1 14 0 7 7 0 0 1-14 0z"></path>
              <path d="M21 21l-4.35-4.35"></path>
            </svg>
            <input
              type="text"
              class="search-input"
              placeholder="Enter RSS feed URL..."
              value="${escapeHtml(this.feedUrl.value)}"
              oninput="this.closest('add-podcast-page').handleUrlInput(this.value)"
              onkeydown="if(event.key === 'Enter') this.closest('add-podcast-page').handleValidate()"
            />
            <button
              class="search-btn"
              onclick="this.closest('add-podcast-page').handleValidate()"
              ${isValidating ? 'disabled' : ''}
            >
              ${isValidating ? 'Validating...' : 'Validate'}
            </button>
          </div>
        </div>

        ${isValidated && result ? this.renderPodcastPreview(result, rootFolders, qualityProfiles) : ''}

        ${
          isValidated && !result && !isValidating
            ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
              <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>
            </svg>
            <p>Could not validate the feed URL</p>
            <p class="hint">Check the URL and try again</p>
          </div>
        `
            : ''
        }
      </div>

      <style>
        .add-podcast-page {
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

        .preview-card {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .preview-header {
          display: flex;
          align-items: flex-start;
          gap: 1.5rem;
          margin-bottom: 1.5rem;
          padding-bottom: 1.5rem;
          border-bottom: 1px solid var(--border-color);
        }

        .preview-poster {
          width: 120px;
          flex-shrink: 0;
        }

        .preview-poster img {
          width: 100%;
          border-radius: 0.375rem;
        }

        .preview-poster-placeholder {
          width: 120px;
          height: 120px;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--bg-card-center);
          border-radius: 0.375rem;
          color: var(--text-color-muted);
        }

        .preview-info {
          flex: 1;
        }

        .preview-title {
          font-size: 1.25rem;
          font-weight: 600;
          margin: 0 0 0.5rem 0;
        }

        .preview-meta {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin-bottom: 0.75rem;
        }

        .preview-overview {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          line-height: 1.5;
        }

        .preview-genres {
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

        .form-select {
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .form-select:focus {
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

  private renderPodcastPreview(
    result: PodcastLookupResult,
    rootFolders: RootFolder[],
    profiles: QualityProfile[],
  ): string {
    const posterImage = result.images?.find((i) => i.coverType === 'poster');
    const epCount = result.statistics?.episodeCount ?? 0;

    return html`
      <div class="preview-card">
        <div class="preview-header">
          <div class="preview-poster">
            ${
              posterImage?.remoteUrl
                ? html`<img src="${escapeHtml(posterImage.remoteUrl)}" alt="${escapeHtml(result.title)}" />`
                : html`<div class="preview-poster-placeholder">
                  <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                    <path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
                    <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>
                  </svg>
                </div>`
            }
          </div>
          <div class="preview-info">
            <h2 class="preview-title">${escapeHtml(result.title)}</h2>
            <div class="preview-meta">
              ${result.author ? `by ${escapeHtml(result.author)}` : ''}
              ${epCount > 0 ? ` - ${epCount} episodes` : ''}
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
              <span>Monitor this podcast</span>
            </label>
          </div>
        </div>

        <div class="form-actions">
          <button class="cancel-btn" onclick="this.closest('add-podcast-page').handleReset()">
            Cancel
          </button>
          <button class="add-btn" onclick="this.closest('add-podcast-page').handleAddPodcast()">
            Add Podcast
          </button>
        </div>
      </div>
    `;
  }

  handleUrlInput(value: string): void {
    this.feedUrl.set(value);
  }

  async handleValidate(): Promise<void> {
    const url = this.feedUrl.value.trim();
    if (!url) return;

    this.isValidating.set(true);
    this.isValidated.set(false);
    this.lookupResult.set(null);

    try {
      const results = await http.get<PodcastLookupResult[]>('/podcast/lookup', {
        params: { feedUrl: url },
      });
      if (results && results.length > 0) {
        this.lookupResult.set(results[0]);
      }
    } catch {
      showError('Failed to validate feed URL');
    } finally {
      this.isValidating.set(false);
      this.isValidated.set(true);
    }
  }

  handleReset(): void {
    this.lookupResult.set(null);
    this.isValidated.set(false);
  }

  handleAddPodcast(): void {
    const result = this.lookupResult.value;
    if (!result) return;

    const card = this.querySelector('.preview-card');
    const rootFolderEl = card?.querySelector('#rootFolder') as HTMLSelectElement | null;
    const qualityProfileEl = card?.querySelector('#qualityProfile') as HTMLSelectElement | null;
    const monitoredEl = card?.querySelector('#monitored') as HTMLInputElement | null;

    this.addPodcastMutation.mutate({
      title: result.title,
      qualityProfileId: qualityProfileEl ? Number.parseInt(qualityProfileEl.value, 10) : 0,
      rootFolderPath: rootFolderEl?.value ?? '',
      monitored: monitoredEl?.checked ?? true,
    } as Partial<Podcast>);
  }
}
