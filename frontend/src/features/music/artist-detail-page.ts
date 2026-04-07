/**
 * Artist Detail page - shows artist info with albums and tracks
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { type Album, type Artist, http } from '../../core/http';
import { createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showInfo, showSuccess } from '../../stores/app.store';

/** Display label for album grouping */
function albumGroupLabel(album: Album): string {
  const secondary = album.secondaryTypes ?? [];
  if (secondary.length > 0) {
    return secondary.join(' + ');
  }
  return album.albumType || 'Album';
}

/** Sort order for album type groups */
const TYPE_ORDER: Record<string, number> = {
  Album: 0,
  EP: 1,
  Single: 2,
  Live: 3,
  Compilation: 4,
  Soundtrack: 5,
  Remix: 6,
  'DJ-mix': 7,
  'Mixtape/Street': 8,
  Demo: 9,
  Broadcast: 10,
  Other: 11,
};

@customElement('artist-detail-page')
export class ArtistDetailPage extends BaseComponent {
  private artistId = signal<number | null>(null);
  private titleSlug = signal<string | null>(null);
  /** Tracks which album type groups have their hidden (unmonitored) albums expanded */
  private expandedGroups = signal<Set<string>>(new Set());

  private artistQuery: ReturnType<typeof createQuery<Artist | null>> | null = null;
  private albumsQuery: ReturnType<typeof createQuery<Album[]>> | null = null;

  static get observedAttributes(): string[] {
    return ['titleslug'];
  }

  private createQueries(id: number): void {
    this.artistQuery = createQuery({
      queryKey: ['/artist', id],
      queryFn: () => http.get<Artist>(`/artist/${id}`),
    });

    this.albumsQuery = createQuery({
      queryKey: ['/album', { artistId: id }],
      queryFn: () => http.get<Album[]>('/album', { params: { artistId: id } }),
    });

    this.watch(this.artistQuery.data, () => this.requestUpdate());
    this.watch(this.artistQuery.isLoading, () => this.requestUpdate());
    this.watch(this.albumsQuery.data, () => this.requestUpdate());
  }

  private setArtistId(id: number): void {
    this.artistId.set(id);
    this.createQueries(id);
  }

  private async lookupArtistId(slug: string): Promise<void> {
    try {
      const artistList = await http.get<Artist[]>('/artist');
      if (artistList) {
        const artist = artistList.find((a) => a.titleSlug === slug);
        if (artist) {
          this.setArtistId(artist.id);
        } else {
          showError(`Artist not found: ${slug}`);
        }
      }
    } catch {
      showError('Failed to load artist');
    }
  }

  protected onInit(): void {
    this.watch(this.artistId);
    this.watch(this.titleSlug);
  }

  protected onMount(): void {
    const slug = this.getAttribute('titleslug');
    if (slug && !this.artistId.value) {
      this.titleSlug.set(slug);
      this.lookupArtistId(slug);
    }
  }

  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null): void {
    if (name === 'titleslug' && newValue && newValue !== oldValue) {
      this.titleSlug.set(newValue);
      if (this._isConnected) {
        this.lookupArtistId(newValue);
      }
    }
  }

  protected template(): string {
    const artist = this.artistQuery?.data.value;
    const isLoading = this.artistQuery?.isLoading.value ?? true;
    const albums = this.albumsQuery?.data.value ?? [];

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
          <span>Loading artist...</span>
        </div>
        ${this.styles()}
      `;
    }

    if (!artist) {
      return html`
        <div class="error-container">
          <p>Artist not found</p>
          <button class="back-btn" onclick="this.closest('artist-detail-page').handleBack()">Back to Music</button>
        </div>
        ${this.styles()}
      `;
    }

    const posterImage = artist.images?.find((i) => i.coverType === 'poster');
    const fanartImage = artist.images?.find((i) => i.coverType === 'fanart');

    // Group albums by display type
    const albumsByType = new Map<string, Album[]>();
    for (const album of albums) {
      const label = albumGroupLabel(album);
      const group = albumsByType.get(label) ?? [];
      group.push(album);
      albumsByType.set(label, group);
    }

    // Sort groups by TYPE_ORDER (Albums first, then EPs, Singles, etc.)
    const sortedGroups = Array.from(albumsByType.entries()).sort(([a], [b]) => {
      const orderA = TYPE_ORDER[a] ?? 99;
      const orderB = TYPE_ORDER[b] ?? 99;
      return orderA - orderB;
    });

    return html`
      <div class="artist-detail">
        <!-- Header with fanart background -->
        <div class="detail-header" style="${fanartImage ? `background-image: linear-gradient(to bottom, rgba(0,0,0,0.3), var(--bg-page)), url('${fanartImage.url}')` : ''}">
          <button class="back-btn" onclick="this.closest('artist-detail-page').handleBack()">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"></polyline>
            </svg>
            Music
          </button>

          <div class="header-content">
            <div class="poster-container">
              ${
                posterImage
                  ? `<img class="detail-poster" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(artist.title)}">`
                  : `<div class="detail-poster-placeholder">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                      <circle cx="12" cy="12" r="10"></circle>
                      <path d="M9 18V5l12-2v13"></path>
                      <circle cx="6" cy="18" r="3"></circle>
                      <circle cx="18" cy="16" r="3"></circle>
                    </svg>
                  </div>`
              }
            </div>

            <div class="header-info">
              <h1 class="artist-title">${escapeHtml(artist.title)}</h1>
              <div class="meta-row">
                <span class="status-badge ${artist.status}">${artist.status}</span>
                <span class="meta-item">${artist.statistics?.albumCount ?? 0} albums</span>
                <span class="meta-item">${artist.statistics?.trackFileCount ?? 0} / ${artist.statistics?.totalTrackCount ?? 0} tracks</span>
              </div>
              ${
                artist.genres.length > 0
                  ? `
                <div class="genres">
                  ${artist.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
                </div>
              `
                  : ''
              }
              ${artist.overview ? `<p class="overview">${escapeHtml(artist.overview)}</p>` : ''}

              <div class="stats-row">
                <div class="stat">
                  <span class="stat-value">${this.formatSize(artist.statistics?.sizeOnDisk ?? 0)}</span>
                  <span class="stat-label">Size</span>
                </div>
                <div class="stat">
                  <span class="stat-value">${artist.statistics?.percentOfTracks?.toFixed(0) ?? 0}%</span>
                  <span class="stat-label">Complete</span>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Info panel -->
        <div class="info-panel">
          <div class="info-grid">
            <div class="info-item">
              <span class="info-label">Path</span>
              <span class="info-value">${escapeHtml(artist.path)}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Quality Profile</span>
              <span class="info-value">${artist.qualityProfileId}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Monitored</span>
              <span class="info-value">${artist.monitored ? 'Yes' : 'No'}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Added</span>
              <span class="info-value">${new Date(artist.added).toLocaleDateString()}</span>
            </div>
          </div>
        </div>

        <!-- Actions -->
        <div class="actions-panel">
          <button class="action-btn primary" onclick="this.closest('artist-detail-page').handleSearch()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            Search
          </button>
          <button class="action-btn" onclick="this.closest('artist-detail-page').handleRefreshMetadata()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 2v6h-6"></path>
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
              <path d="M3 22v-6h6"></path>
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
            </svg>
            Refresh
          </button>
          <button class="action-btn" onclick="this.closest('artist-detail-page').handleRescanFiles()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
              <line x1="12" y1="11" x2="12" y2="17"></line>
              <line x1="9" y1="14" x2="15" y2="14"></line>
            </svg>
            Rescan Files
          </button>
          <button class="action-btn danger" onclick="this.closest('artist-detail-page').handleDelete()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
            Delete
          </button>
        </div>

        <!-- Albums grouped by type -->
        ${sortedGroups
          .map(([groupLabel, typeAlbums]) => {
            const monitored = typeAlbums.filter((a) => a.monitored);
            const hidden = typeAlbums.filter((a) => !a.monitored);
            const isExpanded = this.expandedGroups.value.has(groupLabel);
            const visible = isExpanded ? typeAlbums : monitored;

            return html`
          <div class="albums-section">
            <div class="section-header">
              <h2 class="section-title">${escapeHtml(groupLabel)}s (${monitored.length})</h2>
              ${
                hidden.length > 0
                  ? `
                <button class="toggle-hidden-btn" onclick="this.closest('artist-detail-page').handleToggleGroup('${escapeHtml(groupLabel)}')">
                  ${isExpanded ? `Hide ${hidden.length} unmonitored` : `Show ${hidden.length} hidden`}
                </button>
              `
                  : ''
              }
            </div>
            ${
              visible.length > 0
                ? `
            <div class="albums-grid">
              ${visible.map((album) => this.renderAlbumCard(album)).join('')}
            </div>
            `
                : `
            <div class="empty-group">No monitored ${escapeHtml(groupLabel).toLowerCase()}s</div>
            `
            }
          </div>
        `;
          })
          .join('')}
      </div>

      ${this.styles()}
    `;
  }

  private renderAlbumCard(album: Album): string {
    const coverImage = album.images?.find((i) => i.coverType === 'poster');
    const trackCount = album.statistics?.trackFileCount ?? 0;
    const totalTracks = album.statistics?.totalTrackCount ?? 0;

    return html`
      <div class="album-card ${album.monitored ? '' : 'unmonitored'}" data-album-id="${album.id}"
           onclick="this.closest('artist-detail-page').handleAlbumClick('${escapeHtml(album.titleSlug || String(album.id))}')"
           style="cursor:pointer">
        <div class="album-cover">
          ${
            coverImage
              ? `<img src="${escapeHtml(coverImage.url)}" alt="${escapeHtml(album.title)}" loading="lazy" onerror="this.style.display='none';this.nextElementSibling.style.display='flex'">`
              : ''
          }
          <div class="album-cover-placeholder" ${coverImage ? 'style="display:none"' : ''}>
            <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
              <circle cx="12" cy="12" r="10"></circle>
              <circle cx="12" cy="12" r="3"></circle>
            </svg>
          </div>
          <button class="monitored-toggle ${album.monitored ? 'active' : ''}"
                  title="${album.monitored ? 'Unmonitor' : 'Monitor'} album"
                  onclick="event.stopPropagation(); this.closest('artist-detail-page').handleToggleAlbumMonitored(${album.id}, ${!album.monitored})">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="${album.monitored ? 'currentColor' : 'none'}" stroke="currentColor" stroke-width="2">
              <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"></path>
            </svg>
          </button>
        </div>
        <div class="album-info">
          <div class="album-title" title="${escapeHtml(album.title)}">${escapeHtml(album.title)}</div>
          <div class="album-meta">
            ${album.releaseDate ? new Date(album.releaseDate).getFullYear() : ''}
            ${totalTracks > 0 ? ` · ${trackCount}/${totalTracks}` : ''}
          </div>
        </div>
      </div>
    `;
  }

  private styles(): string {
    return html`
      <style>
        .artist-detail {
          display: flex;
          flex-direction: column;
          gap: 1.25rem;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from { opacity: 0; transform: translateY(12px); }
          to { opacity: 1; transform: translateY(0); }
        }

        .loading-container {
          display: flex;
          flex-direction: column;
          align-items: center;
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

        .detail-header {
          padding: 1.5rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          background-size: cover;
          background-position: center;
        }

        .back-btn {
          display: inline-flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 0.75rem;
          background: var(--bg-card);
          color: var(--text-color);
          border: 1px solid var(--border-glass);
          border-radius: 0.5rem;
          cursor: pointer;
          font-size: 0.875rem;
          margin-bottom: 1rem;
          transition: all var(--transition-normal);
        }

        .back-btn:hover {
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .header-content {
          display: flex;
          gap: 1.5rem;
        }

        .detail-poster {
          width: 180px;
          aspect-ratio: 1/1;
          object-fit: cover;
          border-radius: 0.5rem;
          box-shadow: 0 4px 20px rgba(0,0,0,0.3);
          flex-shrink: 0;
        }

        .detail-poster-placeholder {
          width: 180px;
          aspect-ratio: 1/1;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--bg-card-center);
          border-radius: 0.5rem;
          color: var(--text-color-muted);
          flex-shrink: 0;
        }

        .header-info {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .artist-title {
          font-size: 1.75rem;
          font-weight: 700;
          margin: 0;
        }

        .meta-row {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          flex-wrap: wrap;
        }

        .meta-item {
          color: var(--text-color-muted);
          font-size: 0.875rem;
        }

        .status-badge {
          display: inline-block;
          padding: 0.2rem 0.625rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
        }
        .status-badge.continuing { background: rgba(39, 174, 96, 0.15); color: var(--color-success); }
        .status-badge.ended { background: rgba(150, 150, 150, 0.15); color: var(--text-color-muted); }

        .genres {
          display: flex;
          gap: 0.375rem;
          flex-wrap: wrap;
        }

        .genre-tag {
          padding: 0.125rem 0.5rem;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .overview {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          line-height: 1.5;
          margin: 0;
        }

        .stats-row {
          display: flex;
          gap: 1.5rem;
          margin-top: 0.5rem;
        }

        .stat {
          display: flex;
          flex-direction: column;
          gap: 0.125rem;
        }

        .stat-value {
          font-size: 1.125rem;
          font-weight: 600;
        }

        .stat-label {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }

        .info-panel {
          padding: 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .info-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(250px, 1fr));
          gap: 1rem;
        }

        .info-item {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
        }

        .info-label {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
        }

        .info-value {
          font-size: 0.875rem;
          word-break: break-all;
        }

        .actions-panel {
          display: flex;
          gap: 0.75rem;
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .action-btn {
          display: flex;
          align-items: center;
          gap: 0.375rem;
          padding: 0.5rem 0.875rem;
          border: 1px solid var(--border-input);
          border-radius: 0.5rem;
          background: var(--bg-input);
          color: var(--text-color);
          cursor: pointer;
          font-size: 0.875rem;
          transition: all var(--transition-normal);
        }

        .action-btn:hover {
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .action-btn.primary {
          background-color: var(--btn-primary-bg);
          border-color: var(--btn-primary-bg);
          color: white;
        }

        .action-btn.primary:hover {
          background-color: var(--btn-primary-bg-hover);
          border-color: var(--btn-primary-bg-hover);
          color: white;
        }

        .action-btn.danger:hover {
          border-color: var(--color-danger);
          color: var(--color-danger);
        }

        .albums-section {
          padding: 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .section-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 1rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0;
        }

        .toggle-hidden-btn {
          padding: 0.25rem 0.625rem;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          border-radius: 0.375rem;
          color: var(--text-color-muted);
          font-size: 0.75rem;
          cursor: pointer;
          transition: all var(--transition-normal);
        }

        .toggle-hidden-btn:hover {
          border-color: var(--pir9-blue);
          color: var(--pir9-blue);
        }

        .empty-group {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          font-style: italic;
          padding: 1rem 0;
        }

        .albums-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
          gap: 1rem;
        }

        .album-card {
          border-radius: 0.5rem;
          overflow: hidden;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          transition: all var(--transition-normal);
        }

        .album-card:hover {
          transform: translateY(-4px);
          box-shadow: var(--shadow-card-hover);
        }

        .album-card.unmonitored {
          opacity: 0.5;
        }

        .album-card.unmonitored:hover {
          opacity: 0.85;
        }

        .album-cover {
          aspect-ratio: 1/1;
          overflow: hidden;
          position: relative;
        }

        .album-cover img {
          width: 100%;
          height: 100%;
          object-fit: cover;
        }

        .album-cover-placeholder {
          width: 100%;
          height: 100%;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--bg-card);
          color: var(--text-color-muted);
        }

        .monitored-toggle {
          position: absolute;
          top: 0.375rem;
          right: 0.375rem;
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          background: rgba(0,0,0,0.6);
          border: none;
          border-radius: 50%;
          color: var(--text-color-muted);
          cursor: pointer;
          opacity: 0;
          transition: all var(--transition-normal);
        }

        .album-card:hover .monitored-toggle {
          opacity: 1;
        }

        .monitored-toggle.active {
          color: var(--color-warning, #f1c40f);
          opacity: 0.8;
        }

        .monitored-toggle:hover {
          transform: scale(1.15);
          opacity: 1 !important;
        }

        .album-info {
          padding: 0.75rem;
        }

        .album-title {
          font-size: 0.875rem;
          font-weight: 600;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .album-meta {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .album-tracks {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin-top: 0.25rem;
        }

        .error-container {
          display: flex;
          flex-direction: column;
          align-items: center;
          gap: 1rem;
          padding: 6rem 2rem;
          text-align: center;
        }

        @media (max-width: 640px) {
          .header-content {
            flex-direction: column;
            align-items: center;
            text-align: center;
          }

          .meta-row, .genres, .stats-row {
            justify-content: center;
          }
        }
      </style>
    `;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
  }

  // Event handlers
  handleBack(): void {
    navigate('/music');
  }

  async handleSearch(): Promise<void> {
    const id = this.artistId.value;
    if (!id) return;

    try {
      await http.post('/command', { name: 'ArtistSearch', artistId: id });
      showSuccess('Artist search started');
    } catch {
      showError('Failed to start artist search');
    }
  }

  async handleRefreshMetadata(): Promise<void> {
    const id = this.artistId.value;
    if (!id) return;

    try {
      await http.post(`/artist/${id}/refresh`, {});
      showSuccess('Artist metadata refreshed');
      invalidateQueries(['/artist', id]);
      invalidateQueries(['/artist']);
      invalidateQueries(['/album', { artistId: id }]);
      this.artistQuery?.refetch();
      this.albumsQuery?.refetch();
    } catch {
      showError('Failed to refresh metadata');
    }
  }

  async handleRescanFiles(): Promise<void> {
    const id = this.artistId.value;
    if (!id) return;

    try {
      await http.post(`/artist/${id}/rescan`, {});
      showInfo('Scanning for artist files...');
    } catch {
      showError('Failed to scan files');
    }
  }

  handleAlbumClick(albumSlug: string): void {
    const artist = this.artistQuery?.data.value;
    const slug = artist?.titleSlug ?? this.titleSlug.value;
    if (slug) {
      navigate(`/music/${slug}/album/${albumSlug}`);
    }
  }

  handleToggleGroup(groupLabel: string): void {
    const current = new Set(this.expandedGroups.value);
    if (current.has(groupLabel)) {
      current.delete(groupLabel);
    } else {
      current.add(groupLabel);
    }
    this.expandedGroups.set(current);
    this.requestUpdate();
  }

  async handleToggleAlbumMonitored(albumId: number, monitored: boolean): Promise<void> {
    try {
      await http.put(`/album/${albumId}`, { monitored });
      const artistId = this.artistId.value;
      if (artistId) {
        invalidateQueries(['/album', { artistId }]);
        this.albumsQuery?.refetch();
      }
    } catch {
      showError('Failed to update album');
    }
  }

  async handleDelete(): Promise<void> {
    const artist = this.artistQuery?.data.value;
    if (!artist) return;

    if (!confirm(`Are you sure you want to delete "${artist.title}"?`)) return;

    try {
      await http.delete(`/artist/${artist.id}`, { params: { deleteFiles: false } });
      showSuccess(`Deleted "${artist.title}"`);
      invalidateQueries(['/artist']);
      navigate('/music');
    } catch {
      showError('Failed to delete artist');
    }
  }
}
