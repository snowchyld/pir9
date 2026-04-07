/**
 * Podcast Detail page - shows podcast info with episode list
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http, type Podcast, type PodcastEpisode } from '../../core/http';
import { createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showInfo, showSuccess } from '../../stores/app.store';

@customElement('podcast-detail-page')
export class PodcastDetailPage extends BaseComponent {
  private podcastId = signal<number | null>(null);
  private titleSlug = signal<string | null>(null);

  private podcastQuery: ReturnType<typeof createQuery<Podcast | null>> | null = null;
  private episodesQuery: ReturnType<typeof createQuery<PodcastEpisode[]>> | null = null;

  static get observedAttributes(): string[] {
    return ['titleslug'];
  }

  private createQueries(id: number): void {
    this.podcastQuery = createQuery({
      queryKey: ['/podcast', id],
      queryFn: () => http.get<Podcast>(`/podcast/${id}`),
    });

    this.episodesQuery = createQuery({
      queryKey: ['/podcast', id, 'episodes'],
      queryFn: () => http.get<PodcastEpisode[]>(`/podcast/${id}/episodes`),
    });

    this.watch(this.podcastQuery.data, () => this.requestUpdate());
    this.watch(this.podcastQuery.isLoading, () => this.requestUpdate());
    this.watch(this.episodesQuery.data, () => this.requestUpdate());
  }

  private setPodcastId(id: number): void {
    this.podcastId.set(id);
    this.createQueries(id);
  }

  private async lookupPodcastId(slug: string): Promise<void> {
    try {
      const podcastList = await http.get<Podcast[]>('/podcast');
      if (podcastList) {
        const podcast = podcastList.find((p) => p.titleSlug === slug);
        if (podcast) {
          this.setPodcastId(podcast.id);
        } else {
          showError(`Podcast not found: ${slug}`);
        }
      }
    } catch {
      showError('Failed to load podcast');
    }
  }

  protected onInit(): void {
    this.watch(this.podcastId);
    this.watch(this.titleSlug);
  }

  protected onMount(): void {
    const slug = this.getAttribute('titleslug');
    if (slug && !this.podcastId.value) {
      this.titleSlug.set(slug);
      this.lookupPodcastId(slug);
    }
  }

  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null): void {
    if (name === 'titleslug' && newValue && newValue !== oldValue) {
      this.titleSlug.set(newValue);
      if (this._isConnected) {
        this.lookupPodcastId(newValue);
      }
    }
  }

  protected template(): string {
    const podcast = this.podcastQuery?.data.value;
    const isLoading = this.podcastQuery?.isLoading.value ?? true;
    const episodes = this.episodesQuery?.data.value ?? [];

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
          <span>Loading podcast...</span>
        </div>
        ${this.styles()}
      `;
    }

    if (!podcast) {
      return html`
        <div class="error-container">
          <p>Podcast not found</p>
          <button class="back-btn" onclick="this.closest('podcast-detail-page').handleBack()">Back to Podcasts</button>
        </div>
        ${this.styles()}
      `;
    }

    const posterImage = podcast.images?.find((i) => i.coverType === 'poster');

    // Sort episodes by air date descending (newest first)
    const sortedEpisodes = [...episodes].sort((a, b) => {
      const dateA = a.airDate ?? '';
      const dateB = b.airDate ?? '';
      return dateB.localeCompare(dateA);
    });

    return html`
      <div class="podcast-detail">
        <!-- Header -->
        <div class="detail-header">
          <button class="back-btn" onclick="this.closest('podcast-detail-page').handleBack()">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"></polyline>
            </svg>
            Podcasts
          </button>

          <div class="header-content">
            <div class="poster-container">
              ${
                posterImage
                  ? `<img class="detail-poster" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(podcast.title)}">`
                  : `<div class="detail-poster-placeholder">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                      <path d="M3 18v-6a9 9 0 0 1 18 0v6"></path>
                      <path d="M21 19a2 2 0 0 1-2 2h-1a2 2 0 0 1-2-2v-3a2 2 0 0 1 2-2h3zM3 19a2 2 0 0 0 2 2h1a2 2 0 0 0 2-2v-3a2 2 0 0 0-2-2H3z"></path>
                    </svg>
                  </div>`
              }
            </div>

            <div class="header-info">
              <h1 class="podcast-title">${escapeHtml(podcast.title)}</h1>
              <div class="meta-row">
                <span class="status-badge ${podcast.status}">${podcast.status}</span>
                ${podcast.author ? `<span class="meta-item">by ${escapeHtml(podcast.author)}</span>` : ''}
                <span class="meta-item">${podcast.statistics?.episodeCount ?? 0} episodes</span>
              </div>
              ${
                podcast.genres.length > 0
                  ? `
                <div class="genres">
                  ${podcast.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
                </div>
              `
                  : ''
              }
              ${podcast.overview ? `<p class="overview">${escapeHtml(podcast.overview)}</p>` : ''}

              <div class="stats-row">
                <div class="stat">
                  <span class="stat-value">${this.formatSize(podcast.statistics?.sizeOnDisk ?? 0)}</span>
                  <span class="stat-label">Size</span>
                </div>
                <div class="stat">
                  <span class="stat-value">${podcast.statistics?.episodeFileCount ?? 0} / ${podcast.statistics?.totalEpisodeCount ?? 0}</span>
                  <span class="stat-label">Downloaded</span>
                </div>
                <div class="stat">
                  <span class="stat-value">${podcast.statistics?.percentOfEpisodes?.toFixed(0) ?? 0}%</span>
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
              <span class="info-value">${escapeHtml(podcast.path)}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Quality Profile</span>
              <span class="info-value">${podcast.qualityProfileId}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Monitored</span>
              <span class="info-value">${podcast.monitored ? 'Yes' : 'No'}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Added</span>
              <span class="info-value">${new Date(podcast.added).toLocaleDateString()}</span>
            </div>
          </div>
        </div>

        <!-- Actions -->
        <div class="actions-panel">
          <button class="action-btn primary" onclick="this.closest('podcast-detail-page').handleRefreshFeed()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 2v6h-6"></path>
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
              <path d="M3 22v-6h6"></path>
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
            </svg>
            Refresh Feed
          </button>
          <button class="action-btn" onclick="this.closest('podcast-detail-page').handleRescanFiles()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
              <line x1="12" y1="11" x2="12" y2="17"></line>
              <line x1="9" y1="14" x2="15" y2="14"></line>
            </svg>
            Rescan
          </button>
          <button class="action-btn danger" onclick="this.closest('podcast-detail-page').handleDelete()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
            Delete
          </button>
        </div>

        <!-- Episodes -->
        <div class="episodes-section">
          <h2 class="section-title">Episodes (${sortedEpisodes.length})</h2>
          ${
            sortedEpisodes.length > 0
              ? html`
            <table class="episodes-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>Title</th>
                  <th>Air Date</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                ${sortedEpisodes
                  .map(
                    (ep) => html`
                  <tr>
                    <td>${ep.episodeNumber}</td>
                    <td class="title-cell">${escapeHtml(ep.title)}</td>
                    <td>${ep.airDate ? new Date(ep.airDate).toLocaleDateString() : '-'}</td>
                    <td>
                      <span class="file-badge ${ep.hasFile ? 'yes' : 'no'}">
                        ${ep.hasFile ? 'Downloaded' : ep.monitored ? 'Missing' : 'Unmonitored'}
                      </span>
                    </td>
                  </tr>
                `,
                  )
                  .join('')}
              </tbody>
            </table>
          `
              : html`<p class="no-episodes">No episodes found</p>`
          }
        </div>
      </div>

      ${this.styles()}
    `;
  }

  private styles(): string {
    return html`
      <style>
        .podcast-detail {
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

        .podcast-title {
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

        .episodes-section {
          padding: 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
        }

        .episodes-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .episodes-table th,
        .episodes-table td {
          padding: 0.75rem 1rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color-light);
        }

        .episodes-table th {
          font-weight: 600;
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-color-muted);
        }

        .title-cell {
          font-weight: 500;
        }

        .file-badge {
          display: inline-block;
          padding: 0.2rem 0.5rem;
          border-radius: 0.25rem;
          font-size: 0.75rem;
          font-weight: 600;
        }

        .file-badge.yes {
          background: rgba(39, 174, 96, 0.15);
          color: var(--color-success);
        }

        .file-badge.no {
          background: rgba(220, 53, 69, 0.15);
          color: var(--color-danger);
        }

        .no-episodes {
          color: var(--text-color-muted);
          text-align: center;
          padding: 2rem;
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
    navigate('/podcasts');
  }

  async handleRefreshFeed(): Promise<void> {
    const id = this.podcastId.value;
    if (!id) return;

    try {
      await http.post('/command', { name: 'RefreshPodcast', podcastId: id });
      showSuccess('Refreshing podcast feed...');

      setTimeout(() => {
        invalidateQueries(['/podcast', id]);
        invalidateQueries(['/podcast']);
        invalidateQueries(['/podcast', id, 'episodes']);
        this.podcastQuery?.refetch();
        this.episodesQuery?.refetch();
      }, 5000);
    } catch {
      showError('Failed to refresh podcast feed');
    }
  }

  async handleRescanFiles(): Promise<void> {
    const id = this.podcastId.value;
    if (!id) return;

    try {
      await http.post('/command', { name: 'RescanPodcast', podcastId: id });
      showInfo('Scanning for podcast files...');
    } catch {
      showError('Failed to scan files');
    }
  }

  async handleDelete(): Promise<void> {
    const podcast = this.podcastQuery?.data.value;
    if (!podcast) return;

    if (!confirm(`Are you sure you want to delete "${podcast.title}"?`)) return;

    try {
      await http.delete(`/podcast/${podcast.id}`, { params: { deleteFiles: false } });
      showSuccess(`Deleted "${podcast.title}"`);
      invalidateQueries(['/podcast']);
      navigate('/podcasts');
    } catch {
      showError('Failed to delete podcast');
    }
  }
}
