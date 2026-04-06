/**
 * Media Management Settings page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

interface RootFolder {
  id: number;
  path: string;
  contentType: string;
  freeSpace?: number;
  totalSpace?: number;
  accessible?: boolean;
}

interface NamingConfig {
  renameEpisodes: boolean;
  replaceIllegalCharacters: boolean;
  colonReplacementFormat: number;
  standardEpisodeFormat: string;
  dailyEpisodeFormat: string;
  animeEpisodeFormat: string;
  seriesFolderFormat: string;
  seasonFolderFormat: string;
  specialsFolderFormat: string;
  multiEpisodeStyle: number;
}

interface MediaManagementConfig {
  autoUnmonitorPreviouslyDownloadedEpisodes: boolean;
  recycleBin: string;
  recycleBinCleanupDays: number;
  downloadPropersAndRepacks: string;
  createEmptySeriesFolders: boolean;
  deleteEmptyFolders: boolean;
  fileDate: string;
  rescanAfterRefresh: string;
  setPermissionsLinux: boolean;
  chmodFolder: string;
  chownGroup: string;
  episodeTitleRequired: string;
  skipFreeSpaceCheckWhenImporting: boolean;
  minimumFreeSpaceWhenImporting: number;
  copyUsingHardlinks: boolean;
  importExtraFiles: boolean;
  extraFileExtensions: string;
  enableMediaInfo: boolean;
}

@customElement('media-management-settings')
export class MediaManagementSettings extends BaseComponent {
  private namingQuery = createQuery({
    queryKey: ['/config/naming'],
    queryFn: () => http.get<NamingConfig>('/config/naming'),
  });

  private mediaManagementQuery = createQuery({
    queryKey: ['/config/mediamanagement'],
    queryFn: () => http.get<MediaManagementConfig>('/config/mediamanagement'),
  });

  private saveMutation = createMutation({
    mutationFn: (data: {
      naming?: Partial<NamingConfig>;
      mediaManagement?: Partial<MediaManagementConfig>;
    }) =>
      Promise.all([
        data.naming ? http.put('/config/naming', data.naming) : Promise.resolve(),
        data.mediaManagement
          ? http.put('/config/mediamanagement', data.mediaManagement)
          : Promise.resolve(),
      ]),
    onSuccess: () => {
      invalidateQueries(['/config/naming']);
      invalidateQueries(['/config/mediamanagement']);
      showSuccess('Settings saved');
    },
    onError: () => {
      showError('Failed to save settings');
    },
  });

  // Root folder management
  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder'),
  });

  private newFolderPath = signal('');
  private newFolderType = signal('series');

  private addFolderMutation = createMutation({
    mutationFn: (data: { path: string; contentType: string }) => http.post('/rootfolder', data),
    onSuccess: () => {
      invalidateQueries(['/rootfolder']);
      this.newFolderPath.set('');
      showSuccess('Root folder added');
    },
    onError: () => {
      showError('Failed to add root folder — check that the path exists and is accessible');
    },
  });

  private deleteFolderMutation = createMutation({
    mutationFn: (id: number) => http.delete(`/rootfolder/${id}`),
    onSuccess: () => {
      invalidateQueries(['/rootfolder']);
      showSuccess('Root folder removed');
    },
    onError: () => {
      showError('Failed to remove root folder');
    },
  });

  protected onInit(): void {
    this.watch(this.namingQuery.data);
    this.watch(this.namingQuery.isLoading);
    this.watch(this.mediaManagementQuery.data);
    this.watch(this.mediaManagementQuery.isLoading);
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.newFolderPath);
    this.watch(this.newFolderType);
  }

  protected template(): string {
    const naming = this.namingQuery.data.value;
    const mediaManagement = this.mediaManagementQuery.data.value;
    const isLoading = this.namingQuery.isLoading.value || this.mediaManagementQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      ${this.renderRootFolders()}

      <div class="settings-section">
        <h2 class="section-title">Episode Naming</h2>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${naming?.renameEpisodes ? 'checked' : ''}
              onchange="this.closest('media-management-settings').handleNamingChange('renameEpisodes', this.checked)"
            />
            <span>Rename Episodes</span>
          </label>
          <p class="form-hint">pir9 will use the existing file name if renaming is disabled</p>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${naming?.replaceIllegalCharacters ? 'checked' : ''}
              onchange="this.closest('media-management-settings').handleNamingChange('replaceIllegalCharacters', this.checked)"
            />
            <span>Replace Illegal Characters</span>
          </label>
        </div>

        <div class="form-group">
          <label class="form-label">Standard Episode Format</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(naming?.standardEpisodeFormat ?? '')}"
            onchange="this.closest('media-management-settings').handleNamingChange('standardEpisodeFormat', this.value)"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Daily Episode Format</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(naming?.dailyEpisodeFormat ?? '')}"
            onchange="this.closest('media-management-settings').handleNamingChange('dailyEpisodeFormat', this.value)"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Anime Episode Format</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(naming?.animeEpisodeFormat ?? '')}"
            onchange="this.closest('media-management-settings').handleNamingChange('animeEpisodeFormat', this.value)"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Specials Folder Format</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(naming?.specialsFolderFormat ?? '')}"
            onchange="this.closest('media-management-settings').handleNamingChange('specialsFolderFormat', this.value)"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Multi-Episode Style</label>
          <select
            class="form-input"
            onchange="this.closest('media-management-settings').handleNamingChange('multiEpisodeStyle', parseInt(this.value))"
          >
            <option value="0" ${naming?.multiEpisodeStyle === 0 ? 'selected' : ''}>Extend (S01E01-02-03)</option>
            <option value="1" ${naming?.multiEpisodeStyle === 1 ? 'selected' : ''}>Duplicate (S01E01.S01E02.S01E03)</option>
            <option value="2" ${naming?.multiEpisodeStyle === 2 ? 'selected' : ''}>Repeat (S01E01E02E03)</option>
            <option value="3" ${naming?.multiEpisodeStyle === 3 ? 'selected' : ''}>Scene (S01E01-E02-E03)</option>
            <option value="4" ${naming?.multiEpisodeStyle === 4 ? 'selected' : ''}>Range (S01E01-03)</option>
            <option value="5" ${naming?.multiEpisodeStyle === 5 ? 'selected' : ''}>Prefixed Range (S01E01-E03)</option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label">Colon Replacement</label>
          <select
            class="form-input"
            onchange="this.closest('media-management-settings').handleNamingChange('colonReplacementFormat', parseInt(this.value))"
          >
            <option value="0" ${naming?.colonReplacementFormat === 0 ? 'selected' : ''}>Delete</option>
            <option value="1" ${naming?.colonReplacementFormat === 1 ? 'selected' : ''}>Replace with Space</option>
            <option value="4" ${naming?.colonReplacementFormat === 4 ? 'selected' : ''}>Replace with Dash</option>
          </select>
        </div>

        <details class="token-legend">
          <summary class="token-legend-toggle">Available Tokens</summary>
          <div class="token-legend-content">
            <table class="token-table">
              <thead>
                <tr><th>Token</th><th>Description</th><th>Example</th></tr>
              </thead>
              <tbody>
                <tr><td><code>{Series Title}</code></td><td>Series name</td><td>The Flash</td></tr>
                <tr><td><code>{Series CleanTitle}</code></td><td>Lowercase, no punctuation</td><td>theflash</td></tr>
                <tr><td><code>{Series TitleYear}</code></td><td>Name with year</td><td>The Flash (2014)</td></tr>
                <tr><td><code>{season:00}</code></td><td>Season number (padded)</td><td>01</td></tr>
                <tr><td><code>{episode:00}</code></td><td>Episode number (padded)</td><td>05</td></tr>
                <tr><td><code>{Episode Title}</code></td><td>Episode name</td><td>Pilot</td></tr>
                <tr><td><code>{Quality Full}</code></td><td>Quality with Proper/Repack</td><td>WEBDL-1080p Proper</td></tr>
                <tr><td><code>{Quality Title}</code></td><td>Quality name only</td><td>WEBDL-1080p</td></tr>
                <tr><td><code>{Air-Date}</code></td><td>Original air date</td><td>2024-01-15</td></tr>
                <tr><td><code>{absolute:000}</code></td><td>Absolute episode number</td><td>015</td></tr>
                <tr><td><code>{Release Group}</code></td><td>Release group name</td><td>EVOLVE</td></tr>
              </tbody>
            </table>
            <p class="token-hint">Padding: <code>:00</code> = 2 digits, <code>:000</code> = 3 digits. Empty <code>[{Release Group}]</code> auto-removes the brackets.</p>
          </div>
        </details>

        <div class="form-group">
          <label class="form-label">Series Folder Format</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(naming?.seriesFolderFormat ?? '')}"
            onchange="this.closest('media-management-settings').handleNamingChange('seriesFolderFormat', this.value)"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Season Folder Format</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(naming?.seasonFolderFormat ?? '')}"
            onchange="this.closest('media-management-settings').handleNamingChange('seasonFolderFormat', this.value)"
          />
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Importing</h2>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${mediaManagement?.skipFreeSpaceCheckWhenImporting ? 'checked' : ''}
              onchange="this.closest('media-management-settings').handleMediaManagementChange('skipFreeSpaceCheckWhenImporting', this.checked)"
            />
            <span>Skip Free Space Check When Importing</span>
          </label>
        </div>

        <div class="form-group">
          <label class="form-label">Minimum Free Space</label>
          <input
            type="number"
            class="form-input"
            value="${mediaManagement?.minimumFreeSpaceWhenImporting ?? 100}"
            onchange="this.closest('media-management-settings').handleMediaManagementChange('minimumFreeSpaceWhenImporting', parseInt(this.value))"
          />
          <p class="form-hint">Prevent import if free space is below this threshold (in MB)</p>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${mediaManagement?.copyUsingHardlinks ? 'checked' : ''}
              onchange="this.closest('media-management-settings').handleMediaManagementChange('copyUsingHardlinks', this.checked)"
            />
            <span>Use Hardlinks instead of Copy</span>
          </label>
          <p class="form-hint">Use hard links when trying to copy files from torrents that are still being seeded</p>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${mediaManagement?.importExtraFiles ? 'checked' : ''}
              onchange="this.closest('media-management-settings').handleMediaManagementChange('importExtraFiles', this.checked)"
            />
            <span>Import Extra Files</span>
          </label>
        </div>

        <div class="form-group">
          <label class="form-label">Extra File Extensions</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(mediaManagement?.extraFileExtensions ?? '')}"
            placeholder="srt,nfo"
            onchange="this.closest('media-management-settings').handleMediaManagementChange('extraFileExtensions', this.value)"
          />
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">File Management</h2>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${mediaManagement?.autoUnmonitorPreviouslyDownloadedEpisodes ? 'checked' : ''}
              onchange="this.closest('media-management-settings').handleMediaManagementChange('autoUnmonitorPreviouslyDownloadedEpisodes', this.checked)"
            />
            <span>Unmonitor Deleted Episodes</span>
          </label>
        </div>

        <div class="form-group">
          <label class="form-label">Recycle Bin</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(mediaManagement?.recycleBin ?? '')}"
            placeholder="/path/to/recycle/bin"
            onchange="this.closest('media-management-settings').handleMediaManagementChange('recycleBin', this.value)"
          />
          <p class="form-hint">Episodes will be moved here instead of being permanently deleted</p>
        </div>

        <div class="form-group">
          <label class="form-label">Recycle Bin Cleanup (days)</label>
          <input
            type="number"
            class="form-input"
            value="${mediaManagement?.recycleBinCleanupDays ?? 7}"
            onchange="this.closest('media-management-settings').handleMediaManagementChange('recycleBinCleanupDays', parseInt(this.value))"
          />
        </div>
      </div>

      <div class="actions">
        <button class="save-btn" onclick="this.closest('media-management-settings').handleSave()">
          Save Changes
        </button>
      </div>

      <style>
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

        .settings-section {
          margin-bottom: 2rem;
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 1.5rem 0;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .form-group {
          margin-bottom: 1.25rem;
        }

        .form-group:last-child {
          margin-bottom: 0;
        }

        .form-label {
          display: block;
          font-size: 0.875rem;
          font-weight: 500;
          margin-bottom: 0.5rem;
          color: var(--text-color);
        }

        .form-input {
          width: 100%;
          max-width: 400px;
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

        .form-hint {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin: 0.25rem 0 0 0;
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

        .actions {
          margin-top: 1.5rem;
        }

        .save-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .save-btn:hover {
          background-color: var(--btn-primary-bg-hover);
        }

        .token-legend {
          margin-bottom: 1.25rem;
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
        }

        .token-legend-toggle {
          padding: 0.5rem 0.75rem;
          font-size: 0.8125rem;
          font-weight: 500;
          color: var(--text-color-muted);
          cursor: pointer;
          user-select: none;
        }

        .token-legend-toggle:hover {
          color: var(--text-color);
        }

        .token-legend-content {
          padding: 0.75rem;
          border-top: 1px solid var(--border-color);
        }

        .token-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.8125rem;
        }

        .token-table th {
          text-align: left;
          font-weight: 600;
          padding: 0.375rem 0.5rem;
          border-bottom: 1px solid var(--border-color);
          color: var(--text-color-muted);
        }

        .token-table td {
          padding: 0.375rem 0.5rem;
          border-bottom: 1px solid var(--border-color);
        }

        .token-table tr:last-child td {
          border-bottom: none;
        }

        .token-table code {
          background-color: var(--bg-input);
          padding: 0.125rem 0.375rem;
          border-radius: 0.1875rem;
          font-size: 0.75rem;
          white-space: nowrap;
        }

        .token-hint {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin: 0.5rem 0 0 0;
        }

        .token-hint code {
          background-color: var(--bg-input);
          padding: 0.0625rem 0.25rem;
          border-radius: 0.125rem;
          font-size: 0.6875rem;
        }
      </style>
    `;
  }

  private pendingNamingChanges: Partial<NamingConfig> = {};
  private pendingMediaManagementChanges: Partial<MediaManagementConfig> = {};

  handleNamingChange(key: keyof NamingConfig, value: unknown): void {
    this.pendingNamingChanges[key] = value as never;
  }

  handleMediaManagementChange(key: keyof MediaManagementConfig, value: unknown): void {
    this.pendingMediaManagementChanges[key] = value as never;
  }

  handleSave(): void {
    const hasNamingChanges = Object.keys(this.pendingNamingChanges).length > 0;
    const hasMediaManagementChanges = Object.keys(this.pendingMediaManagementChanges).length > 0;

    if (!hasNamingChanges && !hasMediaManagementChanges) {
      showSuccess('No changes to save');
      return;
    }

    this.saveMutation.mutate({
      naming: hasNamingChanges
        ? { ...this.namingQuery.data.value, ...this.pendingNamingChanges }
        : undefined,
      mediaManagement: hasMediaManagementChanges
        ? { ...this.mediaManagementQuery.data.value, ...this.pendingMediaManagementChanges }
        : undefined,
    });

    this.pendingNamingChanges = {};
    this.pendingMediaManagementChanges = {};
  }

  // ── Root Folders ──────────────────────────────────────────────────

  private renderRootFolders(): string {
    const folders = this.rootFoldersQuery.data.value ?? [];
    const contentTypes = ['series', 'anime', 'movie', 'music', 'podcast', 'audiobook'];

    // Group by content type
    const grouped: Record<string, RootFolder[]> = {};
    for (const ct of contentTypes) {
      grouped[ct] = folders.filter((f) => f.contentType === ct);
    }
    // Also include any with unknown content type
    const knownTypes = new Set(contentTypes);
    const other = folders.filter((f) => !knownTypes.has(f.contentType));
    if (other.length > 0) grouped['other'] = other;

    return html`
      <div class="settings-section">
        <h2 class="section-title">Root Folders</h2>
        <p class="section-description">
          Root folders are where pir9 looks for your media library. Each folder has a content type
          that determines which library it belongs to.
        </p>

        ${contentTypes
          .map((ct) => {
            const ctFolders = grouped[ct] ?? [];
            const label = ct.charAt(0).toUpperCase() + ct.slice(1);
            return html`
            <div class="root-folder-group" style="margin-bottom: 1rem;">
              <h3 class="subsection-title" style="font-size: 0.95em; margin-bottom: 0.5rem;">${label}</h3>
              ${
                ctFolders.length > 0
                  ? html`
                <div class="root-folder-list">
                  ${ctFolders
                    .map(
                      (f) => html`
                    <div class="root-folder-item" style="display: flex; align-items: center; justify-content: space-between; padding: 0.4rem 0.75rem; background: var(--bg-secondary); border-radius: 4px; margin-bottom: 0.25rem;">
                      <span style="font-family: monospace; font-size: 0.9em;">${escapeHtml(f.path)}</span>
                      <button
                        class="danger-btn"
                        style="padding: 0.2rem 0.5rem; font-size: 0.8em;"
                        onclick="this.closest('media-management-settings').handleDeleteFolder(${f.id})"
                      >Remove</button>
                    </div>
                  `,
                    )
                    .join('')}
                </div>
              `
                  : html`<div style="color: var(--text-muted); font-size: 0.85em; padding: 0.3rem 0;">No ${label.toLowerCase()} root folders configured</div>`
              }
            </div>
          `;
          })
          .join('')}

        <div class="add-folder-form" style="margin-top: 1rem; padding: 1rem; background: var(--bg-secondary); border-radius: 6px;">
          <h3 class="subsection-title" style="margin-bottom: 0.75rem;">Add Root Folder</h3>
          <div style="display: flex; gap: 0.5rem; align-items: end; flex-wrap: wrap;">
            <div style="flex: 1; min-width: 250px;">
              <label class="form-label" style="font-size: 0.85em;">Path</label>
              <input
                type="text"
                class="form-input"
                placeholder="/volume1/Music"
                value="${escapeHtml(this.newFolderPath.value)}"
                oninput="this.closest('media-management-settings').handleNewFolderPathChange(this.value)"
              />
            </div>
            <div style="min-width: 150px;">
              <label class="form-label" style="font-size: 0.85em;">Content Type</label>
              <select
                class="form-select"
                onchange="this.closest('media-management-settings').handleNewFolderTypeChange(this.value)"
              >
                ${contentTypes
                  .map(
                    (ct) => html`
                  <option value="${ct}" ${this.newFolderType.value === ct ? 'selected' : ''}>${ct.charAt(0).toUpperCase() + ct.slice(1)}</option>
                `,
                  )
                  .join('')}
              </select>
            </div>
            <button
              class="primary-btn"
              style="height: 38px;"
              onclick="this.closest('media-management-settings').handleAddFolder()"
              ${this.addFolderMutation.isLoading.value ? 'disabled' : ''}
            >
              ${this.addFolderMutation.isLoading.value ? 'Adding...' : 'Add Folder'}
            </button>
          </div>
        </div>
      </div>
    `;
  }

  handleNewFolderPathChange(value: string): void {
    this.newFolderPath.set(value);
  }

  handleNewFolderTypeChange(value: string): void {
    this.newFolderType.set(value);
  }

  handleAddFolder(): void {
    const path = this.newFolderPath.value.trim();
    if (!path) {
      showError('Path is required');
      return;
    }
    this.addFolderMutation.mutate({ path, contentType: this.newFolderType.value });
  }

  handleDeleteFolder(id: number): void {
    if (window.confirm('Remove this root folder? (Files will not be deleted)')) {
      this.deleteFolderMutation.mutate(id);
    }
  }
}
