/**
 * Music index page - artist grid/table view
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { type Artist, http } from '../../core/http';
import { useArtistsQuery } from '../../core/query';
import { navigate } from '../../router';
import {
  type MusicSortKey,
  musicFilter,
  musicSortDirection,
  musicSortKey,
  musicViewMode,
  searchQuery,
  setMusicFilter,
  setMusicSort,
  setMusicViewMode,
  showError,
  showInfo,
  type ViewMode,
} from '../../stores/app.store';

@customElement('music-index-page')
export class MusicIndexPage extends BaseComponent {
  private artistsQuery = useArtistsQuery();

  protected onInit(): void {
    this.watch(this.artistsQuery.data);
    this.watch(this.artistsQuery.isLoading);
    this.watch(this.artistsQuery.isError);
    this.watch(musicViewMode);
    this.watch(musicSortKey);
    this.watch(musicSortDirection);
    this.watch(musicFilter);
    this.watch(searchQuery);
  }

  protected template(): string {
    const artists = this.artistsQuery.data.value ?? [];
    const isLoading = this.artistsQuery.isLoading.value;
    const isError = this.artistsQuery.isError.value;
    const viewMode = musicViewMode.value;
    const sortKey = musicSortKey.value;
    const sortDir = musicSortDirection.value;
    const filter = musicFilter.value;
    const search = searchQuery.value.toLowerCase();

    let filtered = artists;

    if (search) {
      filtered = filtered.filter((a) => a.title.toLowerCase().includes(search));
    }

    if (filter !== 'all') {
      filtered = filtered.filter((a) => {
        switch (filter) {
          case 'monitored':
            return a.monitored;
          case 'unmonitored':
            return !a.monitored;
          case 'continuing':
            return a.status === 'continuing';
          case 'ended':
            return a.status === 'ended';
          default:
            return true;
        }
      });
    }

    filtered = [...filtered].sort((a, b) => {
      let comparison = 0;
      const aVal = this.getSortValue(a, sortKey);
      const bVal = this.getSortValue(b, sortKey);

      if (aVal < bVal) comparison = -1;
      if (aVal > bVal) comparison = 1;

      return sortDir === 'descending' ? -comparison : comparison;
    });

    return html`
      <div class="music-page">
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Music</h1>
            <span class="item-count">${filtered.length} artists</span>
          </div>

          <div class="toolbar-right">
            <select
              class="filter-select"
              onchange="this.closest('music-index-page').handleFilterChange(event)"
            >
              <option value="all" ${filter === 'all' ? 'selected' : ''}>All</option>
              <option value="monitored" ${filter === 'monitored' ? 'selected' : ''}>Monitored</option>
              <option value="unmonitored" ${filter === 'unmonitored' ? 'selected' : ''}>Unmonitored</option>
              <option value="continuing" ${filter === 'continuing' ? 'selected' : ''}>Continuing</option>
              <option value="ended" ${filter === 'ended' ? 'selected' : ''}>Ended</option>
            </select>

            <select
              class="sort-select"
              onchange="this.closest('music-index-page').handleSortChange(event)"
            >
              <option value="sortTitle" ${sortKey === 'sortTitle' ? 'selected' : ''}>Title</option>
              <option value="status" ${sortKey === 'status' ? 'selected' : ''}>Status</option>
              <option value="added" ${sortKey === 'added' ? 'selected' : ''}>Added</option>
              <option value="sizeOnDisk" ${sortKey === 'sizeOnDisk' ? 'selected' : ''}>Size</option>
            </select>

            <button
              class="sort-dir-btn"
              onclick="this.closest('music-index-page').handleSortDirToggle()"
              title="${sortDir === 'ascending' ? 'Ascending' : 'Descending'}"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                   class="${sortDir === 'descending' ? 'rotate-180' : ''}">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </button>

            <button
              class="refresh-all-btn"
              onclick="this.closest('music-index-page').handleRefreshAll()"
              title="Refresh all artist metadata"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="23 4 23 10 17 10"></polyline>
                <polyline points="1 20 1 14 7 14"></polyline>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
              </svg>
              <span>Refresh All</span>
            </button>

            <button
              class="rescan-all-btn"
              onclick="this.closest('music-index-page').handleRescanAll()"
              title="Rescan disk files for all artists"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 12h-4l-3 9L9 3l-3 9H2"></path>
              </svg>
              <span>Rescan All</span>
            </button>

            <div class="view-modes">
              ${this.renderViewModeButton('posters', 'Posters')}
              ${this.renderViewModeButton('table', 'Table')}
            </div>

            <button
              class="add-btn"
              onclick="this.closest('music-index-page').handleAddArtist()"
              title="Add Artist"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="12" y1="5" x2="12" y2="19"></line>
                <line x1="5" y1="12" x2="19" y2="12"></line>
              </svg>
              Add Artist
            </button>
          </div>
        </div>

        ${isLoading ? this.renderLoading() : ''}
        ${isError ? this.renderError() : ''}
        ${!isLoading && !isError ? this.renderContent(filtered, viewMode) : ''}
      </div>

      ${this.styles()}
    `;
  }

  private styles(): string {
    return html`
      <style>
        .music-page {
          display: flex;
          flex-direction: column;
          gap: 1.25rem;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from { opacity: 0; transform: translateY(12px); }
          to { opacity: 1; transform: translateY(0); }
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
          flex-wrap: wrap;
          gap: 1rem;
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .toolbar-left {
          display: flex;
          align-items: baseline;
          gap: 1rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
          background: linear-gradient(135deg, var(--text-color) 0%, var(--pir9-blue) 100%);
          -webkit-background-clip: text;
          -webkit-text-fill-color: transparent;
          background-clip: text;
        }

        .item-count {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          padding: 0.25rem 0.625rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
        }

        .toolbar-right {
          display: flex;
          align-items: center;
          gap: 0.625rem;
        }

        .filter-select,
        .sort-select {
          padding: 0.5rem 0.875rem;
          background-color: var(--bg-input);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          font-size: 0.875rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .filter-select:hover,
        .sort-select:hover {
          border-color: var(--border-glass);
          background-color: var(--bg-input-hover);
        }

        .sort-dir-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.5rem;
          background-color: var(--bg-input);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .sort-dir-btn:hover {
          background-color: var(--bg-input-hover);
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .sort-dir-btn svg.rotate-180 { transform: rotate(180deg); }

        .refresh-all-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.875rem;
          background: var(--btn-primary-bg);
          color: var(--color-white);
          border: none;
          border-radius: 0.5rem;
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .refresh-all-btn:hover {
          background: var(--btn-primary-bg-hover);
          box-shadow: var(--glow-primary);
          transform: translateY(-1px);
        }

        .rescan-all-btn {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.5rem 0.875rem;
          background: var(--bg-input);
          color: var(--text-primary);
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .rescan-all-btn:hover {
          background: var(--bg-card-hover);
          border-color: var(--color-primary);
          transform: translateY(-1px);
        }

        .view-modes {
          display: flex;
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          overflow: hidden;
          background: var(--bg-input);
        }

        .view-mode-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.5rem 0.625rem;
          background-color: transparent;
          color: var(--text-color-muted);
          border: none;
          cursor: pointer;
          transition: all var(--transition-fast) var(--ease-out-expo);
        }

        .view-mode-btn:not(:last-child) {
          border-right: 1px solid var(--border-input);
        }

        .view-mode-btn.active {
          background: linear-gradient(135deg, var(--color-primary), var(--pir9-blue));
          color: var(--color-white);
          box-shadow: 0 2px 8px rgba(93, 156, 236, 0.4);
        }

        .add-btn {
          display: flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 0.875rem;
          background: linear-gradient(135deg, var(--color-primary), var(--pir9-blue));
          color: var(--color-white);
          border: none;
          border-radius: 0.5rem;
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
          box-shadow: 0 2px 8px rgba(93, 156, 236, 0.3);
        }

        .add-btn:hover {
          box-shadow: 0 4px 16px rgba(93, 156, 236, 0.5);
          transform: translateY(-1px);
        }

        .loading-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 1rem;
          padding: 6rem 2rem;
        }

        .loading-spinner {
          width: 48px;
          height: 48px;
          border: 3px solid var(--border-glass);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }

        @keyframes spin { to { transform: rotate(360deg); } }

        .error-container,
        .empty-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1.25rem;
          padding: 6rem 2rem;
          text-align: center;
        }

        .poster-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
          gap: 1.25rem;
        }

        .poster-card {
          position: relative;
          border-radius: 0.875rem;
          overflow: hidden;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          box-shadow: var(--shadow-card);
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .poster-card:hover {
          transform: translateY(-6px) scale(1.02);
          box-shadow: var(--shadow-card-hover), 0 0 30px rgba(93, 156, 236, 0.15);
          border-color: rgba(93, 156, 236, 0.3);
        }

        .poster-image {
          width: 100%;
          aspect-ratio: 1/1;
          object-fit: cover;
          background-color: var(--bg-card-center);
        }

        .poster-placeholder {
          width: 100%;
          aspect-ratio: 1/1;
          display: flex;
          align-items: center;
          justify-content: center;
          background: linear-gradient(135deg, var(--bg-card-center), var(--bg-card));
          color: var(--text-color-muted);
        }

        .poster-info {
          padding: 0.75rem;
          background: linear-gradient(to top, rgba(0,0,0,0.6), transparent);
          position: absolute;
          bottom: 0;
          left: 0;
          right: 0;
        }

        .poster-title {
          font-size: 0.875rem;
          font-weight: 600;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          color: #fff;
          text-shadow: 0 1px 3px rgba(0,0,0,0.5);
        }

        .poster-meta {
          font-size: 0.75rem;
          color: rgba(255,255,255,0.8);
        }

        .poster-status {
          position: absolute;
          top: 0.5rem;
          right: 0.5rem;
          width: 10px;
          height: 10px;
          border-radius: 50%;
          z-index: 2;
          box-shadow: 0 0 8px currentColor;
        }

        .poster-status.continuing { background-color: var(--color-success); color: var(--color-success); }
        .poster-status.ended { background-color: var(--color-gray-600); color: var(--color-gray-600); }

        .artist-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          overflow: hidden;
        }

        .artist-table th,
        .artist-table td {
          padding: 0.875rem 1rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color-light);
        }

        .artist-table th {
          font-weight: 600;
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-color-muted);
          background: var(--bg-card-center);
        }

        .artist-table th.sortable {
          cursor: pointer;
          user-select: none;
          transition: color var(--transition-fast);
        }

        .artist-table th.sortable:hover {
          color: var(--pir9-blue);
          background: var(--bg-input-hover);
        }

        .artist-table th.sortable.sorted {
          color: var(--pir9-blue);
        }

        .artist-table th.sortable svg {
          display: inline-block;
          vertical-align: middle;
          margin-left: 0.25rem;
        }

        .artist-table tr {
          cursor: pointer;
          transition: background var(--transition-fast);
        }

        .artist-table tr:hover {
          background-color: var(--bg-hover);
        }

        .title-cell {
          font-weight: 500;
          color: var(--text-color);
        }

        .status-badge {
          display: inline-block;
          padding: 0.2rem 0.5rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
        }

        .status-badge.continuing { background: rgba(39, 174, 96, 0.15); color: var(--color-success); }
        .status-badge.ended { background: rgba(150, 150, 150, 0.15); color: var(--text-color-muted); }
      </style>
    `;
  }

  private renderContent(artists: Artist[], viewMode: ViewMode): string {
    if (artists.length === 0) {
      return this.renderEmpty();
    }

    if (viewMode === 'table') {
      return this.renderTable(artists);
    }

    return this.renderPosterGrid(artists);
  }

  private renderPosterGrid(artists: Artist[]): string {
    return html`
      <div class="poster-grid">
        ${artists.map((a) => this.renderPosterCard(a)).join('')}
      </div>
    `;
  }

  private renderPosterCard(artist: Artist): string {
    const posterImage = artist.images?.find((i) => i.coverType === 'poster');
    const statusClass = artist.monitored ? artist.status : 'unmonitored';
    const albumCount = artist.statistics?.albumCount ?? 0;

    return html`
      <div class="poster-card"
           onclick="this.closest('music-index-page').handleArtistClick('${escapeHtml(artist.titleSlug)}')">
        <div class="poster-status ${statusClass}"></div>
        ${
          posterImage
            ? `<img class="poster-image" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(artist.title)}" loading="lazy">`
            : `<div class="poster-placeholder">
              <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                <circle cx="12" cy="12" r="10"></circle>
                <path d="M9 18V5l12-2v13"></path>
                <circle cx="6" cy="18" r="3"></circle>
                <circle cx="18" cy="16" r="3"></circle>
              </svg>
            </div>`
        }
        <div class="poster-info">
          <div class="poster-title">${escapeHtml(artist.title)}</div>
          <div class="poster-meta">${albumCount} album${albumCount !== 1 ? 's' : ''}</div>
        </div>
      </div>
    `;
  }

  private renderTable(artists: Artist[]): string {
    const sortKey = musicSortKey.value;
    const sortDir = musicSortDirection.value;
    const sortIcon =
      sortDir === 'ascending'
        ? '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
        : '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>';

    return html`
      <table class="artist-table">
        <thead>
          <tr>
            <th class="sortable ${sortKey === 'sortTitle' ? 'sorted' : ''}" onclick="this.closest('music-index-page').handleColumnSort('sortTitle')">
              Artist ${sortKey === 'sortTitle' ? safeHtml(sortIcon) : ''}
            </th>
            <th>Albums</th>
            <th>Tracks</th>
            <th class="sortable ${sortKey === 'status' ? 'sorted' : ''}" onclick="this.closest('music-index-page').handleColumnSort('status')">
              Status ${sortKey === 'status' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'sizeOnDisk' ? 'sorted' : ''}" onclick="this.closest('music-index-page').handleColumnSort('sizeOnDisk')">
              Size ${sortKey === 'sizeOnDisk' ? safeHtml(sortIcon) : ''}
            </th>
          </tr>
        </thead>
        <tbody>
          ${artists
            .map(
              (a) => html`
            <tr onclick="this.closest('music-index-page').handleArtistClick('${escapeHtml(a.titleSlug)}')">
              <td class="title-cell">${escapeHtml(a.title)}</td>
              <td>${a.statistics?.albumCount ?? 0}</td>
              <td>${a.statistics?.trackFileCount ?? 0} / ${a.statistics?.totalTrackCount ?? 0}</td>
              <td><span class="status-badge ${a.status}">${a.status}</span></td>
              <td>${this.formatSize(a.statistics?.sizeOnDisk ?? 0)}</td>
            </tr>
          `,
            )
            .join('')}
        </tbody>
      </table>
    `;
  }

  private renderLoading(): string {
    return html`
      <div class="loading-container">
        <div class="loading-spinner"></div>
        <span>Loading artists...</span>
      </div>
    `;
  }

  private renderError(): string {
    return html`
      <div class="error-container">
        <span>Failed to load artists</span>
        <button onclick="this.closest('music-index-page').handleRetry()">Retry</button>
      </div>
    `;
  }

  private renderEmpty(): string {
    return html`
      <div class="empty-container">
        <svg width="72" height="72" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1" style="color: var(--text-color-dim)">
          <circle cx="12" cy="12" r="10"></circle>
          <path d="M9 18V5l12-2v13"></path>
          <circle cx="6" cy="18" r="3"></circle>
          <circle cx="18" cy="16" r="3"></circle>
        </svg>
        <p style="color: var(--text-color-muted)">No artists found. Add an artist to get started.</p>
        <button
          class="add-btn"
          onclick="this.closest('music-index-page').handleAddArtist()"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="12" y1="5" x2="12" y2="19"></line>
            <line x1="5" y1="12" x2="19" y2="12"></line>
          </svg>
          Add Artist
        </button>
      </div>
    `;
  }

  private renderViewModeButton(mode: ViewMode, label: string): string {
    const active = musicViewMode.value === mode;
    const icons: Record<string, string> = {
      posters:
        '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>',
      table:
        '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>',
    };

    return html`
      <button
        class="view-mode-btn ${active ? 'active' : ''}"
        onclick="this.closest('music-index-page').handleViewModeChange('${mode}')"
        title="${label}"
      >
        ${icons[mode] || ''}
      </button>
    `;
  }

  private getSortValue(artist: Artist, key: MusicSortKey): string | number {
    switch (key) {
      case 'sortTitle':
        return artist.sortTitle || artist.title.toLowerCase();
      case 'status':
        return artist.status;
      case 'added':
        return artist.added || '';
      case 'sizeOnDisk':
        return artist.statistics?.sizeOnDisk ?? 0;
      default:
        return artist.sortTitle || '';
    }
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
  }

  // Event handlers
  handleArtistClick(titleSlug: string): void {
    navigate(`/music/${titleSlug}`);
  }

  handleFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMusicFilter(select.value);
  }

  handleSortChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setMusicSort(select.value as MusicSortKey);
  }

  handleColumnSort(key: string): void {
    if (musicSortKey.value === key) {
      const current = musicSortDirection.value;
      musicSortDirection.set(current === 'ascending' ? 'descending' : 'ascending');
    } else {
      setMusicSort(key as MusicSortKey);
      musicSortDirection.set('ascending');
    }
  }

  handleSortDirToggle(): void {
    setMusicSort(musicSortKey.value);
  }

  handleViewModeChange(mode: string): void {
    setMusicViewMode(mode as ViewMode);
  }

  handleAddArtist(): void {
    navigate('/add-music');
  }

  async handleRefreshAll(): Promise<void> {
    try {
      await http.post('/command', { name: 'RefreshArtist' });
      showInfo('Refreshing all artist metadata...', 'Refresh Started');
    } catch {
      showError('Failed to start refresh command', 'Refresh Failed');
    }
  }

  async handleRescanAll(): Promise<void> {
    try {
      await http.post('/command', { name: 'RescanArtist' });
      showInfo('Rescanning all artist files...', 'Rescan Started');
    } catch {
      showError('Failed to start rescan command', 'Rescan Failed');
    }
  }

  handleRetry(): void {
    this.artistsQuery.refetch();
  }
}
