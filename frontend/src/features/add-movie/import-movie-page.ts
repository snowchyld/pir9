/**
 * Import Movie page - import existing movies from disk
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

interface RootFolder {
  id: number;
  path: string;
  freeSpace: number;
  contentType: string;
  unmappedFolders: Array<{
    name: string;
    path: string;
  }>;
}

interface QualityProfile {
  id: number;
  name: string;
}

interface ImportPreview {
  path: string;
  name: string;
  imdbId?: string;
  title?: string;
  year?: number;
  overview?: string;
  remotePoster?: string;
}

interface MovieLookupResult {
  imdbId?: string;
  title: string;
  year: number;
  overview?: string;
  images?: Array<{ coverType: string; url: string }>;
}

interface FilesystemEntry {
  type: string;
  name: string;
  path: string;
}

interface FilesystemResponse {
  parent: string | null;
  directories: FilesystemEntry[];
  files: FilesystemEntry[];
}

@customElement('import-movie-page')
export class ImportMoviePage extends BaseComponent {
  private selectedRootFolder = signal<string | null>(null);
  private unmappedFolders = signal<ImportPreview[]>([]);
  private selectedFolders = signal<Set<string>>(new Set());
  private showAddRootFolder = signal<boolean>(false);
  private selectedSuggestionIndex = signal<number>(-1);
  private currentSuggestions: FilesystemEntry[] = [];
  private debounceTimer: ReturnType<typeof setTimeout> | null = null;
  private pendingRootFolders = signal<string[]>([]);

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder', 'movie'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder', { params: { contentType: 'movie' } }),
  });

  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => http.get<QualityProfile[]>('/qualityprofile'),
  });

  private importMutation = createMutation({
    mutationFn: (movies: Record<string, unknown>[]) => http.post('/movie/import', movies),
    onSuccess: (_data, variables) => {
      const count = (variables as Record<string, unknown>[]).length;
      invalidateQueries(['/movie']);
      invalidateQueries(['/rootfolder']);
      showSuccess(`${count} movie${count !== 1 ? 's' : ''} imported successfully`);
      this.selectedFolders.set(new Set());
      const currentRoot = this.selectedRootFolder.value;
      if (currentRoot) {
        setTimeout(() => this.selectRootFolder(currentRoot), 100);
      }
    },
    onError: () => {
      showError('Failed to import movies');
    },
  });

  protected onInit(): void {
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.rootFoldersQuery.isLoading);
    this.watch(this.qualityProfilesQuery.data);
    this.watch(this.selectedRootFolder);
    this.watch(this.unmappedFolders);
    this.watch(this.selectedFolders);
    this.watch(this.showAddRootFolder);
    this.watch(this.pendingRootFolders);
  }

  protected template(): string {
    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const profiles = this.qualityProfilesQuery.data.value ?? [];
    const isLoading = this.rootFoldersQuery.isLoading.value;
    const selected = this.selectedRootFolder.value;
    const folders = this.unmappedFolders.value;
    const selectedSet = this.selectedFolders.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="import-page">
        <h1 class="page-title">Import Existing Movies</h1>

        <div class="step-section">
          <h2 class="step-title">1. Select Root Folder</h2>
          <p class="step-description">Choose a root folder containing movies you want to import.</p>

          <div class="root-folders-grid">
            ${rootFolders
              .map(
                (folder) => html`
              <div
                class="root-folder-card ${selected === folder.path ? 'selected' : ''}"
                onclick="this.closest('import-movie-page').selectRootFolder('${escapeHtml(folder.path)}')"
              >
                <div class="folder-icon">
                  <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
                  </svg>
                </div>
                <div class="folder-info">
                  <div class="folder-path">${escapeHtml(folder.path)}</div>
                  <div class="folder-meta">
                    ${folder.unmappedFolders?.length ?? 0} unmapped folders
                    • ${this.formatBytes(folder.freeSpace)} free
                  </div>
                </div>
              </div>
            `,
              )
              .join('')}

            <div
              class="root-folder-card add-new"
              onclick="this.closest('import-movie-page').openAddRootFolder()"
            >
              <div class="folder-icon add-icon">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <line x1="12" y1="5" x2="12" y2="19"></line>
                  <line x1="5" y1="12" x2="19" y2="12"></line>
                </svg>
              </div>
              <div class="folder-info">
                <div class="folder-path">Add Root Folder</div>
                <div class="folder-meta">Add a new location for movies</div>
              </div>
            </div>
          </div>

          ${
            this.showAddRootFolder.value
              ? html`
            <div class="add-root-folder-form">
              ${
                this.pendingRootFolders.value.length > 0
                  ? html`
                <div class="pending-folders">
                  <div class="pending-label">Folders to add:</div>
                  ${this.pendingRootFolders.value
                    .map(
                      (path, index) => html`
                    <div class="pending-folder-item">
                      <span class="pending-path">${escapeHtml(path)}</span>
                      <button class="btn-remove-pending" onclick="this.closest('import-movie-page').removePendingFolder(${index})">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <line x1="18" y1="6" x2="6" y2="18"></line>
                          <line x1="6" y1="6" x2="18" y2="18"></line>
                        </svg>
                      </button>
                    </div>
                  `,
                    )
                    .join('')}
                </div>
              `
                  : ''
              }
              <div class="form-row">
                <div class="path-input-wrapper">
                  <input
                    type="text"
                    class="form-input path-input"
                    id="rootFolderPathInput"
                    placeholder="Start typing a path (e.g., /volume1)"
                    oninput="this.closest('import-movie-page').onPathInput(this.value)"
                    onkeydown="this.closest('import-movie-page').onPathKeydown(event)"
                    onblur="setTimeout(() => this.closest('import-movie-page')?.hideSuggestions(), 200)"
                    autocomplete="off"
                  />
                  <div class="path-suggestions" id="pathSuggestions" style="display: none;"></div>
                </div>
                <button class="btn-add-another" onclick="this.closest('import-movie-page').queueRootFolder()">
                  + Add Another
                </button>
              </div>
              <div class="form-actions">
                <button class="btn-save-all" onclick="this.closest('import-movie-page').saveAllRootFolders()" ${this.pendingRootFolders.value.length === 0 ? 'disabled' : ''}>
                  Save ${this.pendingRootFolders.value.length > 0 ? `(${this.pendingRootFolders.value.length})` : ''}
                </button>
                <button class="btn-cancel" onclick="this.closest('import-movie-page').cancelAddRootFolder()">
                  Cancel
                </button>
              </div>
            </div>
          `
              : ''
          }
        </div>

        ${
          folders.length > 0
            ? html`
          <div class="step-section">
            <h2 class="step-title">2. Select Movies to Import</h2>
            <p class="step-description">Select the movie folders you want to import.</p>

            <div class="import-toolbar">
              <label class="checkbox-label">
                <input
                  type="checkbox"
                  ${selectedSet.size === folders.length ? 'checked' : ''}
                  onchange="this.closest('import-movie-page').toggleSelectAll()"
                />
                <span>Select All (${selectedSet.size}/${folders.length})</span>
              </label>
            </div>

            <div class="folders-list">
              ${folders
                .map(
                  (folder) => html`
                <div class="folder-row ${selectedSet.has(folder.path) ? 'selected' : ''}">
                  <input
                    type="checkbox"
                    class="folder-checkbox"
                    ${selectedSet.has(folder.path) ? 'checked' : ''}
                    onchange="this.closest('import-movie-page').toggleFolder('${escapeHtml(folder.path)}')"
                  />
                  <div class="folder-poster">
                    ${
                      folder.remotePoster
                        ? html`
                      <img src="${folder.remotePoster}" alt="${escapeHtml(folder.title ?? folder.name)}" loading="lazy" />
                    `
                        : html`
                      <div class="poster-placeholder">
                        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                          <rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect>
                        </svg>
                      </div>
                    `
                    }
                  </div>
                  <div class="folder-details">
                    <div class="folder-name">${escapeHtml(folder.title ?? folder.name)}</div>
                    <div class="folder-path-small">${escapeHtml(folder.path)}</div>
                  </div>
                  ${folder.year ? html`<div class="folder-year">${folder.year}</div>` : ''}
                </div>
              `,
                )
                .join('')}
            </div>
          </div>

          <div class="step-section">
            <h2 class="step-title">3. Configure Import Options</h2>

            <div class="options-grid">
              <div class="form-group">
                <label class="form-label">Quality Profile</label>
                <select class="form-select" id="importQualityProfile">
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
                  <input type="checkbox" id="importMonitored" checked />
                  <span>Monitor imported movies</span>
                </label>
              </div>
            </div>
          </div>

          <div class="import-actions">
            <button
              class="import-btn"
              onclick="this.closest('import-movie-page').handleImport()"
              ${selectedSet.size === 0 ? 'disabled' : ''}
            >
              Import ${selectedSet.size} Movie${selectedSet.size !== 1 ? 's' : ''}
            </button>
          </div>
        `
            : ''
        }

        ${
          selected && folders.length === 0
            ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
            </svg>
            <p>No unmapped folders found</p>
            <p class="hint">All movies in this root folder have already been imported</p>
          </div>
        `
            : ''
        }
      </div>

      <style>
        .import-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .loading-container {
          display: flex;
          justify-content: center;
          padding: 4rem;
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

        .step-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .step-title {
          font-size: 1rem;
          font-weight: 600;
          margin: 0 0 0.5rem 0;
        }

        .step-description {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin: 0 0 1rem 0;
        }

        .root-folders-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 1rem;
        }

        .root-folder-card {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 1rem;
          background-color: var(--bg-card-alt);
          border: 2px solid var(--border-color);
          border-radius: 0.375rem;
          cursor: pointer;
          transition: border-color 0.15s;
        }

        .root-folder-card:hover {
          border-color: var(--color-primary);
        }

        .root-folder-card.selected {
          border-color: var(--color-primary);
          background-color: rgba(93, 156, 236, 0.1);
        }

        .folder-icon {
          display: flex;
          color: var(--color-primary);
        }

        .folder-info {
          flex: 1;
          min-width: 0;
        }

        .folder-path {
          font-weight: 500;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .folder-meta {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .root-folder-card.add-new {
          border-style: dashed;
          background-color: transparent;
        }

        .root-folder-card.add-new:hover {
          background-color: rgba(93, 156, 236, 0.05);
        }

        .add-icon {
          color: var(--text-color-muted);
        }

        .root-folder-card.add-new:hover .add-icon {
          color: var(--color-primary);
        }

        .add-root-folder-form {
          margin-top: 1rem;
          padding: 1rem;
          background-color: var(--bg-card-alt);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
        }

        .form-row {
          display: flex;
          gap: 0.5rem;
          align-items: center;
        }

        .form-input {
          flex: 1;
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .form-input:focus {
          outline: none;
          border-color: var(--color-primary);
        }

        .btn-cancel {
          padding: 0.5rem 1rem;
          background-color: transparent;
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          cursor: pointer;
        }

        .btn-cancel:hover {
          background-color: var(--bg-card-alt);
        }

        .pending-folders {
          margin-bottom: 1rem;
        }

        .pending-label {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin-bottom: 0.5rem;
        }

        .pending-folder-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 0.5rem 0.75rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          margin-bottom: 0.25rem;
        }

        .pending-path {
          font-size: 0.875rem;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .btn-remove-pending {
          display: flex;
          padding: 0.25rem;
          background: transparent;
          border: none;
          color: var(--text-color-muted);
          cursor: pointer;
        }

        .btn-remove-pending:hover {
          color: var(--color-danger);
        }

        .form-actions {
          display: flex;
          gap: 0.5rem;
          margin-top: 1rem;
        }

        .btn-add-another {
          padding: 0.5rem 1rem;
          background-color: transparent;
          border: 1px solid var(--color-primary);
          border-radius: 0.25rem;
          color: var(--color-primary);
          font-size: 0.875rem;
          cursor: pointer;
          white-space: nowrap;
        }

        .btn-add-another:hover {
          background-color: rgba(93, 156, 236, 0.1);
        }

        .btn-save-all {
          padding: 0.5rem 1rem;
          background-color: var(--color-success);
          border: none;
          border-radius: 0.25rem;
          color: white;
          font-size: 0.875rem;
          cursor: pointer;
        }

        .btn-save-all:hover:not(:disabled) {
          opacity: 0.9;
        }

        .btn-save-all:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .path-input-wrapper {
          position: relative;
          flex: 1;
        }

        .path-input {
          width: 100%;
        }

        .path-suggestions {
          position: absolute;
          top: 100%;
          left: 0;
          right: 0;
          max-height: 300px;
          overflow-y: auto;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-top: none;
          border-radius: 0 0 0.25rem 0.25rem;
          box-shadow: 0 4px 12px rgba(0, 0, 0, 0.3);
          z-index: 100;
        }

        .suggestion-item {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.75rem;
          cursor: pointer;
          color: var(--text-color);
        }

        .suggestion-item:hover,
        .suggestion-item.selected {
          background-color: var(--color-primary);
          color: white;
        }

        .suggestion-item svg {
          flex-shrink: 0;
          opacity: 0.7;
        }

        .suggestion-path {
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          font-size: 0.875rem;
        }

        .import-toolbar {
          display: flex;
          align-items: center;
          margin-bottom: 1rem;
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

        .folders-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
          max-height: 400px;
          overflow-y: auto;
        }

        .folder-row {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem;
          background-color: var(--bg-card-alt);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
        }

        .folder-row.selected {
          border-color: var(--color-primary);
          background-color: rgba(93, 156, 236, 0.05);
        }

        .folder-checkbox {
          width: 18px;
          height: 18px;
          accent-color: var(--color-primary);
        }

        .folder-poster {
          width: 40px;
          height: 60px;
          flex-shrink: 0;
          border-radius: 0.25rem;
          overflow: hidden;
          background-color: var(--bg-card);
        }

        .folder-poster img {
          width: 100%;
          height: 100%;
          object-fit: cover;
        }

        .poster-placeholder {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 100%;
          height: 100%;
          color: var(--text-color-muted);
        }

        .folder-details {
          flex: 1;
          min-width: 0;
        }

        .folder-name {
          font-weight: 500;
        }

        .folder-path-small {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .folder-year {
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .options-grid {
          display: grid;
          grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
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

        .import-actions {
          display: flex;
          justify-content: flex-end;
        }

        .import-btn {
          padding: 0.75rem 1.5rem;
          background-color: var(--color-success);
          border: 1px solid var(--color-success);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .import-btn:hover:not(:disabled) {
          opacity: 0.9;
        }

        .import-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
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

  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  async selectRootFolder(path: string): Promise<void> {
    this.selectedRootFolder.set(path);
    this.selectedFolders.set(new Set());

    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const folder = rootFolders.find((f) => f.path === path);

    if (folder?.unmappedFolders && folder.unmappedFolders.length > 0) {
      // Set initial previews with just folder names
      const previews: ImportPreview[] = folder.unmappedFolders.map((f) => ({
        path: f.path,
        name: f.name,
      }));
      this.unmappedFolders.set(previews);

      // Look up each folder on IMDB to get movie metadata
      const updatedPreviews = await Promise.all(
        folder.unmappedFolders.map(async (f) => {
          const searchTerm = this.parseMovieName(f.name);
          try {
            const results = await http.get<MovieLookupResult[]>(
              `/movie/lookup?term=${encodeURIComponent(searchTerm)}`,
            );
            if (results.length > 0) {
              const match = results[0];
              return {
                path: f.path,
                name: f.name,
                imdbId: match.imdbId,
                title: match.title,
                year: match.year,
                overview: match.overview,
                remotePoster: match.images?.find(
                  (img: { coverType: string }) => img.coverType === 'poster',
                )?.url,
              };
            }
          } catch {
            // Lookup failed, keep original
          }
          return { path: f.path, name: f.name };
        }),
      );

      this.unmappedFolders.set(updatedPreviews);
      // Auto-select all folders that have a match
      const matched = updatedPreviews.filter((p) => p.imdbId);
      this.selectedFolders.set(new Set(matched.map((p) => p.path)));
    } else {
      this.unmappedFolders.set([]);
    }
  }

  private parseMovieName(folderName: string): string {
    // Remove year suffix like " (2020)" or "(2020)"
    let name = folderName.replace(/\s*\(\d{4}\)\s*$/, '');
    // Remove quality indicators
    name = name.replace(/\s*(1080p|720p|480p|2160p|4k)/gi, '');
    // Remove source indicators
    name = name.replace(/\s*(bluray|bdrip|hdtv|webrip|web-dl|dvdrip|remux)/gi, '');
    return name.trim();
  }

  toggleFolder(path: string): void {
    const current = new Set(this.selectedFolders.value);
    if (current.has(path)) {
      current.delete(path);
    } else {
      current.add(path);
    }
    this.selectedFolders.set(current);
  }

  toggleSelectAll(): void {
    const folders = this.unmappedFolders.value;
    const current = this.selectedFolders.value;

    if (current.size === folders.length) {
      this.selectedFolders.set(new Set());
    } else {
      this.selectedFolders.set(new Set(folders.map((f) => f.path)));
    }
  }

  handleImport(): void {
    const selectedPaths = Array.from(this.selectedFolders.value);
    const folders = this.unmappedFolders.value;
    const rootFolder = this.selectedRootFolder.value;

    const section = this.querySelector('.step-section:last-of-type');
    const qualityProfileEl = section?.querySelector(
      '#importQualityProfile',
    ) as HTMLSelectElement | null;
    const monitoredEl = section?.querySelector('#importMonitored') as HTMLInputElement | null;

    const moviesToImport = selectedPaths.map((path) => {
      const folder = folders.find((f) => f.path === path);
      return {
        path,
        title: folder?.title ?? folder?.name,
        imdbId: folder?.imdbId,
        qualityProfileId: qualityProfileEl ? parseInt(qualityProfileEl.value, 10) : 1,
        rootFolderPath: rootFolder,
        monitored: monitoredEl?.checked ?? true,
        year: folder?.year,
      };
    });

    this.importMutation.mutate(moviesToImport);
  }

  openAddRootFolder(): void {
    this.showAddRootFolder.set(true);
    this.currentSuggestions = [];
    this.selectedSuggestionIndex.set(-1);
    setTimeout(() => {
      const input = this.querySelector('#rootFolderPathInput') as HTMLInputElement;
      if (input) {
        input.value = '/';
        input.focus();
        this.fetchSuggestions('/');
      }
    }, 0);
  }

  hideSuggestions(): void {
    const container = this.querySelector('#pathSuggestions') as HTMLElement;
    if (container) {
      container.style.display = 'none';
      while (container.firstChild) container.removeChild(container.firstChild);
    }
    this.currentSuggestions = [];
    this.selectedSuggestionIndex.set(-1);
  }

  onPathInput(value: string): void {
    if (this.debounceTimer) {
      clearTimeout(this.debounceTimer);
    }
    this.debounceTimer = setTimeout(() => {
      this.fetchSuggestions(value);
    }, 150);
  }

  async fetchSuggestions(path: string): Promise<void> {
    const container = this.querySelector('#pathSuggestions') as HTMLElement;
    if (!container) return;

    if (!path) {
      this.hideSuggestions();
      return;
    }

    try {
      let queryPath = path;
      if (!path.endsWith('/')) {
        const lastSlash = path.lastIndexOf('/');
        queryPath = lastSlash > 0 ? path.substring(0, lastSlash) : '/';
      }

      const response = await http.get<FilesystemResponse>(
        `/filesystem?path=${encodeURIComponent(queryPath)}`,
      );

      let filtered = response.directories;
      if (!path.endsWith('/')) {
        const searchTerm = path.substring(path.lastIndexOf('/') + 1).toLowerCase();
        filtered = response.directories.filter((d) => d.name.toLowerCase().startsWith(searchTerm));
      }

      this.currentSuggestions = filtered.slice(0, 10);
      this.selectedSuggestionIndex.set(-1);
      this.renderSuggestions();
    } catch {
      this.hideSuggestions();
    }
  }

  private renderSuggestions(): void {
    const container = this.querySelector('#pathSuggestions') as HTMLElement;
    if (!container) return;

    while (container.firstChild) container.removeChild(container.firstChild);

    if (this.currentSuggestions.length === 0) {
      container.style.display = 'none';
      return;
    }

    const selectedIndex = this.selectedSuggestionIndex.value;

    this.currentSuggestions.forEach((entry, index) => {
      const item = document.createElement('div');
      item.className = `suggestion-item${index === selectedIndex ? ' selected' : ''}`;
      item.dataset.path = entry.path;

      const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      svg.setAttribute('width', '16');
      svg.setAttribute('height', '16');
      svg.setAttribute('viewBox', '0 0 24 24');
      svg.setAttribute('fill', 'none');
      svg.setAttribute('stroke', 'currentColor');
      svg.setAttribute('stroke-width', '2');
      const path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      path.setAttribute(
        'd',
        'M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z',
      );
      svg.appendChild(path);
      item.appendChild(svg);

      const span = document.createElement('span');
      span.className = 'suggestion-path';
      span.textContent = entry.path;
      item.appendChild(span);

      item.addEventListener('mousedown', (e) => {
        e.preventDefault();
        this.selectSuggestion(entry.path);
      });

      container.appendChild(item);
    });

    container.style.display = 'block';
  }

  onPathKeydown(event: KeyboardEvent): void {
    const suggestions = this.currentSuggestions;
    const currentIndex = this.selectedSuggestionIndex.value;

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      const newIndex = currentIndex < suggestions.length - 1 ? currentIndex + 1 : 0;
      this.selectedSuggestionIndex.set(newIndex);
      this.renderSuggestions();
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      const newIndex = currentIndex > 0 ? currentIndex - 1 : suggestions.length - 1;
      this.selectedSuggestionIndex.set(newIndex);
      this.renderSuggestions();
    } else if (event.key === 'Tab' || event.key === 'Enter') {
      if (suggestions.length > 0 && currentIndex >= 0) {
        event.preventDefault();
        this.selectSuggestion(suggestions[currentIndex].path);
      } else if (event.key === 'Tab' && suggestions.length > 0) {
        event.preventDefault();
        this.selectSuggestion(suggestions[0].path);
      } else if (event.key === 'Enter') {
        this.queueRootFolder();
      }
    } else if (event.key === 'Escape') {
      this.hideSuggestions();
    }
  }

  selectSuggestion(path: string): void {
    const input = this.querySelector('#rootFolderPathInput') as HTMLInputElement;
    if (input) {
      input.value = `${path}/`;
      input.focus();
    }
    this.fetchSuggestions(`${path}/`);
  }

  queueRootFolder(): void {
    const input = this.querySelector('#rootFolderPathInput') as HTMLInputElement;
    const path = input?.value?.trim().replace(/\/+$/, '');

    if (!path) {
      showError('Please enter a folder path');
      return;
    }

    const current = this.pendingRootFolders.value;
    if (current.includes(path)) {
      showError('This path is already in the list');
      return;
    }

    const existingFolders = this.rootFoldersQuery.data.value ?? [];
    if (existingFolders.some((f) => f.path === path)) {
      showError('This root folder already exists');
      return;
    }

    this.pendingRootFolders.set([...current, path]);

    if (input) {
      input.value = '/';
      input.focus();
      this.fetchSuggestions('/');
    }
  }

  removePendingFolder(index: number): void {
    const current = [...this.pendingRootFolders.value];
    current.splice(index, 1);
    this.pendingRootFolders.set(current);
  }

  async saveAllRootFolders(): Promise<void> {
    const paths = this.pendingRootFolders.value;

    const input = this.querySelector('#rootFolderPathInput') as HTMLInputElement;
    const currentPath = input?.value?.trim().replace(/\/+$/, '');
    if (currentPath && !paths.includes(currentPath)) {
      paths.push(currentPath);
    }

    if (paths.length === 0) {
      showError('Please add at least one folder path');
      return;
    }

    let successCount = 0;
    const errors: string[] = [];

    for (const path of paths) {
      try {
        await http.post('/rootfolder', { path, contentType: 'movie' });
        successCount++;
      } catch {
        errors.push(path);
      }
    }

    invalidateQueries(['/rootfolder']);
    this.showAddRootFolder.set(false);
    this.pendingRootFolders.set([]);
    this.currentSuggestions = [];

    if (successCount > 0) {
      showSuccess(`${successCount} root folder${successCount > 1 ? 's' : ''} added successfully`);
    }
    if (errors.length > 0) {
      showError(`Failed to add: ${errors.join(', ')}`);
    }
  }

  cancelAddRootFolder(): void {
    this.showAddRootFolder.set(false);
    this.pendingRootFolders.set([]);
    this.currentSuggestions = [];
    this.selectedSuggestionIndex.set(-1);
  }
}
