/**
 * Media Management Settings page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { showError, showSuccess } from '../../stores/app.store';

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
      invalidateQueries(['/config/naming', '/config/mediamanagement']);
      showSuccess('Settings saved');
    },
    onError: () => {
      showError('Failed to save settings');
    },
  });

  protected onInit(): void {
    this.watch(this.namingQuery.data);
    this.watch(this.namingQuery.isLoading);
    this.watch(this.mediaManagementQuery.data);
    this.watch(this.mediaManagementQuery.isLoading);
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
}
