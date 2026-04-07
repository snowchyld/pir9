/**
 * Album Detail page - shows album info with tracks, releases, and download
 */

import type { ReleaseSearchModal } from '../../components/release-search-modal';
import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { type Album, type Artist, http } from '../../core/http';
import { createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';
import '../../components/release-search-modal';

interface TrackFileInfo {
  id: number;
  path: string;
  relativePath: string;
  size: number;
  audioFormat?: string;
  bitrate?: number;
  sampleRate?: number;
  channels?: number;
  dateAdded: string;
}

interface Track {
  id: number;
  albumId: number;
  artistId: number;
  title: string;
  trackNumber: number;
  discNumber: number;
  durationMs?: number;
  hasFile: boolean;
  monitored: boolean;
  trackFile?: TrackFileInfo;
}

interface RenameChange {
  trackId: number;
  fileId: number;
  existingFilename: string;
  newFilename: string;
}

interface MbRelease {
  mbid: string;
  title: string;
  date?: string;
  country?: string;
  status?: string;
  barcode?: string;
  packaging?: string;
  trackCount?: number;
}

@customElement('album-detail-page')
export class AlbumDetailPage extends BaseComponent {
  private albumId = signal<number | null>(null);
  private artistSlug = signal<string | null>(null);
  private showReleases = signal(false);
  private expandedTrackId = signal<number | null>(null);
  private lyricsCache = new Map<number, string | null>();
  private lyricsLoading = signal(false);
  /** Rename modal state */
  private renameChanges = signal<RenameChange[]>([]);
  private renameModalOpen = signal(false);

  private albumQuery: ReturnType<typeof createQuery<Album | null>> | null = null;
  private artistQuery: ReturnType<typeof createQuery<Artist | null>> | null = null;
  private tracksQuery: ReturnType<typeof createQuery<Track[]>> | null = null;
  private releasesQuery: ReturnType<typeof createQuery<MbRelease[]>> | null = null;

  static get observedAttributes(): string[] {
    return ['albumid', 'titleslug'];
  }

  private createQueries(id: number): void {
    this.albumQuery = createQuery({
      queryKey: ['/album', id],
      queryFn: () => http.get<Album>(`/album/${id}`),
    });

    this.tracksQuery = createQuery({
      queryKey: ['/track', { albumId: id }],
      queryFn: () => http.get<Track[]>('/track', { params: { albumId: id } }),
    });

    this.watch(this.albumQuery.data, () => {
      this.requestUpdate();
      // Load releases when album data arrives (need musicbrainzId)
      const album = this.albumQuery?.data.value;
      if (album?.musicbrainzId && !this.releasesQuery) {
        this.releasesQuery = createQuery({
          queryKey: ['/musicbrainz/albums', album.musicbrainzId, 'releases'],
          queryFn: () =>
            http.get<MbRelease[]>(`/musicbrainz/albums/${album.musicbrainzId}/releases`),
        });
        this.watch(this.releasesQuery.data, () => this.requestUpdate());
      }
    });
    this.watch(this.albumQuery.isLoading, () => this.requestUpdate());
    this.watch(this.tracksQuery.data, () => this.requestUpdate());
  }

  protected onInit(): void {
    this.watch(this.albumId);
    this.watch(this.artistSlug);
    this.watch(this.showReleases);
    this.watch(this.expandedTrackId);
    this.watch(this.lyricsLoading);
    this.watch(this.renameModalOpen);
    this.watch(this.renameChanges);
  }

  protected onMount(): void {
    const idAttr = this.getAttribute('albumid');
    const slug = this.getAttribute('titleslug');
    if (slug) this.artistSlug.set(slug);
    if (idAttr) {
      const id = Number.parseInt(idAttr, 10);
      if (!Number.isNaN(id)) {
        this.albumId.set(id);
        this.createQueries(id);
      }
    }

    // Also load artist data for back-navigation context
    if (slug) {
      this.artistQuery = createQuery({
        queryKey: ['/artist/lookup', slug],
        queryFn: async () => {
          const list = await http.get<Artist[]>('/artist');
          return list?.find((a) => a.titleSlug === slug) ?? null;
        },
      });
      this.watch(this.artistQuery.data, () => this.requestUpdate());
    }
  }

  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null): void {
    if (newValue && newValue !== oldValue && this._isConnected) {
      if (name === 'albumid') {
        const id = Number.parseInt(newValue, 10);
        if (!Number.isNaN(id)) {
          this.albumId.set(id);
          this.createQueries(id);
        }
      }
      if (name === 'titleslug') {
        this.artistSlug.set(newValue);
      }
    }
  }

  protected template(): string {
    const album = this.albumQuery?.data.value;
    const isLoading = this.albumQuery?.isLoading.value ?? true;
    const tracks = this.tracksQuery?.data.value ?? [];
    const releases = this.releasesQuery?.data.value ?? [];
    const artist = this.artistQuery?.data.value;
    const artistName = artist?.title ?? this.artistSlug.value ?? 'Artist';

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
          <span>Loading album...</span>
        </div>
        ${this.styles()}
      `;
    }

    if (!album) {
      return html`
        <div class="error-container">
          <p>Album not found</p>
          <button class="back-btn" onclick="this.closest('album-detail-page').handleBack()">Back</button>
        </div>
        ${this.styles()}
      `;
    }

    const coverImage = album.images?.find((i) => i.coverType === 'poster');
    const grouped = this.groupTracksByDisc(tracks);

    return html`
      <div class="album-detail">
        <!-- Header -->
        <div class="detail-header">
          <button class="back-btn" onclick="this.closest('album-detail-page').handleBack()">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"></polyline>
            </svg>
            ${escapeHtml(artistName)}
          </button>

          <div class="header-content">
            <div class="cover-container">
              ${
                coverImage
                  ? `<img class="album-cover" src="${escapeHtml(coverImage.url)}" alt="${escapeHtml(album.title)}" onerror="this.style.display='none';this.nextElementSibling.style.display='flex'">`
                  : ''
              }
              <div class="cover-placeholder" ${coverImage ? 'style="display:none"' : ''}>
                <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                  <circle cx="12" cy="12" r="10"></circle>
                  <circle cx="12" cy="12" r="3"></circle>
                </svg>
              </div>
            </div>

            <div class="header-info">
              <h1 class="album-title">${escapeHtml(album.title)}</h1>
              <div class="meta-row">
                <span class="type-badge">${escapeHtml(album.albumType)}</span>
                ${album.releaseDate ? `<span class="meta-item">${new Date(album.releaseDate).getFullYear()}</span>` : ''}
                <span class="meta-item">${tracks.length} tracks</span>
                ${album.monitored ? '<span class="monitored-badge">Monitored</span>' : '<span class="unmonitored-badge">Unmonitored</span>'}
              </div>
              ${
                album.genres?.length > 0
                  ? `<div class="genres">${album.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}</div>`
                  : ''
              }
              ${
                album.rating
                  ? `<div class="rating-row">
                    <span class="rating-stars">${'★'.repeat(Math.round(album.rating / 20))}${'☆'.repeat(5 - Math.round(album.rating / 20))}</span>
                    <span class="rating-value">${(album.rating / 20).toFixed(1)}/5</span>
                    ${album.ratingCount ? `<span class="rating-count">(${album.ratingCount} votes)</span>` : ''}
                  </div>`
                  : ''
              }
              ${album.overview ? `<p class="overview">${escapeHtml(album.overview)}</p>` : ''}
              ${
                album.tags && album.tags.length > 0
                  ? `<div class="tags-row">${album.tags
                      .slice(0, 8)
                      .map((t) => `<span class="tag">${escapeHtml(t)}</span>`)
                      .join('')}</div>`
                  : ''
              }
            </div>
          </div>
        </div>

        <!-- Actions -->
        <div class="actions-panel">
          <button class="action-btn primary" onclick="this.closest('album-detail-page').handleSearch()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            Search &amp; Download
          </button>
          <button class="action-btn ${album.monitored ? '' : 'primary'}"
                  onclick="this.closest('album-detail-page').handleToggleMonitored()">
            ${album.monitored ? 'Unmonitor' : 'Monitor'}
          </button>
          <button class="action-btn" onclick="this.closest('album-detail-page').handleRescan()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21.5 2v6h-6"></path>
              <path d="M21.34 15.57a10 10 0 1 1-.57-8.38"></path>
            </svg>
            Rescan Files
          </button>
          <button class="action-btn" onclick="this.closest('album-detail-page').handleRename()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"></path>
            </svg>
            Rename Files
          </button>
        </div>

        <!-- Tracks -->
        <div class="tracks-section">
          <h2 class="section-title">Tracks</h2>
          ${
            tracks.length > 0
              ? Array.from(grouped.entries())
                  .map(
                    ([disc, discTracks]) => `
              ${grouped.size > 1 ? `<div class="disc-header">Disc ${disc}</div>` : ''}
              <div class="tracks-list">
                ${discTracks
                  .map(
                    (t) => `
                  <div class="track-row ${t.hasFile ? 'has-file' : ''} ${this.expandedTrackId.value === t.id ? 'expanded' : ''}"
                       onclick="this.closest('album-detail-page').handleTrackClick(${t.id})">
                    <span class="track-num">${t.trackNumber}</span>
                    <span class="track-title">${escapeHtml(t.title)}</span>
                    <span class="track-meta">${t.trackFile ? `${t.trackFile.audioFormat ?? ''} ${t.trackFile.bitrate ? `${t.trackFile.bitrate}k` : ''}` : ''}</span>
                    <span class="track-size">${t.trackFile ? this.formatSize(t.trackFile.size) : ''}</span>
                    <span class="track-duration">${this.formatDuration(t.durationMs)}</span>
                    ${t.hasFile ? '<span class="track-status">&#10003;</span>' : '<span class="track-status missing">&#8226;</span>'}
                  </div>
                  ${this.expandedTrackId.value === t.id ? this.renderTrackDetail(t) : ''}
                `,
                  )
                  .join('')}
              </div>
            `,
                  )
                  .join('')
              : '<div class="empty-message">No tracks found. Try refreshing the artist metadata.</div>'
          }
        </div>

        <!-- Releases / Editions -->
        ${
          releases.length > 0
            ? `
          <div class="releases-section">
            <div class="section-header">
              <h2 class="section-title">Editions (${releases.length})</h2>
              <button class="toggle-hidden-btn" onclick="this.closest('album-detail-page').handleToggleReleases()">
                ${this.showReleases.value ? 'Hide' : 'Show'}
              </button>
            </div>
            ${
              this.showReleases.value
                ? `
              <div class="releases-table">
                <div class="release-header-row">
                  <span>Title</span>
                  <span>Date</span>
                  <span>Country</span>
                  <span>Tracks</span>
                  <span>Status</span>
                </div>
                ${releases
                  .map(
                    (r) => `
                  <div class="release-row">
                    <span class="release-title">${escapeHtml(r.title)}</span>
                    <span>${r.date ?? '—'}</span>
                    <span>${r.country ?? '—'}</span>
                    <span>${r.trackCount ?? '—'}</span>
                    <span class="release-status">${r.status ?? '—'}</span>
                  </div>
                `,
                  )
                  .join('')}
              </div>
            `
                : ''
            }
          </div>
        `
            : ''
        }
      </div>

      ${this.renameModalOpen.value ? this.renderRenameModal() : ''}
      <release-search-modal></release-search-modal>

      ${this.styles()}
    `;
  }

  private renderRenameModal(): string {
    const changes = this.renameChanges.value;
    return html`
      <div class="modal-overlay" onclick="this.closest('album-detail-page').handleCancelRename()">
        <div class="modal-content" onclick="event.stopPropagation()">
          <div class="modal-header">
            <h3>Rename ${changes.length} Files</h3>
            <button class="modal-close" onclick="this.closest('album-detail-page').handleCancelRename()">&times;</button>
          </div>
          <div class="modal-body">
            <div class="rename-list">
              ${changes
                .map(
                  (c) => `
                <div class="rename-item">
                  <div class="rename-old">${escapeHtml(c.existingFilename)}</div>
                  <div class="rename-arrow">&rarr;</div>
                  <div class="rename-new">${escapeHtml(c.newFilename)}</div>
                </div>
              `,
                )
                .join('')}
            </div>
          </div>
          <div class="modal-footer">
            <button class="action-btn" onclick="this.closest('album-detail-page').handleCancelRename()">Cancel</button>
            <button class="action-btn primary" onclick="this.closest('album-detail-page').handleConfirmRename()">Rename ${changes.length} Files</button>
          </div>
        </div>
      </div>
    `;
  }

  private groupTracksByDisc(tracks: Track[]): Map<number, Track[]> {
    const map = new Map<number, Track[]>();
    for (const t of tracks) {
      const disc = t.discNumber ?? 1;
      const group = map.get(disc) ?? [];
      group.push(t);
      map.set(disc, group);
    }
    return map;
  }

  private formatDuration(ms?: number): string {
    if (!ms) return '';
    const s = Math.floor(ms / 1000);
    const m = Math.floor(s / 60);
    const sec = s % 60;
    return `${m}:${sec.toString().padStart(2, '0')}`;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '-';
    const units = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / 1024 ** i).toFixed(1)} ${units[i]}`;
  }

  private renderTrackDetail(track: Track): string {
    const f = track.trackFile;

    // File metadata section
    const metaHtml = f
      ? `<div class="track-detail-meta">
          <div class="meta-grid">
            ${f.audioFormat ? `<div class="meta-pair"><span class="meta-key">Format</span><span class="meta-val">${f.audioFormat}</span></div>` : ''}
            ${f.bitrate ? `<div class="meta-pair"><span class="meta-key">Bitrate</span><span class="meta-val">${f.bitrate} kbps</span></div>` : ''}
            ${f.sampleRate ? `<div class="meta-pair"><span class="meta-key">Sample Rate</span><span class="meta-val">${(f.sampleRate / 1000).toFixed(1)} kHz</span></div>` : ''}
            ${f.channels ? `<div class="meta-pair"><span class="meta-key">Channels</span><span class="meta-val">${f.channels === 2 ? 'Stereo' : f.channels === 1 ? 'Mono' : f.channels.toString()}</span></div>` : ''}
            <div class="meta-pair"><span class="meta-key">Size</span><span class="meta-val">${this.formatSize(f.size)}</span></div>
            <div class="meta-pair"><span class="meta-key">Path</span><span class="meta-val file-path">${escapeHtml(f.relativePath)}</span></div>
          </div>
        </div>`
      : '<div class="track-detail-meta"><em>No file linked</em></div>';

    // Lyrics section
    let lyricsHtml = '';
    if (this.lyricsLoading.value) {
      lyricsHtml = '<div class="lyrics-section"><em>Loading lyrics...</em></div>';
    } else {
      const lyrics = this.lyricsCache.get(track.id);
      if (lyrics === undefined) {
        lyricsHtml = '<div class="lyrics-section"><em>Loading lyrics...</em></div>';
      } else if (lyrics === null) {
        lyricsHtml =
          '<div class="lyrics-section"><em class="muted">Lyrics not available</em></div>';
      } else {
        lyricsHtml = `<div class="lyrics-section">
          <div class="lyrics-header">Lyrics</div>
          <pre class="lyrics-text">${escapeHtml(lyrics)}</pre>
        </div>`;
      }
    }

    return `<div class="track-detail-panel">${metaHtml}${lyricsHtml}</div>`;
  }

  async handleTrackClick(trackId: number): Promise<void> {
    if (this.expandedTrackId.value === trackId) {
      this.expandedTrackId.set(null);
      this.requestUpdate();
      return;
    }

    this.expandedTrackId.set(trackId);
    this.requestUpdate();

    // Fetch lyrics if not cached
    if (!this.lyricsCache.has(trackId)) {
      this.lyricsLoading.set(true);
      this.requestUpdate();
      try {
        const artist = this.artistQuery?.data.value;
        const params: Record<string, string> = {};
        if (artist?.title) {
          params.artist = artist.title;
        }
        const result = await http.get<{ lyrics: string | null }>(`/track/${trackId}/lyrics`, {
          params,
        });
        this.lyricsCache.set(trackId, result?.lyrics ?? null);
      } catch {
        this.lyricsCache.set(trackId, null);
      }
      this.lyricsLoading.set(false);
      this.requestUpdate();
    }
  }

  // Event handlers
  handleBack(): void {
    const slug = this.artistSlug.value;
    navigate(slug ? `/music/${slug}` : '/music');
  }

  handleToggleReleases(): void {
    this.showReleases.set(!this.showReleases.value);
    this.requestUpdate();
  }

  async handleToggleMonitored(): Promise<void> {
    const album = this.albumQuery?.data.value;
    if (!album) return;

    try {
      await http.put(`/album/${album.id}`, { monitored: !album.monitored });
      invalidateQueries(['/album', album.id]);
      this.albumQuery?.refetch();
      showSuccess(album.monitored ? 'Album unmonitored' : 'Album monitored');
    } catch {
      showError('Failed to update album');
    }
  }

  async handleRescan(): Promise<void> {
    const album = this.albumQuery?.data.value;
    if (!album) return;

    try {
      await http.post(`/album/${album.id}/rescan`, {});
      this.albumQuery?.refetch();
      this.tracksQuery?.refetch();
      showSuccess('Album rescan complete');
    } catch {
      showError('Failed to rescan album');
    }
  }

  async handleRename(): Promise<void> {
    const album = this.albumQuery?.data.value;
    if (!album) return;

    try {
      // Get preview first
      const preview = await http.post<{ changes: RenameChange[]; totalChanges: number }>(
        `/album/${album.id}/rename?preview=true`,
        {},
      );
      if (!preview?.changes?.length) {
        showSuccess('All files already correctly named');
        return;
      }
      this.renameChanges.set(preview.changes);
      this.renameModalOpen.set(true);
      this.requestUpdate();
    } catch {
      showError('Failed to generate rename preview');
    }
  }

  async handleConfirmRename(): Promise<void> {
    const album = this.albumQuery?.data.value;
    if (!album) return;

    try {
      const result = await http.post<{ renamed: number }>(`/album/${album.id}/rename`, {});
      showSuccess(`${result?.renamed ?? 0} files renamed`);
      this.renameModalOpen.set(false);
      this.tracksQuery?.refetch();
    } catch {
      showError('Failed to rename files');
    }
  }

  handleCancelRename(): void {
    this.renameModalOpen.set(false);
    this.requestUpdate();
  }

  handleSearch(): void {
    const album = this.albumQuery?.data.value;
    const artist = this.artistQuery?.data.value;
    if (!album || !artist) return;

    const modal = this.querySelector('release-search-modal') as ReleaseSearchModal | null;
    if (modal) {
      const searchQuery = `${artist.title} ${album.title}`;
      modal.open({
        query: searchQuery,
        queryTitle: `${artist.title} - ${album.title}`,
      });
    }
  }

  private styles(): string {
    return html`
      <style>
        .album-detail {
          display: flex;
          flex-direction: column;
          gap: 1.25rem;
          animation: pageEnter var(--transition-page) var(--ease-out-expo);
        }

        @keyframes pageEnter {
          from { opacity: 0; transform: translateY(12px); }
          to { opacity: 1; transform: translateY(0); }
        }

        .loading-container, .error-container {
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

        .cover-container {
          flex-shrink: 0;
        }

        .album-cover {
          width: 200px;
          aspect-ratio: 1/1;
          object-fit: cover;
          border-radius: 0.5rem;
          box-shadow: 0 4px 20px rgba(0,0,0,0.3);
        }

        .cover-placeholder {
          width: 200px;
          aspect-ratio: 1/1;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--bg-card-center);
          border-radius: 0.5rem;
          color: var(--text-color-muted);
        }

        .header-info {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 0.625rem;
        }

        .album-title {
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

        .meta-item { color: var(--text-color-muted); font-size: 0.875rem; }

        .type-badge {
          padding: 0.2rem 0.625rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
          background: rgba(52, 152, 219, 0.15);
          color: var(--pir9-blue);
        }

        .monitored-badge {
          padding: 0.2rem 0.625rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
          background: rgba(39, 174, 96, 0.15);
          color: var(--color-success);
        }

        .unmonitored-badge {
          padding: 0.2rem 0.625rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
          background: rgba(150, 150, 150, 0.15);
          color: var(--text-color-muted);
        }

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

        /* Tracks */
        .tracks-section, .releases-section {
          padding: 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0;
        }

        .section-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 1rem;
        }

        .disc-header {
          font-size: 0.875rem;
          font-weight: 600;
          color: var(--text-color-muted);
          padding: 0.5rem 0;
          margin-top: 0.5rem;
          border-top: 1px solid var(--border-glass);
        }

        .tracks-list {
          display: flex;
          flex-direction: column;
        }

        .track-row {
          display: grid;
          grid-template-columns: 2.5rem 1fr 5rem 4rem 3.5rem 1.5rem;
          gap: 0.5rem;
          align-items: center;
          padding: 0.5rem 0.5rem;
          border-radius: 0.375rem;
          transition: background var(--transition-normal);
        }

        .track-row {
          cursor: pointer;
        }

        .track-row:hover {
          background: var(--bg-card-center);
        }

        .track-row.expanded {
          background: var(--bg-card-center);
        }

        .track-row.has-file {
          color: var(--text-color);
        }

        .track-row:not(.has-file) {
          color: var(--text-color-muted);
        }

        .track-num {
          font-size: 0.875rem;
          text-align: right;
          color: var(--text-color-muted);
          font-variant-numeric: tabular-nums;
        }

        .track-title {
          font-size: 0.875rem;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .track-duration {
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          font-variant-numeric: tabular-nums;
        }

        .track-status {
          font-size: 0.875rem;
          color: var(--color-success);
        }

        .track-status.missing {
          color: var(--text-color-muted);
        }

        .lyrics-panel {
          padding: 0.75rem 1rem 0.75rem 3.25rem;
          border-bottom: 1px solid var(--border-glass);
          background: var(--bg-card-center);
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          animation: fadeIn 0.2s ease;
        }

        @keyframes fadeIn {
          from { opacity: 0; max-height: 0; }
          to { opacity: 1; max-height: 500px; }
        }

        .lyrics-text {
          white-space: pre-wrap;
          font-family: inherit;
          font-size: 0.8125rem;
          line-height: 1.6;
          margin: 0;
          max-height: 400px;
          overflow-y: auto;
        }

        .track-meta, .track-size {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-align: right;
          font-variant-numeric: tabular-nums;
        }

        /* Track detail panel (metadata + lyrics) */
        .track-detail-panel {
          padding: 0.75rem 1rem 0.75rem 3.25rem;
          border-bottom: 1px solid var(--border-glass);
          background: var(--bg-card-center);
          animation: fadeIn 0.2s ease;
          display: flex;
          gap: 1.5rem;
        }

        .track-detail-meta {
          flex: 0 0 auto;
          min-width: 220px;
        }

        .meta-grid {
          display: grid;
          grid-template-columns: auto 1fr;
          gap: 0.25rem 0.75rem;
          font-size: 0.8125rem;
        }

        .meta-pair {
          display: contents;
        }

        .meta-key {
          color: var(--text-color-muted);
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.03em;
        }

        .meta-val {
          color: var(--text-color);
        }

        .file-path {
          font-family: monospace;
          font-size: 0.75rem;
          word-break: break-all;
        }

        .lyrics-section {
          flex: 1;
          min-width: 0;
        }

        .lyrics-header {
          font-size: 0.75rem;
          font-weight: 600;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
          margin-bottom: 0.5rem;
        }

        .muted { color: var(--text-color-muted); }

        /* Rating & tags */
        .rating-row {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          font-size: 0.875rem;
        }

        .rating-stars { color: var(--color-warning, #f1c40f); }
        .rating-value { font-weight: 600; }
        .rating-count { color: var(--text-color-muted); font-size: 0.75rem; }

        .overview {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          line-height: 1.5;
          margin: 0;
          max-height: 4.5em;
          overflow: hidden;
        }

        .tags-row {
          display: flex;
          gap: 0.25rem;
          flex-wrap: wrap;
        }

        .tag {
          padding: 0.1rem 0.4rem;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          border-radius: 0.25rem;
          font-size: 0.6875rem;
          color: var(--text-color-muted);
        }

        /* Rename modal */
        .modal-overlay {
          position: fixed;
          top: 0;
          left: 0;
          right: 0;
          bottom: 0;
          background: rgba(0, 0, 0, 0.6);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1000;
          animation: fadeIn 0.15s ease;
        }

        .modal-content {
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          width: min(640px, 90vw);
          max-height: 80vh;
          display: flex;
          flex-direction: column;
          box-shadow: 0 20px 60px rgba(0,0,0,0.4);
        }

        .modal-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 1rem 1.25rem;
          border-bottom: 1px solid var(--border-glass);
        }

        .modal-header h3 { margin: 0; font-size: 1rem; }

        .modal-close {
          background: none;
          border: none;
          color: var(--text-color-muted);
          font-size: 1.5rem;
          cursor: pointer;
          padding: 0;
          line-height: 1;
        }

        .modal-body {
          padding: 1rem 1.25rem;
          overflow-y: auto;
          flex: 1;
        }

        .modal-footer {
          display: flex;
          justify-content: flex-end;
          gap: 0.75rem;
          padding: 1rem 1.25rem;
          border-top: 1px solid var(--border-glass);
        }

        .rename-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .rename-item {
          display: grid;
          grid-template-columns: 1fr auto 1fr;
          gap: 0.75rem;
          align-items: center;
          padding: 0.5rem;
          border-radius: 0.375rem;
          background: var(--bg-card-center);
          font-size: 0.8125rem;
        }

        .rename-old {
          color: var(--text-color-muted);
          word-break: break-all;
        }

        .rename-arrow {
          color: var(--pir9-blue);
          font-size: 1rem;
        }

        .rename-new {
          color: var(--color-success);
          font-weight: 500;
          word-break: break-all;
        }

        .empty-message {
          color: var(--text-color-muted);
          font-size: 0.875rem;
          font-style: italic;
          padding: 1rem 0;
        }

        /* Releases table */
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

        .releases-table {
          display: flex;
          flex-direction: column;
          margin-top: 0.75rem;
        }

        .release-header-row {
          display: grid;
          grid-template-columns: 1fr 6rem 4rem 4rem 5rem;
          gap: 0.75rem;
          padding: 0.5rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 600;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
          border-bottom: 1px solid var(--border-glass);
        }

        .release-row {
          display: grid;
          grid-template-columns: 1fr 6rem 4rem 4rem 5rem;
          gap: 0.75rem;
          padding: 0.5rem 0.5rem;
          font-size: 0.875rem;
          border-bottom: 1px solid var(--border-glass);
        }

        .release-row:last-child {
          border-bottom: none;
        }

        .release-title {
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .release-status {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        @media (max-width: 640px) {
          .header-content {
            flex-direction: column;
            align-items: center;
            text-align: center;
          }
          .meta-row, .genres {
            justify-content: center;
          }
          .track-row {
            grid-template-columns: 2rem 1fr 3rem 1.5rem;
          }
          .track-meta, .track-size {
            display: none;
          }
          .track-detail-panel {
            flex-direction: column;
            padding-left: 1rem;
          }
          .release-header-row, .release-row {
            grid-template-columns: 1fr 5rem 3rem;
          }
          .release-header-row span:nth-child(4),
          .release-header-row span:nth-child(5),
          .release-row span:nth-child(4),
          .release-row span:nth-child(5) {
            display: none;
          }
        }
      </style>
    `;
  }
}
