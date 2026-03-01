/**
 * Series index page - main grid/table view
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { getRootFolder, http, type Series } from '../../core/http';
import { useSeriesQuery } from '../../core/query';
import { navigate } from '../../router';
import {
  type SeriesSortKey,
  searchQuery,
  seriesFilter,
  seriesNetworkFilter,
  seriesRootFolderFilter,
  seriesSortDirection,
  seriesSortKey,
  seriesViewMode,
  setSeriesFilter,
  setSeriesNetworkFilter,
  setSeriesRootFolderFilter,
  setSeriesSort,
  setSeriesViewMode,
  showError,
  showInfo,
  showSuccess,
  type ViewMode,
} from '../../stores/app.store';

@customElement('series-index-page')
export class SeriesIndexPage extends BaseComponent {
  private seriesQuery = useSeriesQuery();

  protected onInit(): void {
    this.watch(this.seriesQuery.data);
    this.watch(this.seriesQuery.isLoading);
    this.watch(this.seriesQuery.isError);
    this.watch(seriesViewMode);
    this.watch(seriesSortKey);
    this.watch(seriesSortDirection);
    this.watch(seriesFilter);
    this.watch(seriesNetworkFilter);
    this.watch(seriesRootFolderFilter);
    this.watch(searchQuery);
  }

  protected template(): string {
    const series = this.seriesQuery.data.value ?? [];
    const isLoading = this.seriesQuery.isLoading.value;
    const isError = this.seriesQuery.isError.value;
    const viewMode = seriesViewMode.value;
    const sortKey = seriesSortKey.value;
    const sortDir = seriesSortDirection.value;
    const filter = seriesFilter.value;
    const networkFilter = seriesNetworkFilter.value;
    const rootFolderFilter = seriesRootFolderFilter.value;
    const search = searchQuery.value.toLowerCase();

    // Exclude anime series (they have their own page at /anime)
    const nonAnimeSeries = series.filter((s) => s.seriesType !== 'anime');

    // Collect unique root folders (before filtering, so dropdown is always complete)
    const rootFolders = [...new Set(nonAnimeSeries.map((s) => getRootFolder(s.path)))].sort();

    // Collect unique networks (before filtering, so dropdown is always complete)
    const networks = [...new Set(nonAnimeSeries.map((s) => s.network || 'Unknown Network'))].sort(
      (a, b) => {
        if (a === 'Unknown Network') return 1;
        if (b === 'Unknown Network') return -1;
        return a.localeCompare(b);
      },
    );

    // Filter and sort series
    let filtered = nonAnimeSeries;

    // Apply search filter
    if (search) {
      filtered = filtered.filter(
        (s) => s.title.toLowerCase().includes(search) || s.network?.toLowerCase().includes(search),
      );
    }

    // Apply status filter
    if (filter !== 'all') {
      filtered = filtered.filter((s) => {
        switch (filter) {
          case 'monitored':
            return s.monitored;
          case 'unmonitored':
            return !s.monitored;
          case 'continuing':
            return s.status === 'continuing';
          case 'ended':
            return s.status === 'ended';
          default:
            return true;
        }
      });
    }

    // Apply network filter
    if (networkFilter !== 'all') {
      filtered = filtered.filter((s) => (s.network || 'Unknown Network') === networkFilter);
    }

    // Apply root folder filter
    if (rootFolderFilter !== 'all') {
      filtered = filtered.filter((s) => getRootFolder(s.path) === rootFolderFilter);
    }

    // Sort
    const isDateSort = sortKey === 'nextAiring' || sortKey === 'previousAiring';
    filtered = [...filtered].sort((a, b) => {
      let comparison = 0;
      const aVal = this.getSortValue(a, sortKey);
      const bVal = this.getSortValue(b, sortKey);

      // Push missing date values to the bottom regardless of direction
      if (isDateSort) {
        if (aVal === '' && bVal !== '') return 1;
        if (aVal !== '' && bVal === '') return -1;
      }

      if (aVal < bVal) comparison = -1;
      if (aVal > bVal) comparison = 1;

      return sortDir === 'descending' ? -comparison : comparison;
    });

    return html`
      <div class="series-page">
        <!-- Toolbar -->
        <div class="toolbar">
          <div class="toolbar-left">
            <h1 class="page-title">Series</h1>
            <span class="series-count">${filtered.length} series</span>
          </div>

          <div class="toolbar-right">
            <!-- Filter dropdown -->
            <select
              class="filter-select"
              value="${filter}"
              onchange="this.closest('series-index-page').handleFilterChange(event)"
            >
              <option value="all">All</option>
              <option value="monitored">Monitored</option>
              <option value="unmonitored">Unmonitored</option>
              <option value="continuing">Continuing</option>
              <option value="ended">Ended</option>
            </select>

            <!-- Network filter dropdown -->
            <select
              class="filter-select"
              onchange="this.closest('series-index-page').handleNetworkFilterChange(event)"
            >
              <option value="all" ${networkFilter === 'all' ? 'selected' : ''}>All Networks</option>
              ${networks.map((n) => html`<option value="${escapeHtml(n)}" ${networkFilter === n ? 'selected' : ''}>${escapeHtml(n)}</option>`).join('')}
            </select>

            <!-- Root folder filter dropdown -->
            ${
              rootFolders.length > 1
                ? html`
            <select
              class="filter-select"
              onchange="this.closest('series-index-page').handleRootFolderFilterChange(event)"
            >
              <option value="all" ${rootFolderFilter === 'all' ? 'selected' : ''}>All Folders</option>
              ${rootFolders.map((f) => html`<option value="${escapeHtml(f)}" ${rootFolderFilter === f ? 'selected' : ''}>${escapeHtml(f)}</option>`).join('')}
            </select>
            `
                : ''
            }

            <!-- Sort dropdown -->
            <select
              class="sort-select"
              value="${sortKey}"
              onchange="this.closest('series-index-page').handleSortChange(event)"
            >
              <option value="sortTitle">Title</option>
              <option value="status">Status</option>
              <option value="network">Network</option>
              <option value="nextAiring">Next Airing</option>
              <option value="previousAiring">Previous Airing</option>
              <option value="added">Added</option>
              <option value="year">Year</option>
              <option value="episodeProgress">Episodes</option>
              <option value="sizeOnDisk">Size</option>
            </select>

            <!-- Sort direction -->
            <button
              class="sort-dir-btn"
              onclick="this.closest('series-index-page').handleSortDirToggle()"
              title="${sortDir === 'ascending' ? 'Ascending' : 'Descending'}"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
                   class="${sortDir === 'descending' ? 'rotate-180' : ''}">
                <polyline points="18 15 12 9 6 15"></polyline>
              </svg>
            </button>

            <!-- Refresh All button -->
            <button
              class="refresh-all-btn"
              onclick="this.closest('series-index-page').handleRefreshAll()"
              title="Refresh all series metadata from Skyhook"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="23 4 23 10 17 10"></polyline>
                <polyline points="1 20 1 14 7 14"></polyline>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
              </svg>
              <span>Refresh All</span>
            </button>

            <!-- View mode buttons -->
            <div class="view-modes">
              ${this.renderViewModeButton('posters', 'Posters')}
              ${this.renderViewModeButton('table', 'Table')}
            </div>
          </div>
        </div>

        <!-- Content -->
        ${isLoading ? this.renderLoading() : ''}
        ${isError ? this.renderError() : ''}
        ${!isLoading && !isError ? this.renderContent(filtered, viewMode) : ''}
      </div>

      <style>
        .series-page {
          display: flex;
          flex-direction: column;
          gap: 1.25rem;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from {
            opacity: 0;
            transform: translateY(12px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
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
          -webkit-backdrop-filter: blur(var(--glass-blur));
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

        .series-count {
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
          backdrop-filter: blur(8px);
          -webkit-backdrop-filter: blur(8px);
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

        .filter-select:focus,
        .sort-select:focus {
          outline: none;
          border-color: var(--border-input-focus);
          box-shadow: var(--shadow-input-focus);
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

        .sort-dir-btn svg {
          transition: transform var(--transition-normal) var(--ease-spring);
        }

        .sort-dir-btn svg.rotate-180 {
          transform: rotate(180deg);
        }

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

        .refresh-all-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
          transform: none;
        }

        .refresh-all-btn.loading svg {
          animation: spin 1s linear infinite;
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
          position: relative;
        }

        .view-mode-btn:not(:last-child) {
          border-right: 1px solid var(--border-input);
        }

        .view-mode-btn.active {
          background: linear-gradient(135deg, var(--color-primary), var(--pir9-blue));
          color: var(--color-white);
          box-shadow: 0 2px 8px rgba(93, 156, 236, 0.4);
        }

        .view-mode-btn:hover:not(.active) {
          background-color: var(--bg-input-hover);
          color: var(--pir9-blue);
        }

        /* Loading state */
        .loading-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 1rem;
          padding: 6rem 2rem;
          animation: fadeIn var(--transition-normal) var(--ease-out-expo);
        }

        .loading-spinner {
          width: 48px;
          height: 48px;
          border: 3px solid var(--border-glass);
          border-top-color: var(--color-primary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
          box-shadow: 0 0 20px rgba(93, 156, 236, 0.3);
        }

        @keyframes spin {
          to { transform: rotate(360deg); }
        }

        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }

        /* Error state */
        .error-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1.25rem;
          padding: 6rem 2rem;
          text-align: center;
          animation: fadeIn var(--transition-normal) var(--ease-out-expo);
        }

        .error-icon {
          width: 56px;
          height: 56px;
          color: var(--color-danger);
          filter: drop-shadow(0 0 12px rgba(240, 80, 80, 0.4));
        }

        .error-message {
          color: var(--text-color-muted);
          font-size: 1rem;
        }

        .retry-btn {
          padding: 0.625rem 1.25rem;
          background: var(--btn-primary-bg);
          color: var(--color-white);
          border: none;
          border-radius: 0.5rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
          font-weight: 500;
        }

        .retry-btn:hover {
          background: var(--btn-primary-bg-hover);
          box-shadow: var(--glow-primary);
          transform: translateY(-1px);
        }

        /* Empty state */
        .empty-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1.25rem;
          padding: 6rem 2rem;
          text-align: center;
          animation: fadeIn var(--transition-normal) var(--ease-out-expo);
        }

        .empty-icon {
          width: 72px;
          height: 72px;
          color: var(--text-color-dim);
        }

        .add-series-btn {
          padding: 0.625rem 1.25rem;
          background: var(--btn-primary-bg);
          color: var(--color-white);
          border: none;
          border-radius: 0.5rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
          font-weight: 500;
        }

        .add-series-btn:hover {
          background: var(--btn-primary-bg-hover);
          box-shadow: var(--glow-primary);
          transform: translateY(-1px);
        }

        /* Poster grid - Glassmorphism cards */
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
          -webkit-backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          box-shadow: var(--shadow-card);
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
          animation: slideUp var(--transition-normal) var(--ease-out-expo) backwards;
        }

        .poster-card:nth-child(1) { animation-delay: 0ms; }
        .poster-card:nth-child(2) { animation-delay: 30ms; }
        .poster-card:nth-child(3) { animation-delay: 60ms; }
        .poster-card:nth-child(4) { animation-delay: 90ms; }
        .poster-card:nth-child(5) { animation-delay: 120ms; }
        .poster-card:nth-child(6) { animation-delay: 150ms; }
        .poster-card:nth-child(7) { animation-delay: 180ms; }
        .poster-card:nth-child(8) { animation-delay: 210ms; }

        @keyframes slideUp {
          from {
            opacity: 0;
            transform: translateY(20px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }

        .poster-card::before {
          content: '';
          position: absolute;
          inset: 0;
          background: linear-gradient(
            135deg,
            rgba(255, 255, 255, 0.1) 0%,
            transparent 50%
          );
          opacity: 0;
          transition: opacity var(--transition-fast);
          pointer-events: none;
          z-index: 1;
        }

        .poster-card:hover {
          transform: translateY(-6px) scale(1.02);
          box-shadow: var(--shadow-card-hover), 0 0 30px rgba(93, 156, 236, 0.15);
          border-color: rgba(93, 156, 236, 0.3);
        }

        .poster-card:hover::before {
          opacity: 1;
        }

        .poster-image {
          width: 100%;
          aspect-ratio: 2/3;
          object-fit: cover;
          background-color: var(--bg-card-center);
          transition: transform var(--transition-normal) var(--ease-out-expo);
        }

        .poster-card:hover .poster-image {
          transform: scale(1.05);
        }

        .poster-placeholder {
          width: 100%;
          aspect-ratio: 2/3;
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

        .poster-network {
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

        .poster-status.continuing {
          background-color: var(--color-success);
          color: var(--color-success);
        }

        .poster-status.ended {
          background-color: var(--color-danger);
          color: var(--color-danger);
        }

        .poster-status.upcoming {
          background-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .poster-status.unknown,
        .poster-status.deleted,
        .poster-status.unmonitored {
          background-color: var(--color-gray-600);
          color: var(--color-gray-600);
        }

        /* Table view - Glass table */
        .series-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          -webkit-backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          overflow: hidden;
        }

        .series-table th,
        .series-table td {
          padding: 0.875rem 1rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color-light);
        }

        .series-table th {
          font-weight: 600;
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-color-muted);
          background: var(--bg-card-alt);
        }

        .series-table th.sortable {
          cursor: pointer;
          user-select: none;
          transition: all var(--transition-fast) var(--ease-out-expo);
        }

        .series-table th.sortable:hover {
          color: var(--pir9-blue);
          background: var(--bg-input-hover);
        }

        .series-table th.sortable.sorted {
          color: var(--pir9-blue);
        }

        .series-table th.sortable svg {
          display: inline-block;
          vertical-align: middle;
          margin-left: 0.25rem;
        }

        .series-table tr {
          cursor: pointer;
          transition: background-color var(--transition-fast);
        }

        .series-table tbody tr:hover td {
          background-color: var(--bg-table-row-hover);
        }

        .series-table tbody tr:last-child td {
          border-bottom: none;
        }

        .status-badge {
          display: inline-flex;
          align-items: center;
          padding: 0.25rem 0.625rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
          border: 1px solid;
        }

        .status-badge.continuing {
          background-color: rgba(39, 194, 76, 0.15);
          border-color: rgba(39, 194, 76, 0.3);
          color: var(--color-success);
        }

        .status-badge.ended {
          background-color: rgba(240, 80, 80, 0.15);
          border-color: rgba(240, 80, 80, 0.3);
          color: var(--color-danger);
        }

        .status-badge.upcoming {
          background-color: rgba(93, 156, 236, 0.15);
          border-color: rgba(93, 156, 236, 0.3);
          color: var(--pir9-blue);
        }

        .status-badge.unknown,
        .status-badge.deleted {
          background-color: rgba(133, 133, 133, 0.15);
          border-color: rgba(133, 133, 133, 0.3);
          color: var(--color-gray-600);
        }

        .monitored-icon {
          width: 18px;
          height: 18px;
          transition: all var(--transition-fast);
        }

        .monitored-icon.true {
          color: var(--color-success);
          filter: drop-shadow(0 0 4px rgba(39, 194, 76, 0.4));
        }
        .monitored-icon.false {
          color: var(--color-gray-600);
        }

        /* Episode progress */
        .episode-progress {
          min-width: 120px;
        }

        .episode-progress-bar {
          height: 6px;
          background-color: var(--bg-progress, rgba(255,255,255,0.08));
          border-radius: 3px;
          overflow: hidden;
        }

        .episode-progress-fill {
          height: 100%;
          border-radius: 3px;
          transition: width var(--transition-normal) var(--ease-out-expo);
        }

        .episode-progress-fill.complete {
          background-color: var(--color-success);
        }

        .episode-progress-fill.partial {
          background-color: var(--color-primary);
        }

        .episode-progress-fill.empty {
          background-color: var(--color-gray-600);
        }

        .episode-progress-text {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin-top: 2px;
        }

        .airing-date {
          white-space: nowrap;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
        }

      </style>
    `;
  }

  private renderViewModeButton(mode: ViewMode, label: string): string {
    const active = seriesViewMode.value === mode;
    const icon =
      mode === 'posters'
        ? '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>'
        : '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"></line><line x1="8" y1="12" x2="21" y2="12"></line><line x1="8" y1="18" x2="21" y2="18"></line><line x1="3" y1="6" x2="3.01" y2="6"></line><line x1="3" y1="12" x2="3.01" y2="12"></line><line x1="3" y1="18" x2="3.01" y2="18"></line></svg>';

    return html`
      <button
        class="view-mode-btn ${active ? 'active' : ''}"
        onclick="this.closest('series-index-page').handleViewModeChange('${mode}')"
        title="${label}"
      >
        ${safeHtml(icon)}
      </button>
    `;
  }

  private renderLoading(): string {
    return html`
      <div class="loading-container">
        <div class="loading-spinner"></div>
      </div>
    `;
  }

  private renderError(): string {
    return html`
      <div class="error-container">
        <svg class="error-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="10"></circle>
          <line x1="15" y1="9" x2="9" y2="15"></line>
          <line x1="9" y1="9" x2="15" y2="15"></line>
        </svg>
        <p class="error-message">Failed to load series</p>
        <button class="retry-btn" onclick="this.closest('series-index-page').handleRetry()">
          Retry
        </button>
      </div>
    `;
  }

  private renderContent(series: Series[], viewMode: ViewMode): string {
    if (series.length === 0) {
      return this.renderEmpty();
    }

    if (viewMode === 'table') {
      return this.renderTable(series);
    }
    return this.renderGrid(series);
  }

  private renderEmpty(): string {
    return html`
      <div class="empty-container">
        <svg class="empty-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <rect x="2" y="7" width="20" height="15" rx="2" ry="2"></rect>
          <polyline points="17 2 12 7 7 2"></polyline>
        </svg>
        <p>No series found</p>
        <button class="add-series-btn" onclick="this.closest('series-index-page').handleAddSeries()">
          Add Series
        </button>
      </div>
    `;
  }

  private renderGrid(series: Series[]): string {
    return html`
      <div class="poster-grid">
        ${series.map((s) => this.renderPosterCard(s)).join('')}
      </div>
    `;
  }

  private renderPosterCard(series: Series): string {
    const poster = series.images?.find((i) => i.coverType === 'poster');
    const statusClass = series.monitored ? series.status : 'unmonitored';

    return html`
      <div
        class="poster-card"
        onclick="this.closest('series-index-page').handleSeriesClick('${escapeHtml(series.titleSlug)}')"
      >
        <div class="poster-status ${statusClass}"></div>
        ${
          poster
            ? `<img class="poster-image" src="${escapeHtml(poster.url)}" alt="${escapeHtml(series.title)}" loading="lazy" />`
            : '<div class="poster-placeholder"><svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><rect x="2" y="7" width="20" height="15" rx="2" ry="2"></rect><polyline points="17 2 12 7 7 2"></polyline></svg></div>'
        }
        <div class="poster-info">
          <div class="poster-title" title="${escapeHtml(series.title)}">${escapeHtml(series.title)}</div>
          ${series.network ? `<div class="poster-network">${escapeHtml(series.network)}</div>` : ''}
        </div>
      </div>
    `;
  }

  private renderTable(series: Series[]): string {
    const sortKey = seriesSortKey.value;
    const sortDir = seriesSortDirection.value;
    const sortIcon =
      sortDir === 'ascending'
        ? '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="18 15 12 9 6 15"></polyline></svg>'
        : '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="6 9 12 15 18 9"></polyline></svg>';

    return html`
      <table class="series-table">
        <thead>
          <tr>
            <th>Monitored</th>
            <th class="sortable ${sortKey === 'sortTitle' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('sortTitle')">
              Title ${sortKey === 'sortTitle' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'network' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('network')">
              Network ${sortKey === 'network' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'status' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('status')">
              Status ${sortKey === 'status' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'nextAiring' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('nextAiring')">
              Next Airing ${sortKey === 'nextAiring' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'previousAiring' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('previousAiring')">
              Prev Airing ${sortKey === 'previousAiring' ? safeHtml(sortIcon) : ''}
            </th>
            <th class="sortable ${sortKey === 'year' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('year')">
              Year ${sortKey === 'year' ? safeHtml(sortIcon) : ''}
            </th>
            <th>Seasons</th>
            <th class="sortable ${sortKey === 'episodeProgress' ? 'sorted' : ''}" onclick="this.closest('series-index-page').handleHeaderSort('episodeProgress')">
              Episodes ${sortKey === 'episodeProgress' ? safeHtml(sortIcon) : ''}
            </th>
          </tr>
        </thead>
        <tbody>
          ${series.map((s) => this.renderTableRow(s)).join('')}
        </tbody>
      </table>
    `;
  }

  private renderTableRow(series: Series): string {
    const stats = series.statistics;
    const episodeCount = stats?.episodeCount ?? 0;
    const fileCount = stats?.episodeFileCount ?? 0;

    return html`
      <tr onclick="this.closest('series-index-page').handleSeriesClick('${escapeHtml(series.titleSlug)}')">
        <td>
          <svg class="monitored-icon ${series.monitored}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            ${
              series.monitored
                ? '<path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path><polyline points="22 4 12 14.01 9 11.01"></polyline>'
                : '<circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line>'
            }
          </svg>
        </td>
        <td>${escapeHtml(series.title)}</td>
        <td>${escapeHtml(series.network ?? '-')}</td>
        <td>
          <span class="status-badge ${series.status}">
            ${this.getStatusLabel(series.status)}
          </span>
        </td>
        <td class="airing-date">${this.formatAiringDate(series.nextAiring)}</td>
        <td class="airing-date">${this.formatAiringDate(series.previousAiring)}</td>
        <td>${series.year > 0 ? series.year : '-'}</td>
        <td>${stats?.seasonCount ?? 0}</td>
        <td class="episode-progress">
          <div class="episode-progress-bar">
            <div class="episode-progress-fill ${episodeCount > 0 && fileCount >= episodeCount ? 'complete' : fileCount > 0 ? 'partial' : 'empty'}" style="width: ${episodeCount > 0 ? Math.round((fileCount / episodeCount) * 100) : 0}%"></div>
          </div>
          <div class="episode-progress-text">${fileCount} / ${episodeCount}</div>
        </td>
      </tr>
    `;
  }

  private formatAiringDate(dateStr?: string): string {
    if (!dateStr) return '-';
    const date = new Date(dateStr);
    if (Number.isNaN(date.getTime())) return '-';
    const now = new Date();
    const diffMs = date.getTime() - now.getTime();
    const diffDays = Math.round(diffMs / (1000 * 60 * 60 * 24));

    // Show relative for near-future/past, absolute for far dates
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Tomorrow';
    if (diffDays === -1) return 'Yesterday';
    if (diffDays > 1 && diffDays <= 7) return `in ${diffDays} days`;
    if (diffDays < -1 && diffDays >= -7) return `${Math.abs(diffDays)} days ago`;

    return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' });
  }

  private getStatusLabel(status: string): string {
    switch (status) {
      case 'continuing':
        return 'Continuing';
      case 'ended':
        return 'Ended';
      case 'upcoming':
        return 'Upcoming';
      case 'deleted':
        return 'Deleted';
      default:
        return 'Unknown';
    }
  }

  private getSortValue(series: Series, key: SeriesSortKey): string | number {
    switch (key) {
      case 'sortTitle':
        return series.sortTitle.toLowerCase();
      case 'status':
        return series.monitored ? (series.status === 'continuing' ? 0 : 1) : 2;
      case 'network':
        return series.network?.toLowerCase() ?? '';
      case 'nextAiring':
        return series.nextAiring ?? '';
      case 'previousAiring':
        return series.previousAiring ?? '';
      case 'added':
        return series.added;
      case 'year':
        return series.year;
      case 'sizeOnDisk':
        return series.statistics?.sizeOnDisk ?? 0;
      case 'episodeProgress':
        return series.statistics?.episodeFileCount ?? 0;
      default:
        return series.sortTitle.toLowerCase();
    }
  }

  handleFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setSeriesFilter(select.value);
  }

  handleNetworkFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setSeriesNetworkFilter(select.value);
  }

  handleRootFolderFilterChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setSeriesRootFolderFilter(select.value);
  }

  handleSortChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    setSeriesSort(select.value as SeriesSortKey);
  }

  handleSortDirToggle(): void {
    const current = seriesSortDirection.value;
    seriesSortDirection.set(current === 'ascending' ? 'descending' : 'ascending');
  }

  handleHeaderSort(key: SeriesSortKey): void {
    // If clicking same column, toggle direction; otherwise set new column with ascending
    if (seriesSortKey.value === key) {
      const current = seriesSortDirection.value;
      seriesSortDirection.set(current === 'ascending' ? 'descending' : 'ascending');
    } else {
      setSeriesSort(key);
      seriesSortDirection.set('ascending');
    }
  }

  handleViewModeChange(mode: ViewMode): void {
    setSeriesViewMode(mode);
  }

  handleRetry(): void {
    this.seriesQuery.refetch();
  }

  handleSeriesClick(titleSlug: string): void {
    navigate(`/series/${titleSlug}`);
  }

  handleAddSeries(): void {
    navigate('/add/new');
  }

  async handleRefreshAll(): Promise<void> {
    const btn = this.querySelector('.refresh-all-btn') as HTMLButtonElement;
    if (btn) {
      btn.disabled = true;
      btn.classList.add('loading');
    }

    try {
      const filter = seriesFilter.value;
      const networkFilter = seriesNetworkFilter.value;
      const rootFolderFilter = seriesRootFolderFilter.value;
      const hasActiveFilter = filter !== 'all' || networkFilter !== 'all' || rootFolderFilter !== 'all';

      if (hasActiveFilter) {
        const allSeries = this.seriesQuery.data.value ?? [];
        let filtered = allSeries.filter((s) => s.seriesType !== 'anime');

        if (filter !== 'all') {
          filtered = filtered.filter((s) => {
            switch (filter) {
              case 'monitored': return s.monitored;
              case 'unmonitored': return !s.monitored;
              case 'continuing': return s.status === 'continuing';
              case 'ended': return s.status === 'ended';
              default: return true;
            }
          });
        }
        if (networkFilter !== 'all') {
          filtered = filtered.filter((s) => (s.network || 'Unknown Network') === networkFilter);
        }
        if (rootFolderFilter !== 'all') {
          filtered = filtered.filter((s) => getRootFolder(s.path) === rootFolderFilter);
        }

        const seriesIds = filtered.map((s) => s.id);
        await http.post('/command', { name: 'RefreshSeries', seriesIds });
        showInfo(`Refreshing ${seriesIds.length} series...`, 'Refresh Started');
      } else {
        await http.post('/command', { name: 'RefreshSeries' });
        showInfo('Refreshing all series metadata...', 'Refresh Started');
      }

      setTimeout(() => {
        this.seriesQuery.refetch();
        showSuccess('Series metadata updated', 'Refresh Complete');
      }, 5000);
    } catch (error) {
      console.error('[SeriesIndex] Failed to refresh all series:', error);
      showError('Failed to start refresh command', 'Refresh Failed');
    } finally {
      if (btn) {
        btn.disabled = false;
        btn.classList.remove('loading');
      }
    }
  }
}
