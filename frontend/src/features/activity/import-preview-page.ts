/**
 * Import preview page — shows what files are in a download
 * and where they'll be imported to before committing.
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

interface ImportPreviewFile {
  sourceFile: string;
  sourceSize: number;
  seasonNumber: number | null;
  episodeNumber: number | null;
  episodeTitle: string | null;
  destinationPath: string | null;
  matched: boolean;
  existingFile: boolean;
  existingFileSize?: number | null;
}

interface ImportPreviewEpisode {
  id: number;
  seasonNumber: number;
  episodeNumber: number;
  title: string;
  hasFile: boolean;
  fileSize?: number | null;
}

interface ImportPreviewSeries {
  id: number;
  title: string;
  path: string;
}

interface ImportPreviewMovie {
  id: number;
  title: string;
  path: string;
}

interface ImportPreviewResponse {
  id: number;
  title: string;
  contentType: string;
  series: ImportPreviewSeries | null;
  movie: ImportPreviewMovie | null;
  outputPath: string;
  files: ImportPreviewFile[];
  episodes?: ImportPreviewEpisode[];
}

const VIDEO_EXTENSIONS = new Set([
  'mkv',
  'mp4',
  'avi',
  'wmv',
  'm4v',
  'ts',
  'webm',
  'mov',
  'flv',
  'mpg',
  'mpeg',
  'vob',
  'ogm',
  'divx',
  'm2ts',
  'mts',
]);

function isVideoFile(filename: string): boolean {
  const ext = filename.split('.').pop()?.toLowerCase() ?? '';
  return VIDEO_EXTENSIONS.has(ext);
}

@customElement('import-preview-page')
export class ImportPreviewPage extends BaseComponent {
  private preview = signal<ImportPreviewResponse | null>(null);
  private loading = signal(true);
  private error = signal<string | null>(null);
  private importing = signal(false);
  /** Manual episode overrides: sourceFile -> { seasonNumber, episodeNumber } */
  private manualOverrides = new Map<string, { seasonNumber: number; episodeNumber: number }>();

  private importMutation = createMutation({
    mutationFn: (params: {
      id: number;
      overrides: Record<string, { seasonNumber: number; episodeNumber: number }>;
    }) => {
      const body =
        Object.keys(params.overrides).length > 0 ? { overrides: params.overrides } : undefined;
      return http.post<{ success: boolean }>(`/queue/${params.id}/import`, body);
    },
    onSuccess: (result: { success: boolean }) => {
      this.importing.set(false);
      if (result.success) {
        invalidateQueries(['/queue']);
        showSuccess('Download imported to library');
        navigate('/activity/queue');
      } else {
        showError('Import failed — could not match series or episodes');
      }
    },
    onError: () => {
      this.importing.set(false);
      showError('Failed to import download');
    },
  });

  protected onInit(): void {
    this.watch(this.preview);
    this.watch(this.loading);
    this.watch(this.error);
    this.watch(this.importing);
  }

  protected onMount(): void {
    const id = this.getAttribute('id');
    if (id) {
      this.fetchPreview(Number(id));
    } else {
      this.loading.set(false);
      this.error.set('No queue item ID provided');
    }
  }

  private async fetchPreview(id: number): Promise<void> {
    try {
      const data = await http.get<ImportPreviewResponse>(`/queue/${id}/import-preview`);
      this.preview.set(data);
    } catch {
      this.error.set('Failed to load import preview');
    } finally {
      this.loading.set(false);
    }
  }

  protected template(): string {
    const isLoading = this.loading.value;
    const errorMsg = this.error.value;
    const data = this.preview.value;
    const isImporting = this.importing.value;

    if (isLoading) {
      return html`
        <div class="preview-page">
          <div class="loading-container">
            <div class="loading-spinner"></div>
            <p>Loading import preview...</p>
          </div>
          ${safeHtml(this.styles())}
        </div>
      `;
    }

    if (errorMsg || !data) {
      return html`
        <div class="preview-page">
          <div class="error-container">
            <p>${escapeHtml(errorMsg ?? 'Unknown error')}</p>
            <button class="btn btn-default" onclick="this.closest('import-preview-page').handleBack()">
              Back to Queue
            </button>
          </div>
          ${safeHtml(this.styles())}
        </div>
      `;
    }

    const contentTitle = data.series?.title ?? data.movie?.title ?? data.title;
    const rootPath = data.series?.path ?? data.movie?.path ?? '';
    const hasExisting = data.files.some(
      (f) => f.existingFile || this.getOverrideHasFile(f.sourceFile),
    );
    const sameFileCount = data.files.filter((f) => {
      const isMatched = f.matched || this.manualOverrides.has(f.sourceFile);
      if (!isMatched) return false;
      const hasExisting = f.existingFile || this.getOverrideHasFile(f.sourceFile);
      if (!hasExisting) return false;
      const existingSize = this.getEffectiveExistingSize(f);
      return existingSize != null && existingSize === f.sourceSize;
    }).length;
    const upgradeCount = data.files.filter((f) => {
      const isMatched = f.matched || this.manualOverrides.has(f.sourceFile);
      if (!isMatched) return false;
      const hasExisting = f.existingFile || this.getOverrideHasFile(f.sourceFile);
      if (!hasExisting) return false;
      const existingSize = this.getEffectiveExistingSize(f);
      return existingSize == null || existingSize !== f.sourceSize;
    }).length;
    const matchedCount = this.getMatchedCount(data);
    const unmatchedCount = data.files.length - sameFileCount - matchedCount;
    const totalSize = data.files
      .filter((f) => {
        const isMatched = f.matched || this.manualOverrides.has(f.sourceFile);
        if (!isMatched) return false;
        const hasExisting = f.existingFile || this.getOverrideHasFile(f.sourceFile);
        const existingSize = this.getEffectiveExistingSize(f);
        if (hasExisting && existingSize != null && existingSize === f.sourceSize) {
          return false;
        }
        return true;
      })
      .reduce((sum, f) => sum + f.sourceSize, 0);

    return html`
      <div class="preview-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <button class="back-btn" onclick="this.closest('import-preview-page').handleBack()" title="Back to Queue">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="15 18 9 12 15 6"></polyline>
              </svg>
            </button>
            <div>
              <h1 class="page-title">Import Preview</h1>
              <div class="subtitle">${escapeHtml(contentTitle)}</div>
            </div>
          </div>
        </div>

        <div class="info-cards">
          <div class="info-card">
            <div class="info-label">Release</div>
            <div class="info-value" title="${escapeHtml(data.title)}">${escapeHtml(data.title)}</div>
          </div>
          <div class="info-card">
            <div class="info-label">Download Path</div>
            <div class="info-value" title="${escapeHtml(data.outputPath)}">${escapeHtml(data.outputPath)}</div>
          </div>
          <div class="info-card">
            <div class="info-label">Destination</div>
            <div class="info-value" title="${escapeHtml(rootPath)}">${escapeHtml(rootPath)}</div>
          </div>
          <div class="info-card">
            <div class="info-label">Files</div>
            <div class="info-value">${matchedCount} matched${unmatchedCount > 0 ? ` · ${unmatchedCount} skipped` : ''}${sameFileCount > 0 ? ` · ${sameFileCount} identical` : ''} · ${this.formatSize(totalSize)}</div>
          </div>
        </div>

        ${hasExisting ? `<div class="warning-banner"><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg> Some episodes already have files${upgradeCount > 0 ? ` — ${upgradeCount} will be upgraded` : ''}${sameFileCount > 0 ? ` — ${sameFileCount} identical (same size), will be skipped` : ''}</div>` : ''}

        <table class="file-table">
          <colgroup>
            <col style="width: 30%">
            <col style="width: 10%">
            <col style="width: 18%">
            <col style="width: 30%">
            <col style="width: 12%">
          </colgroup>
          <thead>
            <tr>
              <th>Source File</th>
              <th>Size</th>
              <th>Episode</th>
              <th>Destination</th>
              <th>Status</th>
            </tr>
          </thead>
          <tbody>
            ${data.files.map((f) => this.renderFileRow(f, data)).join('')}
          </tbody>
        </table>

        <div class="footer">
          <button class="btn btn-default" onclick="this.closest('import-preview-page').handleBack()">
            Back to Queue
          </button>
          <button
            class="btn btn-primary"
            onclick="this.closest('import-preview-page').handleImport()"
            ${isImporting || matchedCount === 0 ? 'disabled' : ''}
          >
            ${isImporting ? 'Importing...' : `Import ${matchedCount} File${matchedCount !== 1 ? 's' : ''}`}
          </button>
        </div>

        ${safeHtml(this.styles())}
      </div>
    `;
  }

  private getMatchedCount(data: ImportPreviewResponse): number {
    return data.files.filter((f) => {
      const isMatched = f.matched || this.manualOverrides.has(f.sourceFile);
      if (!isMatched) return false;
      // Exclude same-size files (identical, no upgrade needed)
      const hasExisting = f.existingFile || this.getOverrideHasFile(f.sourceFile);
      const existingSize = this.getEffectiveExistingSize(f);
      if (hasExisting && existingSize != null && existingSize === f.sourceSize) {
        return false;
      }
      return true;
    }).length;
  }

  /** Get the existing file size — uses episode fileSize for manual overrides, backend value otherwise */
  private getEffectiveExistingSize(file: ImportPreviewFile): number | null {
    const override = this.manualOverrides.get(file.sourceFile);
    if (override) {
      const ep = this.preview.value?.episodes?.find(
        (e) =>
          e.seasonNumber === override.seasonNumber && e.episodeNumber === override.episodeNumber,
      );
      return ep?.fileSize ?? null;
    }
    return file.existingFileSize ?? null;
  }

  private getOverrideHasFile(sourceFile: string): boolean {
    const ov = this.manualOverrides.get(sourceFile);
    if (!ov) return false;
    const ep = this.preview.value?.episodes?.find(
      (e) => e.seasonNumber === ov.seasonNumber && e.episodeNumber === ov.episodeNumber,
    );
    return ep?.hasFile ?? false;
  }

  private renderFileRow(file: ImportPreviewFile, data: ImportPreviewResponse): string {
    const filename = file.sourceFile.split('/').pop() ?? file.sourceFile;
    const override = this.manualOverrides.get(file.sourceFile);
    const isManuallyMatched = !!override;
    const effectivelyMatched = file.matched || isManuallyMatched;

    const seasonNum = override?.seasonNumber ?? file.seasonNumber;
    const episodeNum = override?.episodeNumber ?? file.episodeNumber;
    const overrideEp = override
      ? data.episodes?.find(
          (e) =>
            e.seasonNumber === override.seasonNumber && e.episodeNumber === override.episodeNumber,
        )
      : null;

    const episodeLabel =
      seasonNum != null && episodeNum != null
        ? `S${String(seasonNum).padStart(2, '0')}E${String(episodeNum).padStart(2, '0')}`
        : '-';
    const epTitle = overrideEp?.title ?? file.episodeTitle;
    const destFilename = file.destinationPath?.split('/').pop() ?? '';
    const hasExistingFile = isManuallyMatched ? (overrideEp?.hasFile ?? false) : file.existingFile;

    // For manually matched episodes, use the episode's file size; otherwise use the backend value
    const effectiveExistingSize = isManuallyMatched
      ? (overrideEp?.fileSize ?? null)
      : (file.existingFileSize ?? null);

    // Detect same-size files: if source size matches existing file size, it's likely identical
    const isSameFile =
      hasExistingFile && effectiveExistingSize != null && effectiveExistingSize === file.sourceSize;

    // Hide identical files entirely — they won't be imported
    if (effectivelyMatched && isSameFile) {
      return '';
    }

    let statusClass = 'status-skip';
    let statusLabel = 'Skipped';
    if (effectivelyMatched && hasExistingFile) {
      statusClass = 'status-upgrade';
      statusLabel = 'Upgrade';
    } else if (effectivelyMatched) {
      statusClass = 'status-ready';
      statusLabel = 'Ready';
    }

    // Show manual matching dropdowns for unmatched video files with available episodes
    const showManualMatch =
      !file.matched && isVideoFile(filename) && data.episodes && data.episodes.length > 0;
    const episodes = data.episodes ?? [];
    const seasons = [...new Set(episodes.map((e) => e.seasonNumber))].sort((a, b) => a - b);

    // Size display: source size on top, existing file size below when present
    const sizeHtml = effectiveExistingSize
      ? `${this.formatSize(file.sourceSize)}<div class="existing-size">${this.formatSize(effectiveExistingSize)} existing</div>`
      : this.formatSize(file.sourceSize);

    const episodeCellContent =
      showManualMatch && !isManuallyMatched
        ? this.renderManualMatchSelects(file.sourceFile, seasons, episodes)
        : `<div>${escapeHtml(episodeLabel)}</div>${epTitle ? `<div class="ep-title">${escapeHtml(epTitle)}</div>` : ''}`;

    const destContent = isManuallyMatched
      ? '<span class="manual-badge">Manual</span>'
      : escapeHtml(destFilename);

    return html`
      <tr class="${!effectivelyMatched ? 'row-skipped' : isSameFile ? 'row-same' : ''}">
        <td class="file-cell" title="${escapeHtml(file.sourceFile)}">
          ${escapeHtml(filename)}
        </td>
        <td>${safeHtml(sizeHtml)}</td>
        <td>
          ${safeHtml(episodeCellContent)}
        </td>
        <td class="dest-cell" title="${escapeHtml(file.destinationPath ?? '')}">
          ${safeHtml(destContent)}
        </td>
        <td>
          <span class="status-badge ${statusClass}">${statusLabel}</span>
        </td>
      </tr>
    `;
  }

  private renderManualMatchSelects(
    sourceFile: string,
    seasons: number[],
    episodes: ImportPreviewEpisode[],
  ): string {
    const escapedSourceFile = escapeHtml(sourceFile).replace(/'/g, "\\'");

    const seasonOptions = seasons
      .map((s) => {
        const label = s === 0 ? 'Specials' : `Season ${s}`;
        return `<option value="${s}">${escapeHtml(label)}</option>`;
      })
      .join('');

    // Default to first season's episodes
    const firstSeason = seasons[0] ?? 0;
    const filteredEps = episodes.filter((e) => e.seasonNumber === firstSeason);
    const episodeOptions = filteredEps
      .map((e) => {
        const label = `E${String(e.episodeNumber).padStart(2, '0')} - ${e.title}${e.hasFile ? ' (has file)' : ''}`;
        return `<option value="${e.episodeNumber}">${escapeHtml(label)}</option>`;
      })
      .join('');

    return `
      <div class="manual-match">
        <select class="match-select" onchange="this.closest('import-preview-page').handleSeasonChange('${escapedSourceFile}', this)">
          <option value="">Season...</option>
          ${seasonOptions}
        </select>
        <select class="match-select episode-select" data-source="${escapeHtml(sourceFile)}" onchange="this.closest('import-preview-page').handleEpisodeSelect('${escapedSourceFile}', this)">
          <option value="">Episode...</option>
          ${episodeOptions}
        </select>
      </div>
    `;
  }

  handleSeasonChange(_sourceFile: string, select: HTMLSelectElement): void {
    const data = this.preview.value;
    if (!data?.episodes) return;

    const seasonNum = Number(select.value);
    if (Number.isNaN(seasonNum)) return;

    // Rebuild episode options for the selected season
    const row = select.closest('tr');
    if (!row) return;
    const episodeSelect = row.querySelector('.episode-select') as HTMLSelectElement | null;
    if (!episodeSelect) return;

    const filteredEps = data.episodes.filter((e) => e.seasonNumber === seasonNum);
    // Clear and repopulate using DOM methods (safe, no innerHTML)
    while (episodeSelect.options.length > 0) {
      episodeSelect.remove(0);
    }
    const placeholder = document.createElement('option');
    placeholder.value = '';
    placeholder.textContent = 'Episode...';
    episodeSelect.add(placeholder);
    for (const e of filteredEps) {
      const opt = document.createElement('option');
      opt.value = String(e.episodeNumber);
      opt.textContent = `E${String(e.episodeNumber).padStart(2, '0')} - ${e.title}${e.hasFile ? ' (has file)' : ''}`;
      episodeSelect.add(opt);
    }
  }

  handleEpisodeSelect(sourceFile: string, select: HTMLSelectElement): void {
    const episodeNum = Number(select.value);
    if (Number.isNaN(episodeNum) || !select.value) return;

    const row = select.closest('tr');
    if (!row) return;
    const seasonSelect = row.querySelector(
      '.match-select:not(.episode-select)',
    ) as HTMLSelectElement | null;
    const seasonNum = Number(seasonSelect?.value);
    if (Number.isNaN(seasonNum)) return;

    this.manualOverrides.set(sourceFile, { seasonNumber: seasonNum, episodeNumber: episodeNum });
    // Re-render to update the row status
    const current = this.preview.value;
    if (current) {
      this.preview.set({ ...current });
    }
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${Number.parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  handleBack(): void {
    navigate('/activity/queue');
  }

  handleImport(): void {
    const data = this.preview.value;
    if (!data) return;
    this.importing.set(true);
    const overrides: Record<string, { seasonNumber: number; episodeNumber: number }> = {};
    for (const [key, value] of this.manualOverrides) {
      overrides[key] = value;
    }
    this.importMutation.mutate({ id: data.id, overrides });
  }

  private styles(): string {
    return `<style>
      .preview-page {
        display: flex;
        flex-direction: column;
        gap: 1rem;
      }

      .toolbar {
        display: flex;
        align-items: center;
        justify-content: space-between;
      }

      .toolbar-left {
        display: flex;
        align-items: center;
        gap: 0.75rem;
      }

      .back-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0.5rem;
        background-color: var(--btn-default-bg);
        border: 1px solid var(--btn-default-border);
        border-radius: 0.25rem;
        color: var(--text-color);
        cursor: pointer;
      }

      .back-btn:hover {
        background-color: var(--btn-default-bg-hover);
      }

      .page-title {
        font-size: 1.5rem;
        font-weight: 600;
        margin: 0;
      }

      .subtitle {
        color: var(--text-color-muted);
        font-size: 0.875rem;
      }

      /* Info cards */
      .info-cards {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
        gap: 0.75rem;
      }

      .info-card {
        background: var(--bg-card);
        border: 1px solid var(--border-color);
        border-radius: 0.5rem;
        padding: 0.75rem 1rem;
      }

      .info-label {
        font-size: 0.75rem;
        color: var(--text-color-muted);
        margin-bottom: 0.25rem;
      }

      .info-value {
        font-size: 0.875rem;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      /* Warning banner */
      .warning-banner {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.75rem 1rem;
        background: rgba(var(--color-warning-rgb, 243, 156, 18), 0.1);
        border: 1px solid var(--color-warning);
        border-radius: 0.5rem;
        color: var(--color-warning);
        font-size: 0.875rem;
      }

      /* File table */
      .file-table {
        width: 100%;
        table-layout: fixed;
        border-collapse: collapse;
        font-size: 0.875rem;
      }

      .file-table th,
      .file-table td {
        padding: 0.75rem;
        text-align: left;
        border-bottom: 1px solid var(--border-color);
      }

      .file-table th {
        font-weight: 600;
        color: var(--text-color-muted);
        white-space: nowrap;
        background-color: var(--bg-card-alt);
      }

      .file-table tbody tr:hover td {
        background-color: var(--bg-table-row-hover);
      }

      .row-skipped {
        opacity: 0.5;
      }

      .file-cell,
      .dest-cell {
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
      }

      .ep-title {
        font-size: 0.75rem;
        color: var(--text-color-muted);
      }

      .existing-size {
        font-size: 0.7rem;
        color: var(--text-color-muted);
        margin-top: 0.125rem;
      }

      /* Manual matching */
      .manual-match {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
      }

      .match-select {
        padding: 0.25rem 0.375rem;
        font-size: 0.75rem;
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        background: var(--bg-card);
        color: var(--text-color);
        width: 100%;
        max-width: 200px;
      }

      .match-select:focus {
        outline: none;
        border-color: var(--color-primary);
      }

      .manual-badge {
        font-size: 0.7rem;
        color: var(--color-primary);
        font-style: italic;
      }

      /* Status badges */
      .status-badge {
        display: inline-flex;
        padding: 0.125rem 0.5rem;
        font-size: 0.75rem;
        font-weight: 500;
        border-radius: 9999px;
      }

      .status-ready {
        background-color: var(--color-success, #2ecc71);
        color: var(--color-white, #fff);
      }

      .status-upgrade {
        background-color: var(--color-warning);
        color: var(--color-white, #fff);
      }

      .status-same {
        background-color: var(--bg-progress);
        color: var(--text-color-muted);
      }

      .status-skip {
        background-color: var(--bg-progress);
        color: var(--text-color-muted);
      }

      .row-same {
        opacity: 0.65;
      }

      /* Footer */
      .footer {
        display: flex;
        justify-content: flex-end;
        gap: 0.75rem;
        padding-top: 0.5rem;
        border-top: 1px solid var(--border-color);
      }

      .btn {
        display: inline-flex;
        align-items: center;
        padding: 0.5rem 1.25rem;
        font-size: 0.875rem;
        font-weight: 500;
        border-radius: 0.375rem;
        cursor: pointer;
        border: 1px solid transparent;
      }

      .btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
      }

      .btn-default {
        background-color: var(--btn-default-bg);
        border-color: var(--btn-default-border);
        color: var(--text-color);
      }

      .btn-default:hover:not(:disabled) {
        background-color: var(--btn-default-bg-hover);
      }

      .btn-primary {
        background-color: var(--color-success, #2ecc71);
        color: var(--color-white, #fff);
      }

      .btn-primary:hover:not(:disabled) {
        filter: brightness(1.1);
      }

      /* Loading / Error */
      .loading-container,
      .error-container {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1rem;
        padding: 4rem 2rem;
        text-align: center;
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
    </style>`;
  }
}
