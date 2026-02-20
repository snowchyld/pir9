/**
 * Episode rename dialog - shows current vs. templated filenames
 * and allows bulk or individual renaming.
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

interface RenamePreviewItem {
  episodeFileId: number;
  seriesId: number;
  seasonNumber: number;
  episodeNumbers: number[];
  existingPath: string;
  newPath: string;
}

interface RenameResult {
  renamed: number;
  failed: number;
  errors: string[];
}

@customElement('episode-rename-dialog')
export class EpisodeRenameDialog extends BaseComponent {
  private isOpen = signal(false);
  private seriesId = signal<number | null>(null);
  private seriesTitle = signal('');
  private seasonFilter = signal<number | null>(null);
  private previews = signal<RenamePreviewItem[]>([]);
  private isLoading = signal(false);
  private selectedIds = signal<Set<number>>(new Set());

  private renameMutation = createMutation({
    mutationFn: (params: {
      seriesId: number;
      files: Array<{ episodeFileId: number; newPath: string }>;
    }) => http.put<RenameResult>('/rename', params),
    onSuccess: (result: RenameResult) => {
      if (result.renamed > 0) {
        showSuccess(`Renamed ${result.renamed} file${result.renamed > 1 ? 's' : ''}`);
      }
      if (result.failed > 0) {
        showError(`Failed to rename ${result.failed} file${result.failed > 1 ? 's' : ''}`);
      }
      // Refresh the preview list to show remaining items
      const id = this.seriesId.value;
      if (id) {
        this.loadPreview(id);
      }
    },
    onError: () => {
      showError('Failed to rename files');
    },
  });

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.previews);
    this.watch(this.isLoading);
    this.watch(this.selectedIds);
    this.watch(this.renameMutation.isLoading);
  }

  async open(seriesId: number, seriesTitle: string, seasonNumber?: number): Promise<void> {
    this.seriesId.set(seriesId);
    this.seriesTitle.set(seriesTitle);
    this.seasonFilter.set(seasonNumber ?? null);
    this.selectedIds.set(new Set());
    this.isOpen.set(true);
    await this.loadPreview(seriesId);
  }

  close(): void {
    this.isOpen.set(false);
    this.previews.set([]);
    this.selectedIds.set(new Set());
  }

  private async loadPreview(seriesId: number): Promise<void> {
    this.isLoading.set(true);
    try {
      const allItems = await http.get<RenamePreviewItem[]>('/rename', {
        params: { seriesId },
      });
      // Filter to target season when set
      const sn = this.seasonFilter.value;
      const items = sn !== null ? allItems.filter((i) => i.seasonNumber === sn) : allItems;
      this.previews.set(items);
      // Select all by default
      this.selectedIds.set(new Set(items.map((i) => i.episodeFileId)));
    } catch {
      showError('Failed to load rename preview');
      this.previews.set([]);
    } finally {
      this.isLoading.set(false);
    }
  }

  toggleItem(fileId: number): void {
    const current = new Set(this.selectedIds.value);
    if (current.has(fileId)) {
      current.delete(fileId);
    } else {
      current.add(fileId);
    }
    this.selectedIds.set(current);
  }

  toggleAll(): void {
    const items = this.previews.value;
    const selected = this.selectedIds.value;
    if (selected.size === items.length) {
      this.selectedIds.set(new Set());
    } else {
      this.selectedIds.set(new Set(items.map((i) => i.episodeFileId)));
    }
  }

  renameSelected(): void {
    const id = this.seriesId.value;
    if (!id) return;

    const selected = this.selectedIds.value;
    const files = this.previews.value
      .filter((p) => selected.has(p.episodeFileId))
      .map((p) => ({ episodeFileId: p.episodeFileId, newPath: p.newPath }));

    if (files.length === 0) return;

    this.renameMutation.mutate({ seriesId: id, files });
  }

  renameSingle(fileId: number): void {
    const id = this.seriesId.value;
    if (!id) return;

    const item = this.previews.value.find((p) => p.episodeFileId === fileId);
    if (!item) return;

    this.renameMutation.mutate({
      seriesId: id,
      files: [{ episodeFileId: item.episodeFileId, newPath: item.newPath }],
    });
  }

  private filenameOnly(fullPath: string): string {
    const parts = fullPath.split('/');
    return parts[parts.length - 1] ?? fullPath;
  }

  private relativeToSeries(fullPath: string): string {
    // Show last 2 path components: Season XX/filename.ext
    const parts = fullPath.split('/');
    if (parts.length >= 2) {
      return `${parts[parts.length - 2]}/${parts[parts.length - 1]}`;
    }
    return this.filenameOnly(fullPath);
  }

  protected template(): string {
    if (!this.isOpen.value) return '';

    const items = this.previews.value;
    const loading = this.isLoading.value;
    const selected = this.selectedIds.value;
    const isRenaming = this.renameMutation.isLoading.value;
    const title = this.seriesTitle.value;
    const sn = this.seasonFilter.value;
    const seasonSuffix = sn !== null ? (sn === 0 ? ' - Specials' : ` - Season ${sn}`) : '';

    return html`
      <div class="rename-backdrop">
        <div class="rename-dialog" role="dialog" aria-modal="true">
          <div class="rename-header">
            <h2>Organize Files - ${escapeHtml(title)}${seasonSuffix}</h2>
            <button class="close-btn" onclick="this.closest('episode-rename-dialog').close()" aria-label="Close">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="rename-body">
            ${
              loading
                ? html`
                <div class="rename-loading">
                  <div class="loading-spinner"></div>
                  <span>Loading rename preview...</span>
                </div>
              `
                : items.length === 0
                  ? html`
                  <div class="rename-empty">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                      <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
                      <polyline points="22 4 12 14.01 9 11.01"></polyline>
                    </svg>
                    <p>All files are already properly named.</p>
                  </div>
                `
                  : html`
                  <div class="rename-toolbar">
                    <label class="select-all">
                      <input
                        type="checkbox"
                        ${selected.size === items.length ? 'checked' : ''}
                        onclick="this.closest('episode-rename-dialog').toggleAll()"
                      />
                      Select All (${selected.size}/${items.length})
                    </label>
                    <button
                      class="rename-btn primary"
                      ${selected.size === 0 || isRenaming ? 'disabled' : ''}
                      onclick="this.closest('episode-rename-dialog').renameSelected()"
                    >
                      ${isRenaming ? 'Renaming...' : `Rename Selected (${selected.size})`}
                    </button>
                  </div>

                  <div class="rename-list">
                    ${items
                      .map((item) => {
                        const isSelected = selected.has(item.episodeFileId);
                        const epLabel =
                          item.episodeNumbers.length === 1
                            ? `S${String(item.seasonNumber).padStart(2, '0')}E${String(item.episodeNumbers[0]).padStart(2, '0')}`
                            : `S${String(item.seasonNumber).padStart(2, '0')}E${String(item.episodeNumbers[0]).padStart(2, '0')}-E${String(item.episodeNumbers[item.episodeNumbers.length - 1]).padStart(2, '0')}`;

                        return html`
                        <div class="rename-item ${isSelected ? 'selected' : ''}">
                          <div class="rename-item-header">
                            <input
                              type="checkbox"
                              ${isSelected ? 'checked' : ''}
                              onclick="event.stopPropagation(); this.closest('episode-rename-dialog').toggleItem(${item.episodeFileId})"
                            />
                            <span class="rename-ep-label">${epLabel}</span>
                            <button
                              class="rename-single-btn"
                              ${isRenaming ? 'disabled' : ''}
                              onclick="event.stopPropagation(); this.closest('episode-rename-dialog').renameSingle(${item.episodeFileId})"
                              title="Rename this file"
                            >
                              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <polyline points="20 6 9 17 4 12"></polyline>
                              </svg>
                            </button>
                          </div>
                          <div class="rename-paths">
                            <div class="rename-path current">
                              <span class="path-label">Current</span>
                              <span class="path-value">${escapeHtml(this.relativeToSeries(item.existingPath))}</span>
                            </div>
                            <div class="rename-arrow">
                              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                                <line x1="5" y1="12" x2="19" y2="12"></line>
                                <polyline points="12 5 19 12 12 19"></polyline>
                              </svg>
                            </div>
                            <div class="rename-path new">
                              <span class="path-label">New</span>
                              <span class="path-value">${escapeHtml(this.relativeToSeries(item.newPath))}</span>
                            </div>
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

      <style>
        .rename-backdrop {
          position: fixed;
          inset: 0;
          z-index: 1000;
          display: flex;
          align-items: center;
          justify-content: center;
          background-color: rgba(0, 0, 0, 0.6);
        }

        .rename-dialog {
          width: min(900px, 95vw);
          max-height: 85vh;
          display: flex;
          flex-direction: column;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
          box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
        }

        .rename-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem 1.25rem;
          border-bottom: 1px solid var(--border-color);
        }

        .rename-header h2 {
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

        .rename-body {
          flex: 1;
          overflow-y: auto;
          padding: 1rem 1.25rem;
        }

        .rename-loading {
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

        .rename-empty {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1rem;
          padding: 3rem;
          color: var(--color-success);
          text-align: center;
        }

        .rename-empty p {
          margin: 0;
          font-size: 1rem;
        }

        .rename-toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 1rem;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .select-all {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          font-size: 0.875rem;
          color: var(--text-color-muted);
          cursor: pointer;
        }

        .select-all input {
          accent-color: var(--color-primary);
        }

        .rename-btn {
          padding: 0.5rem 1rem;
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          font-size: 0.875rem;
          cursor: pointer;
          background-color: var(--btn-default-bg);
          color: var(--text-color);
        }

        .rename-btn.primary {
          background-color: var(--btn-primary-bg);
          border-color: var(--btn-primary-border);
          color: var(--color-white);
        }

        .rename-btn.primary:hover:not(:disabled) {
          background-color: var(--btn-primary-bg-hover);
        }

        .rename-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .rename-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .rename-item {
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          overflow: hidden;
          transition: border-color 0.15s;
        }

        .rename-item.selected {
          border-color: var(--color-primary);
        }

        .rename-item-header {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.5rem 0.75rem;
          background-color: var(--bg-card-alt);
        }

        .rename-item-header input {
          accent-color: var(--color-primary);
        }

        .rename-ep-label {
          font-weight: 600;
          font-size: 0.875rem;
          flex: 1;
        }

        .rename-single-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.25rem 0.5rem;
          background-color: var(--color-success);
          border: none;
          border-radius: 0.25rem;
          color: var(--color-white);
          cursor: pointer;
          font-size: 0.75rem;
        }

        .rename-single-btn:hover:not(:disabled) {
          opacity: 0.9;
        }

        .rename-single-btn:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .rename-paths {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
          padding: 0.5rem 0.75rem;
        }

        .rename-path {
          display: flex;
          align-items: baseline;
          gap: 0.5rem;
          font-size: 0.8125rem;
          font-family: monospace;
          line-height: 1.4;
        }

        .path-label {
          font-family: inherit;
          font-size: 0.6875rem;
          text-transform: uppercase;
          font-weight: 600;
          color: var(--text-color-muted);
          min-width: 50px;
        }

        .rename-path.current .path-value {
          color: var(--color-danger);
          text-decoration: line-through;
          opacity: 0.7;
        }

        .rename-path.new .path-value {
          color: var(--color-success);
        }

        .rename-arrow {
          display: flex;
          justify-content: center;
          color: var(--text-color-muted);
          padding: 0 0 0 50px;
        }
      </style>
    `;
  }
}
