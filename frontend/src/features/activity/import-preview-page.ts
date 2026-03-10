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
  episodeNumbers?: number[];
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

type SortField = 'name' | 'size' | 'episode' | 'status';
type SortDirection = 'asc' | 'desc';
type FilterMode = 'all' | 'video' | 'unmatched';

@customElement('import-preview-page')
export class ImportPreviewPage extends BaseComponent {
  private preview = signal<ImportPreviewResponse | null>(null);
  private loading = signal(true);
  private error = signal<string | null>(null);
  private importing = signal(false);
  private sortField = signal<SortField>('name');
  private sortDirection = signal<SortDirection>('asc');
  private filterMode = signal<FilterMode>('all');
  /** Which file is currently in episode-edit mode (sourceFile key, or null) */
  private editingFile = signal<string | null>(null);
  /** Manual episode overrides: sourceFile -> { seasonNumber, episodeNumbers[] } */
  private manualOverrides = new Map<string, { seasonNumber: number; episodeNumbers: number[] }>();
  /** Source files to force-reimport even if identical (same size as existing) */
  private forceReimportFiles = signal<Set<string>>(new Set());

  private importMutation = createMutation({
    mutationFn: (params: {
      id: number;
      overrides: Record<string, { seasonNumber: number; episodeNumbers: number[] }>;
      seriesId?: number;
      forceReimport?: string[];
    }) => {
      const hasOverrides = Object.keys(params.overrides).length > 0;
      const hasForceReimport = params.forceReimport && params.forceReimport.length > 0;
      const body =
        hasOverrides || params.seriesId || hasForceReimport
          ? {
              overrides: hasOverrides ? params.overrides : undefined,
              seriesId: params.seriesId,
              forceReimport: hasForceReimport ? params.forceReimport : undefined,
            }
          : undefined;
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
    this.watch(this.sortField);
    this.watch(this.sortDirection);
    this.watch(this.filterMode);
    this.watch(this.editingFile);
    this.watch(this.forceReimportFiles);
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
      if (this.forceReimportFiles.value.has(f.sourceFile)) return false;
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
          return this.forceReimportFiles.value.has(f.sourceFile);
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

        <div class="filter-bar">
          <div class="filter-buttons">
            <button class="filter-btn ${this.filterMode.value === 'all' ? 'active' : ''}"
              onclick="this.closest('import-preview-page').setFilter('all')">All</button>
            <button class="filter-btn ${this.filterMode.value === 'video' ? 'active' : ''}"
              onclick="this.closest('import-preview-page').setFilter('video')">Video Only</button>
            <button class="filter-btn ${this.filterMode.value === 'unmatched' ? 'active' : ''}"
              onclick="this.closest('import-preview-page').setFilter('unmatched')">Unmatched</button>
          </div>
        </div>

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
              <th class="sortable" onclick="this.closest('import-preview-page').toggleSort('name')">
                Source File ${this.renderSortIcon('name')}
              </th>
              <th class="sortable" onclick="this.closest('import-preview-page').toggleSort('size')">
                Size ${this.renderSortIcon('size')}
              </th>
              <th class="sortable" onclick="this.closest('import-preview-page').toggleSort('episode')">
                Episode ${this.renderSortIcon('episode')}
              </th>
              <th>Destination</th>
              <th class="sortable" onclick="this.closest('import-preview-page').toggleSort('status')">
                Status ${this.renderSortIcon('status')}
              </th>
            </tr>
          </thead>
          <tbody>
            ${this.getSortedFilteredFiles(data)
              .map((f) => this.renderFileRow(f, data))
              .join('')}
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
      // Exclude same-size files unless force-reimport is toggled
      const hasExisting = f.existingFile || this.getOverrideHasFile(f.sourceFile);
      const existingSize = this.getEffectiveExistingSize(f);
      if (hasExisting && existingSize != null && existingSize === f.sourceSize) {
        return this.forceReimportFiles.value.has(f.sourceFile);
      }
      return true;
    }).length;
  }

  /** Get the existing file size — uses episode fileSize for manual overrides, backend value otherwise */
  private getEffectiveExistingSize(file: ImportPreviewFile): number | null {
    const ov = this.manualOverrides.get(file.sourceFile);
    if (ov && ov.episodeNumbers.length > 0) {
      const ep = this.preview.value?.episodes?.find(
        (e) => e.seasonNumber === ov.seasonNumber && e.episodeNumber === ov.episodeNumbers[0],
      );
      return ep?.fileSize ?? null;
    }
    return file.existingFileSize ?? null;
  }

  private getOverrideHasFile(sourceFile: string): boolean {
    const ov = this.manualOverrides.get(sourceFile);
    if (!ov || ov.episodeNumbers.length === 0) return false;
    const ep = this.preview.value?.episodes?.find(
      (e) => e.seasonNumber === ov.seasonNumber && e.episodeNumber === ov.episodeNumbers[0],
    );
    return ep?.hasFile ?? false;
  }

  private renderFileRow(file: ImportPreviewFile, data: ImportPreviewResponse): string {
    const filename = file.sourceFile.split('/').pop() ?? file.sourceFile;
    const ov = this.manualOverrides.get(file.sourceFile);
    const isManuallyMatched = !!ov && ov.episodeNumbers.length > 0;
    const effectivelyMatched = file.matched || isManuallyMatched;

    const seasonNum = ov?.seasonNumber ?? file.seasonNumber;
    // Effective episode numbers: override > backend multi > backend single
    const effectiveEpNums: number[] = ov
      ? ov.episodeNumbers
      : file.episodeNumbers && file.episodeNumbers.length > 0
        ? file.episodeNumbers
        : file.episodeNumber != null
          ? [file.episodeNumber]
          : [];
    const firstEpNum = effectiveEpNums[0] ?? file.episodeNumber;
    const overrideEps = ov
      ? (ov.episodeNumbers
          .map((epNum) =>
            data.episodes?.find(
              (e) => e.seasonNumber === ov.seasonNumber && e.episodeNumber === epNum,
            ),
          )
          .filter(Boolean) as ImportPreviewEpisode[])
      : [];
    const firstOverrideEp = overrideEps[0] ?? null;

    let episodeLabel: string;
    if (effectiveEpNums.length > 1 && seasonNum != null) {
      const sorted = [...effectiveEpNums].sort((a, b) => a - b);
      episodeLabel = `S${String(seasonNum).padStart(2, '0')}E${String(sorted[0]).padStart(2, '0')}-E${String(sorted[sorted.length - 1]).padStart(2, '0')}`;
    } else if (seasonNum != null && firstEpNum != null) {
      episodeLabel = `S${String(seasonNum).padStart(2, '0')}E${String(firstEpNum).padStart(2, '0')}`;
    } else {
      episodeLabel = '-';
    }
    const epTitle =
      overrideEps.length > 1
        ? overrideEps.map((e) => e.title).join(' + ')
        : (firstOverrideEp?.title ?? file.episodeTitle);
    const destFilename = file.destinationPath?.split('/').pop() ?? '';
    const hasExistingFile = isManuallyMatched
      ? (firstOverrideEp?.hasFile ?? false)
      : file.existingFile;

    // For manually matched episodes, use the episode's file size; otherwise use the backend value
    const effectiveExistingSize = isManuallyMatched
      ? (firstOverrideEp?.fileSize ?? null)
      : (file.existingFileSize ?? null);

    // Detect same-size files: if source size matches existing file size, it's likely identical
    const isSameFile =
      hasExistingFile && effectiveExistingSize != null && effectiveExistingSize === file.sourceSize;

    const isForceReimport = this.forceReimportFiles.value.has(file.sourceFile);

    let statusClass = 'status-skip';
    let statusLabel = 'Skipped';
    if (effectivelyMatched && isSameFile && !isForceReimport) {
      statusClass = 'status-same';
      statusLabel = 'Identical';
    } else if (effectivelyMatched && isSameFile && isForceReimport) {
      statusClass = 'status-upgrade';
      statusLabel = 'Reimport';
    } else if (effectivelyMatched && hasExistingFile) {
      statusClass = 'status-upgrade';
      statusLabel = 'Upgrade';
    } else if (effectivelyMatched) {
      statusClass = 'status-ready';
      statusLabel = 'Ready';
    }

    const hasEpisodes = data.episodes && data.episodes.length > 0;
    const canEdit = isVideoFile(filename) && hasEpisodes;
    const isEditing = this.editingFile.value === file.sourceFile;
    const episodes = data.episodes ?? [];
    const seasons = [...new Set(episodes.map((e) => e.seasonNumber))].sort((a, b) => a - b);

    // Size display: source size on top, existing file size below when present
    const sizeHtml = effectiveExistingSize
      ? `${this.formatSize(file.sourceSize)}<div class="existing-size">${this.formatSize(effectiveExistingSize)} existing</div>`
      : this.formatSize(file.sourceSize);

    const escapedSourceFile = escapeHtml(file.sourceFile).replace(/'/g, "\\'");
    let episodeCellContent: string;
    if (isEditing) {
      episodeCellContent = this.renderManualMatchSelects(
        file.sourceFile,
        seasons,
        episodes,
        effectiveEpNums,
        seasonNum,
      );
    } else if (canEdit) {
      const epTitleHtml = epTitle ? `<div class="ep-title">${escapeHtml(epTitle)}</div>` : '';
      episodeCellContent = `<div class="ep-clickable" onclick="this.closest('import-preview-page').startEditEpisode('${escapedSourceFile}')" title="Click to change">${escapeHtml(episodeLabel)}${epTitleHtml}</div>`;
    } else {
      episodeCellContent = `<div>${escapeHtml(episodeLabel)}</div>${epTitle ? `<div class="ep-title">${escapeHtml(epTitle)}</div>` : ''}`;
    }

    const manualLabel =
      ov && ov.episodeNumbers.length > 1
        ? `Manual (${ov.episodeNumbers.length} episodes)`
        : 'Manual';
    const destContent = isManuallyMatched
      ? `<span class="manual-badge">${manualLabel}</span>`
      : escapeHtml(destFilename);

    const rowClass = !effectivelyMatched
      ? 'row-skipped'
      : isSameFile && !isForceReimport
        ? 'row-same'
        : '';
    const reimportBtn =
      effectivelyMatched && isSameFile
        ? `<button class="reimport-btn ${isForceReimport ? 'active' : ''}" onclick="this.closest('import-preview-page').toggleForceReimport('${escapedSourceFile}')" title="${isForceReimport ? 'Cancel reimport' : 'Force reimport (overwrite existing file)'}">${isForceReimport ? 'Undo' : 'Reimport'}</button>`
        : '';

    return html`
      <tr class="${rowClass}">
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
          ${safeHtml(reimportBtn)}
        </td>
      </tr>
    `;
  }

  private renderManualMatchSelects(
    sourceFile: string,
    seasons: number[],
    episodes: ImportPreviewEpisode[],
    selectedEpNums: number[] = [],
    selectedSeason: number | null = null,
  ): string {
    const escapedSourceFile = escapeHtml(sourceFile).replace(/'/g, "\\'");
    const activeSeason = selectedSeason ?? seasons[0] ?? 0;

    const seasonOptions = seasons
      .map((s) => {
        const label = s === 0 ? 'Specials' : `Season ${s}`;
        const selected = s === activeSeason ? ' selected' : '';
        return `<option value="${s}"${selected}>${escapeHtml(label)}</option>`;
      })
      .join('');

    const filteredEps = episodes.filter((e) => e.seasonNumber === activeSeason);
    const episodeOptions = filteredEps
      .map((e) => {
        const label = `E${String(e.episodeNumber).padStart(2, '0')} - ${e.title}${e.hasFile ? ' (has file)' : ''}`;
        const selected = selectedEpNums.includes(e.episodeNumber) ? ' selected' : '';
        return `<option value="${e.episodeNumber}"${selected}>${escapeHtml(label)}</option>`;
      })
      .join('');

    const visibleRows = Math.min(filteredEps.length, 6);

    return `
      <div class="manual-match">
        <select class="match-select" onchange="this.closest('import-preview-page').handleSeasonChange('${escapedSourceFile}', this)">
          ${seasonOptions}
        </select>
        <select class="match-select episode-select" data-source="${escapeHtml(sourceFile)}"
          multiple size="${visibleRows}">
          ${episodeOptions}
        </select>
        <div class="match-actions">
          <button class="match-confirm-btn" onclick="this.closest('import-preview-page').confirmEpisodeSelect('${escapedSourceFile}', this)">Assign</button>
          <button class="match-cancel-btn" onclick="this.closest('import-preview-page').cancelEditEpisode()">Cancel</button>
        </div>
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
    while (episodeSelect.options.length > 0) {
      episodeSelect.remove(0);
    }
    for (const e of filteredEps) {
      const opt = document.createElement('option');
      opt.value = String(e.episodeNumber);
      opt.textContent = `E${String(e.episodeNumber).padStart(2, '0')} - ${e.title}${e.hasFile ? ' (has file)' : ''}`;
      episodeSelect.add(opt);
    }
    episodeSelect.size = Math.min(filteredEps.length, 6);
  }

  startEditEpisode(sourceFile: string): void {
    this.editingFile.set(sourceFile);
  }

  cancelEditEpisode(): void {
    this.editingFile.set(null);
  }

  confirmEpisodeSelect(sourceFile: string, button: HTMLButtonElement): void {
    const row = button.closest('tr');
    if (!row) return;

    const episodeSelect = row.querySelector('.episode-select') as HTMLSelectElement | null;
    if (!episodeSelect) return;

    const selectedEpisodes: number[] = [];
    for (const opt of episodeSelect.selectedOptions) {
      const num = Number(opt.value);
      if (!Number.isNaN(num)) {
        selectedEpisodes.push(num);
      }
    }
    if (selectedEpisodes.length === 0) return;

    const seasonSelect = row.querySelector(
      '.match-select:not(.episode-select)',
    ) as HTMLSelectElement | null;
    const seasonNum = Number(seasonSelect?.value);
    if (Number.isNaN(seasonNum)) return;

    this.manualOverrides.set(sourceFile, {
      seasonNumber: seasonNum,
      episodeNumbers: selectedEpisodes,
    });
    this.editingFile.set(null);
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
    const overrides: Record<string, { seasonNumber: number; episodeNumbers: number[] }> = {};
    for (const [key, value] of this.manualOverrides) {
      overrides[key] = value;
    }
    const forceReimport = [...this.forceReimportFiles.value];
    this.importMutation.mutate({
      id: data.id,
      overrides,
      seriesId: data.series?.id,
      forceReimport: forceReimport.length > 0 ? forceReimport : undefined,
    });
  }

  toggleForceReimport(sourceFile: string): void {
    const current = new Set(this.forceReimportFiles.value);
    if (current.has(sourceFile)) {
      current.delete(sourceFile);
    } else {
      current.add(sourceFile);
    }
    this.forceReimportFiles.set(current);
  }

  toggleSort(field: SortField): void {
    if (this.sortField.value === field) {
      this.sortDirection.set(this.sortDirection.value === 'asc' ? 'desc' : 'asc');
    } else {
      this.sortField.set(field);
      this.sortDirection.set('asc');
    }
  }

  setFilter(mode: FilterMode): void {
    this.filterMode.set(mode);
  }

  private renderSortIcon(field: SortField): string {
    if (this.sortField.value !== field) return '<span class="sort-icon"></span>';
    const arrow = this.sortDirection.value === 'asc' ? '\u25B2' : '\u25BC';
    return `<span class="sort-icon active">${arrow}</span>`;
  }

  private getFileStatus(file: ImportPreviewFile): string {
    const ov = this.manualOverrides.get(file.sourceFile);
    const effectivelyMatched = file.matched || (!!ov && ov.episodeNumbers.length > 0);
    if (!effectivelyMatched) return 'skip';
    const hasExisting = file.existingFile || this.getOverrideHasFile(file.sourceFile);
    if (hasExisting) {
      const existingSize = this.getEffectiveExistingSize(file);
      if (existingSize != null && existingSize === file.sourceSize) {
        return this.forceReimportFiles.value.has(file.sourceFile) ? 'reimport' : 'same';
      }
      return 'upgrade';
    }
    return 'ready';
  }

  private getSortedFilteredFiles(data: ImportPreviewResponse): ImportPreviewFile[] {
    let files = [...data.files];

    // Filter
    const mode = this.filterMode.value;
    if (mode === 'video') {
      files = files.filter((f) => {
        const name = f.sourceFile.split('/').pop() ?? f.sourceFile;
        return isVideoFile(name);
      });
    } else if (mode === 'unmatched') {
      files = files.filter((f) => {
        return !f.matched && !this.manualOverrides.has(f.sourceFile);
      });
    }

    // Sort
    const field = this.sortField.value;
    const dir = this.sortDirection.value === 'asc' ? 1 : -1;
    files.sort((a, b) => {
      switch (field) {
        case 'name': {
          const nameA = (a.sourceFile.split('/').pop() ?? '').toLowerCase();
          const nameB = (b.sourceFile.split('/').pop() ?? '').toLowerCase();
          return dir * nameA.localeCompare(nameB);
        }
        case 'size':
          return dir * (a.sourceSize - b.sourceSize);
        case 'episode': {
          const seA = (a.seasonNumber ?? 999) * 10000 + (a.episodeNumber ?? 999);
          const seB = (b.seasonNumber ?? 999) * 10000 + (b.episodeNumber ?? 999);
          return dir * (seA - seB);
        }
        case 'status': {
          const order: Record<string, number> = { ready: 0, upgrade: 1, skip: 2 };
          return dir * ((order[this.getFileStatus(a)] ?? 3) - (order[this.getFileStatus(b)] ?? 3));
        }
        default:
          return 0;
      }
    });

    return files;
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

      /* Filter bar */
      .filter-bar {
        display: flex;
        align-items: center;
        gap: 0.75rem;
      }

      .filter-buttons {
        display: flex;
        gap: 0.25rem;
      }

      .filter-btn {
        padding: 0.25rem 0.75rem;
        font-size: 0.75rem;
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        background: var(--bg-card);
        color: var(--text-color-muted);
        cursor: pointer;
      }

      .filter-btn:hover {
        background: var(--bg-card-alt);
      }

      .filter-btn.active {
        background: var(--color-primary);
        color: var(--color-white, #fff);
        border-color: var(--color-primary);
      }

      /* Sortable headers */
      .sortable {
        cursor: pointer;
        user-select: none;
      }

      .sortable:hover {
        color: var(--text-color);
      }

      .sort-icon {
        font-size: 0.625rem;
        margin-left: 0.25rem;
        opacity: 0.3;
      }

      .sort-icon.active {
        opacity: 1;
        color: var(--color-primary);
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

      .match-select[multiple] {
        padding: 0;
      }

      .match-select[multiple] option {
        padding: 0.2rem 0.375rem;
      }

      .match-select:focus {
        outline: none;
        border-color: var(--color-primary);
      }

      .match-actions {
        display: flex;
        align-items: center;
        gap: 0.5rem;
      }

      .match-confirm-btn {
        padding: 0.2rem 0.5rem;
        font-size: 0.7rem;
        font-weight: 500;
        border: 1px solid var(--color-primary);
        border-radius: 0.25rem;
        background: var(--color-primary);
        color: var(--color-white, #fff);
        cursor: pointer;
        white-space: nowrap;
      }

      .match-confirm-btn:hover {
        filter: brightness(1.1);
      }

      .match-cancel-btn {
        padding: 0.2rem 0.5rem;
        font-size: 0.7rem;
        font-weight: 500;
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        background: var(--bg-card);
        color: var(--text-color-muted);
        cursor: pointer;
        white-space: nowrap;
      }

      .match-cancel-btn:hover {
        background: var(--bg-card-alt);
      }

      .ep-clickable {
        cursor: pointer;
        border-bottom: 1px dashed var(--text-color-muted);
        display: inline-block;
      }

      .ep-clickable:hover {
        color: var(--color-primary);
        border-bottom-color: var(--color-primary);
      }

      .multi-hint {
        font-size: 0.625rem;
        color: var(--text-color-muted);
        opacity: 0.7;
        white-space: nowrap;
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

      .reimport-btn {
        display: inline-block;
        margin-top: 0.25rem;
        padding: 0.15rem 0.4rem;
        font-size: 0.7rem;
        border: 1px solid var(--border-color);
        border-radius: 3px;
        background: transparent;
        color: var(--text-color-muted);
        cursor: pointer;
      }
      .reimport-btn:hover {
        background: var(--bg-hover);
        color: var(--text-color);
      }
      .reimport-btn.active {
        background: var(--color-warning);
        color: var(--color-white, #fff);
        border-color: var(--color-warning);
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
