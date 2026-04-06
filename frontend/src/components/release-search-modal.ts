/**
 * Interactive Release Search Modal
 * Shows releases from indexers and allows manual selection
 */

import { BaseComponent, customElement, escapeHtml, html } from '../core/component';
import { httpV3 } from '../core/http';
import { signal } from '../core/reactive';
import { showError, showSuccess } from '../stores/app.store';

interface ReleaseResource {
  guid: string;
  quality: {
    quality: {
      id: number;
      name: string;
      source: string;
      resolution: number;
    };
    revision: {
      version: number;
      real: number;
      isRepack: boolean;
    };
  };
  qualityWeight: number;
  age: number;
  ageHours: number;
  ageMinutes: number;
  size: number;
  indexerId: number;
  indexer: string;
  releaseGroup: string | null;
  title: string;
  fullSeason: boolean;
  seasonNumber: number;
  episodeNumbers: number[];
  approved: boolean;
  rejected: boolean;
  rejections: string[];
  publishDate: string;
  downloadUrl: string | null;
  seeders: number | null;
  leechers: number | null;
  protocol: string;
}

interface SearchParams {
  seriesId?: number;
  seriesTitle?: string;
  seasonNumber?: number;
  episodeId?: number;
  episodeNumber?: number;
  episodeTitle?: string;
  movieId?: number;
  movieTitle?: string;
  /** Free-text query for music/general search */
  query?: string;
  queryTitle?: string;
}

@customElement('release-search-modal')
export class ReleaseSearchModal extends BaseComponent {
  private isOpen = signal(false);
  private isLoading = signal(false);
  private isGrabbing = signal<string | null>(null);
  private releases = signal<ReleaseResource[]>([]);
  private searchParams = signal<SearchParams | null>(null);
  private sortBy = signal<'quality' | 'age' | 'size' | 'seeders'>('quality');
  private sortDesc = signal(true);

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.isLoading);
    this.watch(this.isGrabbing);
    this.watch(this.releases);
    this.watch(this.searchParams);
    this.watch(this.sortBy);
    this.watch(this.sortDesc);
  }

  // Public API to open the modal
  async open(params: SearchParams): Promise<void> {
    this.searchParams.set(params);
    this.isOpen.set(true);
    this.releases.set([]);
    await this.search();
  }

  close(): void {
    this.isOpen.set(false);
    this.searchParams.set(null);
    this.releases.set([]);
  }

  private async search(): Promise<void> {
    const params = this.searchParams.value;
    if (!params) return;

    this.isLoading.set(true);

    try {
      const queryParams: Record<string, string | number> = {};

      if (params.query) {
        queryParams.query = params.query;
      } else if (params.movieId) {
        queryParams.movieId = params.movieId;
      } else if (params.seriesId) {
        queryParams.seriesId = params.seriesId;
        if (params.episodeId) {
          queryParams.episodeId = params.episodeId;
        }
        if (params.seasonNumber !== undefined) {
          queryParams.seasonNumber = params.seasonNumber;
        }
      }

      const results = await httpV3.get<ReleaseResource[]>('/release', { params: queryParams });
      this.releases.set(results);
    } catch (_e) {
      showError('Failed to search for releases');
      this.releases.set([]);
    } finally {
      this.isLoading.set(false);
    }
  }

  private async grabRelease(release: ReleaseResource): Promise<void> {
    this.isGrabbing.set(release.guid);
    const params = this.searchParams.value;

    try {
      const grabBody: Record<string, unknown> = {
        guid: release.guid,
        indexerId: release.indexerId,
      };
      if (params?.movieId) {
        grabBody.movieId = params.movieId;
      }
      await httpV3.post('/release', grabBody);
      showSuccess(`Grabbed: ${release.title}`);
      this.close();
    } catch (_e) {
      showError('Failed to grab release');
    } finally {
      this.isGrabbing.set(null);
    }
  }

  private getSortedReleases(): ReleaseResource[] {
    const releases = [...this.releases.value];
    const sortBy = this.sortBy.value;
    const desc = this.sortDesc.value;

    releases.sort((a, b) => {
      let cmp = 0;
      switch (sortBy) {
        case 'quality':
          cmp = a.qualityWeight - b.qualityWeight;
          break;
        case 'age':
          cmp = a.age - b.age;
          break;
        case 'size':
          cmp = a.size - b.size;
          break;
        case 'seeders':
          cmp = (a.seeders ?? 0) - (b.seeders ?? 0);
          break;
      }
      return desc ? -cmp : cmp;
    });

    return releases;
  }

  private toggleSort(column: 'quality' | 'age' | 'size' | 'seeders'): void {
    if (this.sortBy.value === column) {
      this.sortDesc.set(!this.sortDesc.value);
    } else {
      this.sortBy.set(column);
      this.sortDesc.set(true);
    }
  }

  protected template(): string {
    if (!this.isOpen.value) {
      return '';
    }

    const params = this.searchParams.value;
    const isLoading = this.isLoading.value;
    const releases = this.getSortedReleases();
    const grabbing = this.isGrabbing.value;

    const title = params?.queryTitle
      ? params.queryTitle
      : params?.movieTitle
        ? params.movieTitle
        : params?.episodeNumber !== undefined
          ? `${params?.seriesTitle} - S${String(params?.seasonNumber ?? 0).padStart(2, '0')}E${String(params.episodeNumber).padStart(2, '0')} - ${params?.episodeTitle ?? ''}`
          : params?.seasonNumber !== undefined
            ? `${params?.seriesTitle} - Season ${params.seasonNumber}`
            : (params?.seriesTitle ?? 'Search');

    return html`
      <div class="modal-overlay" onclick="if(event.target === this) this.closest('release-search-modal').close()">
        <div class="modal-content">
          <div class="modal-header">
            <h2 class="modal-title">Interactive Search</h2>
            <p class="modal-subtitle">${escapeHtml(title)}</p>
            <button class="close-btn" onclick="this.closest('release-search-modal').close()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="modal-body">
            ${
              isLoading
                ? html`
              <div class="loading-state">
                <div class="spinner"></div>
                <p>Searching indexers...</p>
              </div>
            `
                : releases.length === 0
                  ? html`
              <div class="empty-state">
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                  <circle cx="11" cy="11" r="8"></circle>
                  <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
                </svg>
                <p>No releases found</p>
                <button class="retry-btn" onclick="this.closest('release-search-modal').retrySearch()">
                  Try Again
                </button>
              </div>
            `
                  : html`
              <div class="results-info">
                Found ${releases.length} releases
              </div>

              <div class="releases-table-container">
                <table class="releases-table">
                  <thead>
                    <tr>
                      <th class="col-title">Release</th>
                      <th class="col-indexer">Indexer</th>
                      <th class="col-quality sortable ${this.sortBy.value === 'quality' ? 'sorted' : ''}"
                          onclick="this.closest('release-search-modal').handleSortClick('quality')">
                        Quality ${this.renderSortIcon('quality')}
                      </th>
                      <th class="col-size sortable ${this.sortBy.value === 'size' ? 'sorted' : ''}"
                          onclick="this.closest('release-search-modal').handleSortClick('size')">
                        Size ${this.renderSortIcon('size')}
                      </th>
                      <th class="col-peers sortable ${this.sortBy.value === 'seeders' ? 'sorted' : ''}"
                          onclick="this.closest('release-search-modal').handleSortClick('seeders')">
                        Peers ${this.renderSortIcon('seeders')}
                      </th>
                      <th class="col-age sortable ${this.sortBy.value === 'age' ? 'sorted' : ''}"
                          onclick="this.closest('release-search-modal').handleSortClick('age')">
                        Age ${this.renderSortIcon('age')}
                      </th>
                      <th class="col-actions"></th>
                    </tr>
                  </thead>
                  <tbody>
                    ${releases.map((r) => this.renderRelease(r, grabbing)).join('')}
                  </tbody>
                </table>
              </div>
            `
            }
          </div>
        </div>
      </div>

      <style>
        .modal-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background-color: rgba(0, 0, 0, 0.7);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1000;
          padding: 1rem;
        }

        .modal-content {
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
          width: 100%;
          max-width: 1200px;
          max-height: 85vh;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }

        .modal-header {
          display: flex;
          flex-direction: column;
          padding: 1rem 1.5rem;
          border-bottom: 1px solid var(--border-color);
          position: relative;
        }

        .modal-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0;
        }

        .modal-subtitle {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin: 0.25rem 0 0 0;
        }

        .close-btn {
          position: absolute;
          top: 1rem;
          right: 1rem;
          padding: 0.25rem;
          background: transparent;
          border: none;
          color: var(--text-color-muted);
          cursor: pointer;
          border-radius: 0.25rem;
        }

        .close-btn:hover {
          color: var(--text-color);
          background-color: var(--bg-input-hover);
        }

        .modal-body {
          flex: 1;
          overflow: auto;
          padding: 1rem;
        }

        .loading-state, .empty-state {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          padding: 3rem;
          gap: 1rem;
          color: var(--text-color-muted);
        }

        .spinner {
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

        .retry-btn {
          padding: 0.5rem 1rem;
          background-color: var(--btn-primary-bg);
          border: none;
          border-radius: 0.25rem;
          color: white;
          cursor: pointer;
        }

        .retry-btn:hover {
          background-color: var(--btn-primary-bg-hover);
        }

        .results-info {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin-bottom: 0.75rem;
        }

        .releases-table-container {
          overflow-x: auto;
        }

        .releases-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
          table-layout: fixed;
        }

        .releases-table th {
          text-align: left;
          padding: 0.75rem 0.5rem;
          font-weight: 500;
          color: var(--text-color-muted);
          border-bottom: 1px solid var(--border-color);
          white-space: nowrap;
        }

        .releases-table th.sortable {
          cursor: pointer;
          user-select: none;
        }

        .releases-table th.sortable:hover {
          color: var(--text-color);
        }

        .releases-table th.sorted {
          color: var(--color-primary);
        }

        .sort-icon {
          display: inline-block;
          margin-left: 0.25rem;
          vertical-align: middle;
        }

        .releases-table td {
          padding: 0.75rem 0.5rem;
          border-bottom: 1px solid var(--border-color);
          vertical-align: middle;
        }

        .releases-table tbody tr:hover {
          background-color: var(--bg-table-row-hover);
        }

        .col-title {
          width: auto;
        }

        .release-title {
          font-weight: 500;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .release-rejected {
          opacity: 0.5;
        }

        .release-rejections {
          font-size: 0.75rem;
          color: var(--color-danger);
          margin-top: 0.25rem;
        }

        .release-group {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .col-indexer {
          width: 110px;
        }

        .col-quality {
          width: 100px;
        }

        .quality-badge {
          display: inline-block;
          padding: 0.125rem 0.5rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 500;
          background-color: var(--color-primary);
          color: white;
        }

        .quality-badge.sd {
          background-color: #6c757d;
        }

        .quality-badge.hd720 {
          background-color: #28a745;
        }

        .quality-badge.hd1080 {
          background-color: #007bff;
        }

        .quality-badge.uhd {
          background-color: #6f42c1;
        }

        .col-size {
          width: 80px;
          text-align: right;
        }

        .col-peers {
          width: 80px;
          text-align: center;
        }

        .peers-info {
          display: flex;
          align-items: center;
          justify-content: center;
          gap: 0.25rem;
          font-size: 0.8rem;
        }

        .seeders {
          color: var(--color-success);
        }

        .leechers {
          color: var(--color-danger);
        }

        .col-age {
          width: 80px;
          text-align: right;
        }

        .col-actions {
          width: 72px;
          text-align: center;
        }

        .grab-btn {
          display: inline-flex;
          align-items: center;
          gap: 0.25rem;
          padding: 0.375rem 0.75rem;
          background-color: var(--color-success);
          border: none;
          border-radius: 0.25rem;
          color: white;
          font-size: 0.75rem;
          cursor: pointer;
        }

        .grab-btn:hover:not(:disabled) {
          opacity: 0.9;
        }

        .grab-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }

        .protocol-badge {
          display: inline-block;
          padding: 0.125rem 0.375rem;
          border-radius: 0.125rem;
          font-size: 0.625rem;
          font-weight: 500;
          text-transform: uppercase;
          background-color: var(--bg-card-alt);
          color: var(--text-color-muted);
          margin-left: 0.25rem;
        }

        .protocol-badge.torrent {
          background-color: rgba(40, 167, 69, 0.2);
          color: var(--color-success);
        }

        .protocol-badge.usenet {
          background-color: rgba(0, 123, 255, 0.2);
          color: var(--color-primary);
        }
      </style>
    `;
  }

  private renderSortIcon(column: 'quality' | 'age' | 'size' | 'seeders'): string {
    if (this.sortBy.value !== column) {
      return '';
    }
    const icon = this.sortDesc.value
      ? '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>'
      : '<svg class="sort-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>';
    return icon;
  }

  private renderRelease(release: ReleaseResource, grabbing: string | null): string {
    const qualityClass = this.getQualityClass(release.quality.quality.resolution);
    const isGrabbing = grabbing === release.guid;
    const isRejected = release.rejected;

    return html`
      <tr class="${isRejected ? 'release-rejected' : ''}">
        <td class="col-title">
          <div class="release-title" title="${escapeHtml(release.title)}">${escapeHtml(release.title)}</div>
          ${
            release.releaseGroup
              ? html`
            <div class="release-group">Group: ${escapeHtml(release.releaseGroup)}</div>
          `
              : ''
          }
          ${
            isRejected && release.rejections.length > 0
              ? html`
            <div class="release-rejections">${release.rejections.map((r) => escapeHtml(r)).join(', ')}</div>
          `
              : ''
          }
        </td>
        <td class="col-indexer">
          ${escapeHtml(release.indexer)}
          <span class="protocol-badge ${release.protocol}">${release.protocol}</span>
        </td>
        <td class="col-quality">
          <span class="quality-badge ${qualityClass}">${escapeHtml(release.quality.quality.name)}</span>
        </td>
        <td class="col-size">${this.formatSize(release.size)}</td>
        <td class="col-peers">
          ${
            release.protocol === 'torrent'
              ? html`
            <div class="peers-info">
              <span class="seeders">${release.seeders ?? '?'}</span>
              /
              <span class="leechers">${release.leechers ?? '?'}</span>
            </div>
          `
              : '-'
          }
        </td>
        <td class="col-age">${this.formatAge(release.age)}</td>
        <td class="col-actions">
          <button
            class="grab-btn"
            onclick="this.closest('release-search-modal').handleGrab('${release.guid}')"
            ${isGrabbing || isRejected ? 'disabled' : ''}
          >
            ${isGrabbing ? 'Grabbing...' : 'Grab'}
          </button>
        </td>
      </tr>
    `;
  }

  private getQualityClass(resolution: number): string {
    if (resolution >= 2160) return 'uhd';
    if (resolution >= 1080) return 'hd1080';
    if (resolution >= 720) return 'hd720';
    return 'sd';
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const gb = bytes / (1024 * 1024 * 1024);
    if (gb >= 1) return `${gb.toFixed(1)} GB`;
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(0)} MB`;
  }

  private formatAge(days: number): string {
    if (days === 0) return 'Today';
    if (days === 1) return '1 day';
    if (days < 7) return `${days} days`;
    if (days < 30) return `${Math.floor(days / 7)} weeks`;
    if (days < 365) return `${Math.floor(days / 30)} months`;
    return `${Math.floor(days / 365)} years`;
  }

  // Event handlers called from HTML
  handleSortClick(column: 'quality' | 'age' | 'size' | 'seeders'): void {
    this.toggleSort(column);
  }

  handleGrab(guid: string): void {
    const release = this.releases.value.find((r) => r.guid === guid);
    if (release) {
      this.grabRelease(release);
    }
  }

  retrySearch(): void {
    this.search();
  }
}
