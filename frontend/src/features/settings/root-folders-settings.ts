/**
 * Root Folders Settings page — dedicated page for root folder management.
 * Re-uses the root folder rendering logic from media-management-settings.
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

const CONTENT_TYPES = ['series', 'anime', 'movie', 'music', 'podcast', 'audiobook'];

@customElement('root-folders-settings')
export class RootFoldersSettings extends BaseComponent {
  private newFolderPath = signal('');
  private newFolderType = signal('series');

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder'),
  });

  private addFolderMutation = createMutation({
    mutationFn: (data: { path: string; contentType: string }) => http.post('/rootfolder', data),
    onSuccess: () => {
      showSuccess('Root folder added');
      this.newFolderPath.set('');
      invalidateQueries(['/rootfolder']);
      this.rootFoldersQuery.refetch();
    },
    onError: () => showError('Failed to add root folder'),
  });

  protected onInit(): void {
    this.watch(this.rootFoldersQuery.data);
    this.watch(this.rootFoldersQuery.isLoading);
    this.watch(this.newFolderPath);
    this.watch(this.newFolderType);
    this.watch(this.addFolderMutation.isLoading);
  }

  protected template(): string {
    const folders = this.rootFoldersQuery.data.value ?? [];
    const isLoading = this.rootFoldersQuery.isLoading.value;

    // Group by content type
    const grouped: Record<string, RootFolder[]> = {};
    for (const ct of CONTENT_TYPES) {
      grouped[ct] = folders.filter((f) => f.contentType === ct);
    }

    return html`
      <div class="root-folders-page">
        <div class="page-header">
          <h2>Root Folders</h2>
          <p class="page-description">
            Configure where pir9 looks for your media libraries. Each folder is assigned a content type.
          </p>
        </div>

        ${isLoading ? '<div class="loading">Loading...</div>' : ''}

        <div class="folders-grid">
          ${CONTENT_TYPES.map((ct) => {
            const ctFolders = grouped[ct] ?? [];
            const label = ct.charAt(0).toUpperCase() + ct.slice(1);
            return html`
              <div class="folder-group">
                <div class="group-header">
                  <h3>${label}</h3>
                  <span class="folder-count">${ctFolders.length}</span>
                </div>
                ${
                  ctFolders.length > 0
                    ? ctFolders
                        .map(
                          (f) => html`
                    <div class="folder-item">
                      <span class="folder-path">${escapeHtml(f.path)}</span>
                      ${f.freeSpace != null ? `<span class="folder-space">${this.formatSize(f.freeSpace)} free</span>` : ''}
                      <button class="remove-btn" onclick="this.closest('root-folders-settings').handleDeleteFolder(${f.id})">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                          <line x1="18" y1="6" x2="6" y2="18"></line>
                          <line x1="6" y1="6" x2="18" y2="18"></line>
                        </svg>
                      </button>
                    </div>
                  `,
                        )
                        .join('')
                    : `<div class="no-folders">No ${label.toLowerCase()} folders</div>`
                }
              </div>
            `;
          }).join('')}
        </div>

        <div class="add-section">
          <h3>Add Root Folder</h3>
          <div class="add-form">
            <input
              type="text"
              class="path-input"
              placeholder="/volume1/Media"
              value="${escapeHtml(this.newFolderPath.value)}"
              oninput="this.closest('root-folders-settings').newFolderPath.set(this.value); this.closest('root-folders-settings').requestUpdate()"
            />
            <select
              class="type-select"
              onchange="this.closest('root-folders-settings').newFolderType.set(this.value); this.closest('root-folders-settings').requestUpdate()"
            >
              ${CONTENT_TYPES.map(
                (ct) =>
                  `<option value="${ct}" ${this.newFolderType.value === ct ? 'selected' : ''}>${ct.charAt(0).toUpperCase() + ct.slice(1)}</option>`,
              ).join('')}
            </select>
            <button
              class="add-btn"
              onclick="this.closest('root-folders-settings').handleAddFolder()"
              ${this.addFolderMutation.isLoading.value ? 'disabled' : ''}
            >
              ${this.addFolderMutation.isLoading.value ? 'Adding...' : 'Add'}
            </button>
          </div>
        </div>
      </div>

      <style>
        .root-folders-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .page-header h2 { margin: 0 0 0.25rem 0; font-size: 1.25rem; }
        .page-description { color: var(--text-color-muted); font-size: 0.875rem; margin: 0; }

        .folders-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
          gap: 1rem;
        }

        .folder-group {
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          padding: 1rem;
        }

        .group-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 0.75rem;
        }

        .group-header h3 { margin: 0; font-size: 0.95rem; font-weight: 600; }

        .folder-count {
          background: var(--bg-card-center);
          padding: 0.1rem 0.5rem;
          border-radius: 9999px;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .folder-item {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.375rem 0.5rem;
          background: var(--bg-card-center);
          border-radius: 0.375rem;
          margin-bottom: 0.375rem;
        }

        .folder-path {
          flex: 1;
          font-family: monospace;
          font-size: 0.8125rem;
          word-break: break-all;
        }

        .folder-space {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          white-space: nowrap;
        }

        .remove-btn {
          background: none;
          border: none;
          color: var(--text-color-muted);
          cursor: pointer;
          padding: 0.25rem;
          border-radius: 0.25rem;
          display: flex;
          transition: all var(--transition-normal);
        }

        .remove-btn:hover { color: var(--color-danger); }

        .no-folders {
          color: var(--text-color-muted);
          font-size: 0.8125rem;
          font-style: italic;
          padding: 0.5rem 0;
        }

        .add-section {
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          padding: 1rem;
        }

        .add-section h3 { margin: 0 0 0.75rem 0; font-size: 1rem; }

        .add-form {
          display: flex;
          gap: 0.5rem;
          align-items: center;
          flex-wrap: wrap;
        }

        .path-input {
          flex: 1;
          min-width: 250px;
          padding: 0.5rem 0.75rem;
          background: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.375rem;
          color: var(--text-color);
          font-family: monospace;
          font-size: 0.875rem;
        }

        .type-select {
          padding: 0.5rem 0.75rem;
          background: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.375rem;
          color: var(--text-color);
          font-size: 0.875rem;
          min-width: 120px;
        }

        .add-btn {
          padding: 0.5rem 1rem;
          background: var(--btn-primary-bg);
          border: none;
          border-radius: 0.375rem;
          color: white;
          cursor: pointer;
          font-size: 0.875rem;
          transition: background var(--transition-normal);
        }

        .add-btn:hover { background: var(--btn-primary-bg-hover); }
        .add-btn:disabled { opacity: 0.5; cursor: not-allowed; }
      </style>
    `;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
  }

  handleAddFolder(): void {
    const path = this.newFolderPath.value.trim();
    if (!path) {
      showError('Path is required');
      return;
    }
    this.addFolderMutation.mutate({
      path,
      contentType: this.newFolderType.value,
    });
  }

  async handleDeleteFolder(id: number): Promise<void> {
    if (!confirm('Remove this root folder?')) return;
    try {
      await http.delete(`/rootfolder/${id}`);
      showSuccess('Root folder removed');
      invalidateQueries(['/rootfolder']);
      this.rootFoldersQuery.refetch();
    } catch {
      showError('Failed to remove root folder');
    }
  }
}
