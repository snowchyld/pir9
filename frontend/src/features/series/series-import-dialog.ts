/**
 * Series manual import dialog — scan the series directory for untracked files
 * and allow manual episode assignment, similar to the queue import preview.
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

interface ManualImportFile {
  path: string;
  name: string;
  size: number;
  relativePath: string;
  seasonNumber: number | null;
  episodeNumbers: number[];
  tracked: boolean;
  episodeFileId: number | null;
}

interface ManualImportEpisode {
  id: number;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  hasFile: boolean;
}

interface ManualImportPreview {
  seriesId: number;
  seriesTitle: string;
  seriesPath: string;
  files: ManualImportFile[];
  episodes: ManualImportEpisode[];
}

type FilterMode = 'untracked' | 'all';

@customElement('series-import-dialog')
export class SeriesImportDialog extends BaseComponent {
  private isOpen = signal(false);
  private loading = signal(false);
  private preview = signal<ManualImportPreview | null>(null);
  private filterMode = signal<FilterMode>('untracked');
  private importing = signal(false);

  /** Which file is currently being edited for episode assignment */
  private editingFile = signal<string | null>(null);

  /** Manual episode overrides: filePath → { seasonNumber, episodeNumbers[] } */
  private overrides = new Map<string, { seasonNumber: number; episodeNumbers: number[] }>();

  /** Files the user chose to skip */
  private skippedFiles = signal<Set<string>>(new Set());

  private importMutation = createMutation({
    mutationFn: (params: {
      seriesId: number;
      imports: Array<{ path: string; seasonNumber: number; episodeNumbers: number[] }>;
    }) =>
      http.post<{ success: boolean; imported: number; linked: number }>(
        `/series/${params.seriesId}/manualimport`,
        { imports: params.imports },
      ),
    onSuccess: (result) => {
      this.importing.set(false);
      if (result.success) {
        showSuccess(`Imported ${result.imported} files, linked ${result.linked} episodes`);
        invalidateQueries(['/series']);
        invalidateQueries(['/episode']);
        invalidateQueries(['/wanted']);
        this.close();
      } else {
        showError('Import failed');
      }
    },
    onError: () => {
      this.importing.set(false);
      showError('Failed to import files');
    },
  });

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.loading);
    this.watch(this.preview);
    this.watch(this.filterMode);
    this.watch(this.importing);
    this.watch(this.editingFile);
    this.watch(this.skippedFiles);
  }

  open(seriesId: number): void {
    this.isOpen.set(true);
    this.overrides.clear();
    this.skippedFiles.set(new Set());
    this.editingFile.set(null);
    this.fetchPreview(seriesId);
  }

  close(): void {
    this.isOpen.set(false);
    this.preview.set(null);
    this.overrides.clear();
    this.dispatchEvent(new CustomEvent('dialog-closed', { bubbles: true }));
  }

  private async fetchPreview(seriesId: number): Promise<void> {
    this.loading.set(true);
    try {
      const data = await http.get<ManualImportPreview>(`/series/${seriesId}/manualimport`);
      this.preview.set(data);
    } catch {
      showError('Failed to scan series directory');
    } finally {
      this.loading.set(false);
    }
  }

  protected template(): string {
    if (!this.isOpen.value) return '';

    const preview = this.preview.value;
    const isLoading = this.loading.value;
    const isImporting = this.importing.value;

    return html`
      <div class="import-backdrop" onclick="if(event.target===this)this.querySelector('series-import-dialog')?.close()">
        <div class="import-dialog">
          <div class="import-header">
            <h2>Import Files${preview ? ` - ${escapeHtml(preview.seriesTitle)}` : ''}</h2>
            <button class="close-btn" onclick="this.closest('series-import-dialog').close()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="import-body">
            ${isLoading ? html`<div class="loading-area"><div class="loading-spinner"></div><span>Scanning directory...</span></div>` : ''}
            ${!isLoading && preview ? safeHtml(this.renderPreview(preview, isImporting)) : ''}
          </div>
        </div>
      </div>

      <style>${this.getStyles()}</style>
    `;
  }

  private renderPreview(preview: ManualImportPreview, isImporting: boolean): string {
    const filter = this.filterMode.value;
    const skipped = this.skippedFiles.value;

    let files = preview.files;
    if (filter === 'untracked') {
      files = files.filter((f) => !f.tracked);
    }

    const importableFiles = files.filter((f) => !f.tracked && !skipped.has(f.path));
    const matchedCount = importableFiles.filter((f) => this.getFileMatch(f) !== null).length;

    // Group episodes by season for the editor
    const seasons = [...new Set(preview.episodes.map((e) => e.seasonNumber))].sort((a, b) => a - b);

    return html`
      <div class="import-toolbar">
        <div class="filter-tabs">
          <button class="filter-tab ${filter === 'untracked' ? 'active' : ''}"
            onclick="this.closest('series-import-dialog').setFilter('untracked')">
            Untracked (${preview.files.filter((f) => !f.tracked).length})
          </button>
          <button class="filter-tab ${filter === 'all' ? 'active' : ''}"
            onclick="this.closest('series-import-dialog').setFilter('all')">
            All Files (${preview.files.length})
          </button>
        </div>
        <div class="import-actions">
          <span class="import-count">${matchedCount} files ready</span>
          <button class="import-btn" onclick="this.closest('series-import-dialog').handleImport()"
            ${matchedCount === 0 || isImporting ? 'disabled' : ''}>
            ${isImporting ? 'Importing...' : `Import ${matchedCount} Files`}
          </button>
        </div>
      </div>

      <div class="import-path">
        <span class="path-label">Path:</span>
        <span class="path-value">${escapeHtml(preview.seriesPath)}</span>
      </div>

      ${
        files.length === 0
          ? html`<div class="empty-msg">No ${filter === 'untracked' ? 'untracked' : ''} files found in series directory</div>`
          : html`
        <table class="import-table">
          <thead>
            <tr>
              <th>File</th>
              <th>Size</th>
              <th>Episode</th>
              <th>Status</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            ${files.map((f) => this.renderFileRow(f, preview, seasons, skipped)).join('')}
          </tbody>
        </table>
      `
      }
    `;
  }

  private renderFileRow(
    file: ManualImportFile,
    preview: ManualImportPreview,
    seasons: number[],
    skipped: Set<string>,
  ): string {
    const match = this.getFileMatch(file);
    const isEditing = this.editingFile.value === file.path;
    const isSkipped = skipped.has(file.path);

    let statusClass = 'unmatched';
    let statusLabel = 'Unmatched';
    if (file.tracked) {
      statusClass = 'tracked';
      statusLabel = 'Tracked';
    } else if (isSkipped) {
      statusClass = 'skipped';
      statusLabel = 'Skipped';
    } else if (match) {
      statusClass = 'ready';
      statusLabel = 'Ready';
    }

    const episodeCell = isEditing
      ? this.renderEpisodeEditor(file, preview, seasons)
      : this.renderEpisodeDisplay(file, match);

    return html`
      <tr class="file-row ${statusClass} ${isSkipped ? 'dim' : ''}">
        <td class="file-name" title="${escapeHtml(file.relativePath)}">${escapeHtml(file.name)}</td>
        <td class="file-size">${this.formatBytes(file.size)}</td>
        <td class="file-episode">${safeHtml(episodeCell)}</td>
        <td><span class="status-badge ${statusClass}">${statusLabel}</span></td>
        <td class="file-actions">
          ${
            file.tracked
              ? ''
              : html`
            ${
              !isSkipped
                ? html`
              <button class="action-sm" onclick="this.closest('series-import-dialog').editFile('${this.escPath(file.path)}')" title="Edit episode">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                  <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                </svg>
              </button>
              <button class="action-sm skip" onclick="this.closest('series-import-dialog').skipFile('${this.escPath(file.path)}')" title="Skip">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle><line x1="4.93" y1="4.93" x2="19.07" y2="19.07"></line>
                </svg>
              </button>
            `
                : html`
              <button class="action-sm" onclick="this.closest('series-import-dialog').unskipFile('${this.escPath(file.path)}')" title="Unskip">Unskip</button>
            `
            }
          `
          }
        </td>
      </tr>
    `;
  }

  private renderEpisodeDisplay(
    file: ManualImportFile,
    match: { seasonNumber: number; episodeNumbers: number[] } | null,
  ): string {
    const clickable = !file.tracked;
    const onclick = clickable
      ? `onclick="this.closest('series-import-dialog').editFile('${this.escPath(file.path)}')"`
      : '';
    if (!match) {
      return html`<span class="no-match ep-clickable" ${onclick}>Click to assign</span>`;
    }
    const isOverride = this.overrides.has(file.path);
    const epLabel = match.episodeNumbers.map((n) => `E${String(n).padStart(2, '0')}`).join('');
    return html`
      <span class="ep-match ${isOverride ? 'manual' : ''} ${clickable ? 'ep-clickable' : ''}" ${onclick}>
        S${String(match.seasonNumber).padStart(2, '0')}${epLabel}
        ${isOverride ? html`<span class="manual-badge">Manual</span>` : ''}
      </span>
    `;
  }

  private renderEpisodeEditor(
    file: ManualImportFile,
    preview: ManualImportPreview,
    seasons: number[],
  ): string {
    const current = this.overrides.get(file.path) ?? {
      seasonNumber: file.seasonNumber ?? seasons[0] ?? 1,
      episodeNumbers: file.episodeNumbers,
    };

    const seasonEpisodes = preview.episodes.filter((e) => e.seasonNumber === current.seasonNumber);

    return html`
      <div class="ep-editor">
        <select class="ep-select" onchange="this.closest('series-import-dialog').setEditSeason('${this.escPath(file.path)}', Number(this.value))">
          ${seasons.map((s) => html`<option value="${s}" ${s === current.seasonNumber ? 'selected' : ''}>Season ${s}</option>`).join('')}
        </select>
        <select class="ep-select ep-multi" multiple size="4"
          onchange="this.closest('series-import-dialog').setEditEpisodes('${this.escPath(file.path)}', Array.from(this.selectedOptions).map(o => Number(o.value)))">
          ${seasonEpisodes
            .map((e) => {
              const sel = current.episodeNumbers.includes(e.episodeNumber) ? 'selected' : '';
              const hasFile = e.hasFile ? ' (has file)' : '';
              return html`<option value="${e.episodeNumber}" ${sel}>E${String(e.episodeNumber).padStart(2, '0')} - ${escapeHtml(e.title)}${hasFile}</option>`;
            })
            .join('')}
        </select>
        <div class="ep-editor-actions">
          <button class="action-sm confirm" onclick="this.closest('series-import-dialog').confirmEdit('${this.escPath(file.path)}')">Confirm</button>
          <button class="action-sm" onclick="this.closest('series-import-dialog').cancelEdit()">Cancel</button>
        </div>
      </div>
    `;
  }

  // --- Helpers ---

  private getFileMatch(
    file: ManualImportFile,
  ): { seasonNumber: number; episodeNumbers: number[] } | null {
    if (this.overrides.has(file.path)) {
      return this.overrides.get(file.path)!;
    }
    if (file.seasonNumber !== null && file.episodeNumbers.length > 0) {
      return { seasonNumber: file.seasonNumber, episodeNumbers: file.episodeNumbers };
    }
    return null;
  }

  /** Escape file path for inline onclick handlers */
  private escPath(path: string): string {
    return path.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
  }

  // --- Event handlers ---

  setFilter(mode: FilterMode): void {
    this.filterMode.set(mode);
  }

  editFile(path: string): void {
    this.editingFile.set(path);
  }

  cancelEdit(): void {
    this.editingFile.set(null);
  }

  setEditSeason(path: string, season: number): void {
    const current = this.overrides.get(path) ?? { seasonNumber: season, episodeNumbers: [] };
    this.overrides.set(path, { seasonNumber: season, episodeNumbers: [] });
    this.requestUpdate();
  }

  setEditEpisodes(path: string, episodes: number[]): void {
    const current = this.overrides.get(path);
    if (current) {
      current.episodeNumbers = episodes;
    } else {
      this.overrides.set(path, { seasonNumber: 1, episodeNumbers: episodes });
    }
  }

  confirmEdit(path: string): void {
    const override = this.overrides.get(path);
    if (!override || override.episodeNumbers.length === 0) {
      showError('Select at least one episode');
      return;
    }
    this.editingFile.set(null);
  }

  skipFile(path: string): void {
    const s = new Set(this.skippedFiles.value);
    s.add(path);
    this.skippedFiles.set(s);
  }

  unskipFile(path: string): void {
    const s = new Set(this.skippedFiles.value);
    s.delete(path);
    this.skippedFiles.set(s);
  }

  handleImport(): void {
    const preview = this.preview.value;
    if (!preview) return;

    const skipped = this.skippedFiles.value;
    const imports: Array<{ path: string; seasonNumber: number; episodeNumbers: number[] }> = [];

    for (const file of preview.files) {
      if (file.tracked || skipped.has(file.path)) continue;
      const match = this.getFileMatch(file);
      if (match) {
        imports.push({
          path: file.path,
          seasonNumber: match.seasonNumber,
          episodeNumbers: match.episodeNumbers,
        });
      }
    }

    if (imports.length === 0) {
      showError('No files to import');
      return;
    }

    this.importing.set(true);
    this.importMutation.mutate({ seriesId: preview.seriesId, imports });
  }

  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  private getStyles(): string {
    return `
      .import-backdrop {
        position: fixed; inset: 0; z-index: 1000; display: flex;
        align-items: center; justify-content: center; background: rgba(0,0,0,0.6);
      }
      .import-dialog {
        width: min(1100px, 95vw); max-height: 85vh; display: flex; flex-direction: column;
        background: var(--bg-card); border: 1px solid var(--border-color);
        border-radius: 0.5rem; box-shadow: 0 20px 60px rgba(0,0,0,0.3);
      }
      .import-header {
        display: flex; align-items: center; justify-content: space-between;
        padding: 1rem 1.25rem; border-bottom: 1px solid var(--border-color);
      }
      .import-header h2 { margin: 0; font-size: 1.125rem; font-weight: 600; }
      .close-btn {
        display: flex; padding: 0.25rem; background: transparent; border: none;
        border-radius: 0.25rem; color: var(--text-color-muted); cursor: pointer;
      }
      .close-btn:hover { color: var(--text-color); background: var(--bg-input-hover); }
      .import-body { flex: 1; overflow-y: auto; padding: 1rem 1.25rem; display: flex; flex-direction: column; gap: 0.75rem; }

      .loading-area { display: flex; flex-direction: column; align-items: center; gap: 1rem; padding: 3rem; color: var(--text-color-muted); }
      .loading-spinner { width: 32px; height: 32px; border: 3px solid var(--border-color); border-top-color: var(--color-primary); border-radius: 50%; animation: spin 0.8s linear infinite; }
      @keyframes spin { to { transform: rotate(360deg); } }

      .import-toolbar { display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 0.5rem; }
      .filter-tabs { display: flex; gap: 0.25rem; }
      .filter-tab {
        padding: 0.375rem 0.75rem; background: var(--bg-card-alt); border: 1px solid var(--border-color);
        border-radius: 0.375rem; color: var(--text-color-muted); font-size: 0.8125rem; cursor: pointer;
      }
      .filter-tab.active { background: var(--color-primary); border-color: var(--color-primary); color: white; }
      .import-actions { display: flex; align-items: center; gap: 0.75rem; }
      .import-count { font-size: 0.8125rem; color: var(--text-color-muted); }
      .import-btn {
        padding: 0.5rem 1rem; background: var(--btn-primary-bg); border: 1px solid var(--btn-primary-border);
        border-radius: 0.25rem; color: white; font-size: 0.875rem; cursor: pointer;
      }
      .import-btn:hover:not(:disabled) { background: var(--btn-primary-bg-hover); }
      .import-btn:disabled { opacity: 0.5; cursor: not-allowed; }

      .import-path { font-size: 0.75rem; color: var(--text-color-muted); }
      .path-label { font-weight: 500; }
      .path-value { font-family: monospace; }

      .empty-msg { text-align: center; padding: 2rem; color: var(--text-color-muted); }

      .import-table { width: 100%; border-collapse: collapse; font-size: 0.8125rem; }
      .import-table th { padding: 0.5rem 0.75rem; text-align: left; font-weight: 600; color: var(--text-color-muted); background: var(--bg-card-alt); border-bottom: 1px solid var(--border-color); white-space: nowrap; }
      .import-table td { padding: 0.5rem 0.75rem; border-bottom: 1px solid var(--border-color); }
      .file-row.dim td { opacity: 0.4; }
      .file-name { max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
      .file-size { white-space: nowrap; color: var(--text-color-muted); }

      .status-badge {
        display: inline-flex; padding: 0.125rem 0.5rem; font-size: 0.6875rem; font-weight: 600;
        border-radius: 0.25rem; text-transform: uppercase;
      }
      .status-badge.ready { background: rgba(92,184,92,0.15); color: #5cb85c; }
      .status-badge.tracked { background: rgba(93,156,236,0.15); color: var(--color-primary); }
      .status-badge.unmatched { background: rgba(240,173,78,0.15); color: #f0ad4e; }
      .status-badge.skipped { background: rgba(150,150,150,0.15); color: var(--text-color-muted); }

      .no-match { color: var(--text-color-muted); }
      .ep-clickable { cursor: pointer; padding: 0.125rem 0.375rem; border-radius: 0.25rem; }
      .ep-clickable:hover { background: rgba(93,156,236,0.15); color: var(--color-primary); }
      .ep-match { font-family: monospace; font-size: 0.8125rem; }
      .ep-match.manual { color: var(--color-primary); }
      .manual-badge {
        margin-left: 0.375rem; padding: 0.0625rem 0.25rem; font-size: 0.5625rem; font-weight: 600;
        background: var(--color-primary); color: white; border-radius: 0.125rem; text-transform: uppercase;
      }

      .ep-editor { display: flex; flex-direction: column; gap: 0.375rem; }
      .ep-select {
        padding: 0.25rem 0.5rem; background: var(--bg-input); border: 1px solid var(--border-color);
        border-radius: 0.25rem; color: var(--text-color); font-size: 0.75rem;
      }
      .ep-select:focus { outline: none; border-color: var(--color-primary); }
      .ep-multi { min-height: 80px; }
      .ep-editor-actions { display: flex; gap: 0.25rem; }

      .file-actions { white-space: nowrap; }
      .action-sm {
        padding: 0.25rem 0.375rem; background: transparent; border: 1px solid var(--border-color);
        border-radius: 0.25rem; color: var(--text-color-muted); font-size: 0.6875rem; cursor: pointer;
      }
      .action-sm:hover { color: var(--text-color); border-color: var(--text-color-muted); }
      .action-sm.confirm { background: var(--color-primary); border-color: var(--color-primary); color: white; }
      .action-sm.skip:hover { color: var(--color-danger); border-color: var(--color-danger); }
    `;
  }
}
