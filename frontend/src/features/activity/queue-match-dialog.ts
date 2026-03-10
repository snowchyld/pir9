/**
 * Queue match dialog - manually fix the series match for a queue item.
 * Episodes are resolved during import, not at match time.
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http, type Movie, type QueueItem, type Series } from '../../core/http';
import { createMutation, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';

interface DownloadFile {
  name: string;
  size: number;
}

@customElement('queue-match-dialog')
export class QueueMatchDialog extends BaseComponent {
  private isOpen = signal(false);
  private queueItem = signal<QueueItem | null>(null);
  private queueTitle = signal('');
  private allSeries = signal<Series[]>([]);
  private selectedSeriesId = signal<number | null>(null);
  private isLoadingSeries = signal(false);
  private seriesFilter = signal('');
  private downloadFiles = signal<DownloadFile[]>([]);
  private isLoadingFiles = signal(false);
  private showFiles = signal(false);

  // Movie matching
  private contentType = signal<'series' | 'movie' | 'anime'>('series');
  private allMovies = signal<Movie[]>([]);
  private isLoadingMovies = signal(false);
  private movieFilter = signal('');
  private selectedMovieId = signal<number | null>(null);

  private matchMutation = createMutation({
    mutationFn: (params: {
      id: number;
      seriesId?: number;
      episodeIds?: number[];
      movieId?: number;
      downloadId?: string;
      downloadClient?: string;
      protocol?: string;
      size?: number;
      title?: string;
    }) =>
      http.put<{ success: boolean }>(`/queue/${params.id}/match`, {
        seriesId: params.seriesId,
        episodeIds: params.episodeIds,
        movieId: params.movieId,
        downloadId: params.downloadId,
        downloadClient: params.downloadClient,
        protocol: params.protocol,
        size: params.size,
        title: params.title,
      }),
    onSuccess: (result: { success: boolean }) => {
      if (result.success) {
        invalidateQueries(['/queue']);
        showSuccess('Queue match updated');
        this.close();
      } else {
        showError('Failed to update queue match');
      }
    },
    onError: () => {
      showError('Failed to update queue match');
    },
  });

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.allSeries);
    this.watch(this.selectedSeriesId);
    this.watch(this.isLoadingSeries);
    this.watch(this.seriesFilter);
    this.watch(this.matchMutation.isLoading);
    this.watch(this.downloadFiles);
    this.watch(this.isLoadingFiles);
    this.watch(this.showFiles);
    this.watch(this.contentType);
    this.watch(this.allMovies);
    this.watch(this.isLoadingMovies);
    this.watch(this.movieFilter);
    this.watch(this.selectedMovieId);
  }

  private onCloseCallback: (() => void) | null = null;

  open(item: QueueItem, onClose?: () => void): void {
    this.onCloseCallback = onClose ?? null;
    this.queueItem.set(item);
    this.queueTitle.set(item.title);
    this.selectedSeriesId.set(null);
    this.seriesFilter.set('');
    this.downloadFiles.set([]);
    this.showFiles.set(false);
    this.selectedMovieId.set(null);
    this.movieFilter.set('');

    const ct = item.contentType ?? 'series';
    this.contentType.set(ct);
    this.isOpen.set(true);

    if (ct === 'movie') {
      this.loadMovies();
    } else {
      this.loadSeries();
    }
    this.loadFiles(item.id);
  }

  close(): void {
    this.isOpen.set(false);
    if (this.onCloseCallback) {
      this.onCloseCallback();
      this.onCloseCallback = null;
    }
  }

  private async loadSeries(): Promise<void> {
    this.isLoadingSeries.set(true);
    try {
      const series = await http.get<Series[]>('/series');
      this.allSeries.set(series.sort((a, b) => a.sortTitle.localeCompare(b.sortTitle)));
    } catch {
      showError('Failed to load series');
    } finally {
      this.isLoadingSeries.set(false);
    }
  }

  private async loadFiles(queueId: number): Promise<void> {
    this.isLoadingFiles.set(true);
    try {
      const files = await http.get<DownloadFile[]>(`/queue/${queueId}/files`);
      this.downloadFiles.set(files);
    } catch {
      // Silently ignore — usenet clients don't support file listing
      this.downloadFiles.set([]);
    } finally {
      this.isLoadingFiles.set(false);
    }
  }

  toggleFiles(): void {
    this.showFiles.set(!this.showFiles.value);
  }

  selectSeries(seriesId: number): void {
    this.selectedSeriesId.set(seriesId);
  }

  confirmMatch(): void {
    const item = this.queueItem.value;
    const seriesId = this.selectedSeriesId.value;
    if (!item || !seriesId) return;

    // Series-only match — episodes resolved during import
    const isUntracked = item.id >= 10000;
    this.matchMutation.mutate({
      id: item.id,
      seriesId,
      ...(isUntracked
        ? {
            downloadId: item.downloadId,
            downloadClient: item.downloadClient,
            protocol: item.protocol,
            size: item.size,
            title: item.title,
          }
        : {}),
    });
  }

  updateSeriesFilter(value: string): void {
    this.seriesFilter.set(value);
  }

  private async loadMovies(): Promise<void> {
    this.isLoadingMovies.set(true);
    try {
      const movies = await http.get<Movie[]>('/movie');
      this.allMovies.set(movies.sort((a, b) => a.sortTitle.localeCompare(b.sortTitle)));
    } catch {
      showError('Failed to load movies');
    } finally {
      this.isLoadingMovies.set(false);
    }
  }

  updateMovieFilter(value: string): void {
    this.movieFilter.set(value);
  }

  selectMovie(movieId: number): void {
    this.selectedMovieId.set(movieId);
  }

  confirmMovieMatch(): void {
    const item = this.queueItem.value;
    const movieId = this.selectedMovieId.value;
    if (!item || !movieId) return;

    const isUntracked = item.id >= 10000;
    this.matchMutation.mutate({
      id: item.id,
      movieId,
      ...(isUntracked
        ? {
            downloadId: item.downloadId,
            downloadClient: item.downloadClient,
            protocol: item.protocol,
            size: item.size,
            title: item.title,
          }
        : {}),
    });
  }

  private formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${Number.parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  protected template(): string {
    if (!this.isOpen.value) return '';

    const title = this.queueTitle.value;
    const seriesId = this.selectedSeriesId.value;
    const isSubmitting = this.matchMutation.isLoading.value;
    const isMovie = this.contentType.value === 'movie';
    const movieId = this.selectedMovieId.value;

    // Determine confirm button state and handler
    const canConfirm = isMovie ? movieId !== null : seriesId !== null;
    const confirmHandler = isMovie ? 'confirmMovieMatch' : 'confirmMatch';

    return html`
      <div class="dialog-overlay" onclick="if(event.target===this) this.querySelector('queue-match-dialog')?.close?.() || this.closest('queue-match-dialog').close()">
        <div class="dialog" onclick="event.stopPropagation()">
          <div class="dialog-header">
            <h3>Fix Match</h3>
            <button class="close-btn" onclick="this.closest('queue-match-dialog').close()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="dialog-body">
            <div class="download-title">${escapeHtml(title)}</div>

            ${this.renderFilesPanel()}

            ${isMovie ? this.renderMovieSelector() : this.renderSeriesSelector()}
          </div>

          <div class="dialog-footer">
            <button class="btn secondary" onclick="this.closest('queue-match-dialog').close()">Cancel</button>
            <button
              class="btn primary"
              onclick="this.closest('queue-match-dialog').${confirmHandler}()"
              ${!canConfirm || isSubmitting ? 'disabled' : ''}
            >
              ${isSubmitting ? 'Saving...' : 'Confirm'}
            </button>
          </div>
        </div>
      </div>

      <style>
        .dialog-overlay {
          position: fixed;
          inset: 0;
          background: rgb(0, 0, 0);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1000;
        }

        .dialog {
          background: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
          width: 600px;
          max-height: 80vh;
          display: flex;
          flex-direction: column;
        }

        .dialog-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem 1.5rem;
          border-bottom: 1px solid var(--border-color);
        }

        .dialog-header h3 {
          margin: 0;
          font-size: 1.125rem;
        }

        .close-btn {
          background: none;
          border: none;
          color: var(--text-color-muted);
          cursor: pointer;
          padding: 0.25rem;
        }

        .dialog-body {
          padding: 1rem 1.5rem;
          overflow-y: auto;
          flex: 1;
        }

        .download-title {
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          margin-bottom: 1rem;
          word-break: break-word;
        }

        .search-input {
          width: 100%;
          padding: 0.5rem 0.75rem;
          background: var(--bg-input);
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          color: var(--text-color);
          font-size: 0.875rem;
          margin-bottom: 0.75rem;
          box-sizing: border-box;
        }

        .series-list {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
          max-height: 400px;
          overflow-y: auto;
        }

        .series-option {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.5rem 0.75rem;
          border-radius: 0.25rem;
          cursor: pointer;
          border: 1px solid transparent;
        }

        .series-option:hover {
          background: var(--bg-table-row-hover);
        }

        .series-option.selected-movie {
          background: var(--color-primary);
          color: white;
          border-color: var(--color-primary);
        }

        .series-option-title {
          font-weight: 500;
        }

        .series-option-year {
          color: var(--text-color-muted);
          font-size: 0.8125rem;
        }

        .dialog-footer {
          display: flex;
          justify-content: flex-end;
          gap: 0.5rem;
          padding: 1rem 1.5rem;
          border-top: 1px solid var(--border-color);
        }

        .btn {
          padding: 0.5rem 1rem;
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          cursor: pointer;
          font-size: 0.875rem;
        }

        .btn.primary {
          background: var(--color-primary);
          color: white;
          border-color: var(--color-primary);
        }

        .btn.primary:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .btn.secondary {
          background: var(--btn-default-bg);
          color: var(--text-color);
        }

        .loading-text {
          color: var(--text-color-muted);
          text-align: center;
          padding: 2rem;
        }



        .files-panel {
          margin-bottom: 0.75rem;
          border: 1px solid var(--border-color);
          border-radius: 0.25rem;
          overflow: hidden;
        }

        .files-header {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.75rem;
          background: var(--bg-card-alt);
          cursor: pointer;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          user-select: none;
        }

        .files-header:hover {
          color: var(--text-color);
        }

        .files-chevron {
          transition: transform 0.15s ease;
          flex-shrink: 0;
        }

        .files-chevron.open {
          transform: rotate(90deg);
        }

        .files-summary {
          font-weight: 500;
        }

        .files-loading {
          display: block;
          padding: 0.5rem 0.75rem;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
        }

        .files-list {
          max-height: 200px;
          overflow-y: auto;
          border-top: 1px solid var(--border-color);
        }

        .file-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 0.5rem;
          padding: 0.375rem 0.75rem;
          font-size: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .file-item:last-child {
          border-bottom: none;
        }

        .file-name {
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          min-width: 0;
        }

        .file-size {
          color: var(--text-color-muted);
          white-space: nowrap;
          flex-shrink: 0;
        }
      </style>
    `;
  }

  private renderFilesPanel(): string {
    const files = this.downloadFiles.value;
    const isLoading = this.isLoadingFiles.value;
    const showFiles = this.showFiles.value;

    // Don't show panel if still loading and no files yet, or if there are no files
    if (isLoading) {
      return html`<div class="files-panel"><span class="files-loading">Loading files...</span></div>`;
    }

    if (files.length <= 3) return '';

    const fileCount = files.length;
    const totalSize = files.reduce((sum, f) => sum + f.size, 0);

    return html`
      <div class="files-panel">
        <div class="files-header" onclick="this.closest('queue-match-dialog').toggleFiles()">
          <svg class="files-chevron ${showFiles ? 'open' : ''}" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"></polyline>
          </svg>
          <span class="files-summary">${fileCount} file${fileCount !== 1 ? 's' : ''} (${this.formatFileSize(totalSize)})</span>
        </div>
        ${
          showFiles
            ? html`
          <div class="files-list">
            ${files
              .map(
                (f) => html`
              <div class="file-item">
                <span class="file-name" title="${escapeHtml(f.name)}">${escapeHtml(f.name)}</span>
                <span class="file-size">${this.formatFileSize(f.size)}</span>
              </div>
            `,
              )
              .join('')}
          </div>
        `
            : ''
        }
      </div>
    `;
  }

  private renderSeriesSelector(): string {
    if (this.isLoadingSeries.value) {
      return html`<div class="loading-text">Loading series...</div>`;
    }

    const filter = this.seriesFilter.value.toLowerCase();
    const series = this.allSeries.value.filter(
      (s) => !filter || s.title.toLowerCase().includes(filter),
    );
    const selectedId = this.selectedSeriesId.value;

    return html`
      <input
        type="text"
        class="search-input"
        placeholder="Filter series..."
        value="${escapeHtml(this.seriesFilter.value)}"
        oninput="this.closest('queue-match-dialog').updateSeriesFilter(this.value)"
      />
      <div class="series-list">
        ${series
          .map(
            (s) => html`
          <div class="series-option ${s.id === selectedId ? 'selected-movie' : ''}" onclick="this.closest('queue-match-dialog').selectSeries(${s.id})">
            <span class="series-option-title">${escapeHtml(s.title)}</span>
            <span class="series-option-year">(${s.year})</span>
          </div>
        `,
          )
          .join('')}
      </div>
    `;
  }

  private renderMovieSelector(): string {
    if (this.isLoadingMovies.value) {
      return html`<div class="loading-text">Loading movies...</div>`;
    }

    const filter = this.movieFilter.value.toLowerCase();
    const movies = this.allMovies.value.filter(
      (m) => !filter || m.title.toLowerCase().includes(filter),
    );
    const selectedId = this.selectedMovieId.value;

    return html`
      <input
        type="text"
        class="search-input"
        placeholder="Filter movies..."
        value="${escapeHtml(this.movieFilter.value)}"
        oninput="this.closest('queue-match-dialog').updateMovieFilter(this.value)"
      />
      <div class="series-list">
        ${movies
          .map(
            (m) => html`
          <div class="series-option ${m.id === selectedId ? 'selected-movie' : ''}" onclick="this.closest('queue-match-dialog').selectMovie(${m.id})">
            <span class="series-option-title">${escapeHtml(m.title)}</span>
            <span class="series-option-year">(${m.year})</span>
          </div>
        `,
          )
          .join('')}
        ${movies.length === 0 ? html`<div class="loading-text">No movies found</div>` : ''}
      </div>
    `;
  }
}
