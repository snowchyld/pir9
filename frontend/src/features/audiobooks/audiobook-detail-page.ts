/**
 * Audiobook Detail page - shows audiobook info with chapter list
 */

import type { ReleaseSearchModal } from '../../components/release-search-modal';
import '../../components/release-search-modal';
import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { type Audiobook, type AudiobookChapter, http } from '../../core/http';
import { createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showInfo, showSuccess } from '../../stores/app.store';

@customElement('audiobook-detail-page')
export class AudiobookDetailPage extends BaseComponent {
  private audiobookId = signal<number | null>(null);
  private titleSlug = signal<string | null>(null);

  private audiobookQuery: ReturnType<typeof createQuery<Audiobook | null>> | null = null;
  private chaptersQuery: ReturnType<typeof createQuery<AudiobookChapter[]>> | null = null;

  static get observedAttributes(): string[] {
    return ['titleslug'];
  }

  private createQueries(id: number): void {
    this.audiobookQuery = createQuery({
      queryKey: ['/audiobook', id],
      queryFn: () => http.get<Audiobook>(`/audiobook/${id}`),
    });

    this.chaptersQuery = createQuery({
      queryKey: ['/audiobook', id, 'chapters'],
      queryFn: () => http.get<AudiobookChapter[]>(`/audiobook/${id}/chapters`),
    });

    this.watch(this.audiobookQuery.data, () => this.requestUpdate());
    this.watch(this.audiobookQuery.isLoading, () => this.requestUpdate());
    this.watch(this.chaptersQuery.data, () => this.requestUpdate());
  }

  private setAudiobookId(id: number): void {
    this.audiobookId.set(id);
    this.createQueries(id);
  }

  private async lookupAudiobookId(slug: string): Promise<void> {
    try {
      const audiobookList = await http.get<Audiobook[]>('/audiobook');
      if (audiobookList) {
        const audiobook = audiobookList.find((a) => a.titleSlug === slug);
        if (audiobook) {
          this.setAudiobookId(audiobook.id);
        } else {
          showError(`Audiobook not found: ${slug}`);
        }
      }
    } catch {
      showError('Failed to load audiobook');
    }
  }

  protected onInit(): void {
    this.watch(this.audiobookId);
    this.watch(this.titleSlug);
  }

  protected onMount(): void {
    const slug = this.getAttribute('titleslug');
    if (slug && !this.audiobookId.value) {
      this.titleSlug.set(slug);
      this.lookupAudiobookId(slug);
    }
  }

  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null): void {
    if (name === 'titleslug' && newValue && newValue !== oldValue) {
      this.titleSlug.set(newValue);
      if (this._isConnected) {
        this.lookupAudiobookId(newValue);
      }
    }
  }

  protected template(): string {
    const audiobook = this.audiobookQuery?.data.value;
    const isLoading = this.audiobookQuery?.isLoading.value ?? true;
    const chapters = this.chaptersQuery?.data.value ?? [];

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
          <span>Loading audiobook...</span>
        </div>
        ${this.styles()}
      `;
    }

    if (!audiobook) {
      return html`
        <div class="error-container">
          <p>Audiobook not found</p>
          <button class="back-btn" onclick="this.closest('audiobook-detail-page').handleBack()">Back to Audiobooks</button>
        </div>
        ${this.styles()}
      `;
    }

    const posterImage = audiobook.images?.find((i) => i.coverType === 'poster');

    // Sort chapters by chapter number
    const sortedChapters = [...chapters].sort((a, b) => a.chapterNumber - b.chapterNumber);

    return html`
      <div class="audiobook-detail">
        <!-- Header -->
        <div class="detail-header">
          <button class="back-btn" onclick="this.closest('audiobook-detail-page').handleBack()">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="15 18 9 12 15 6"></polyline>
            </svg>
            Audiobooks
          </button>

          <div class="header-content">
            <div class="poster-container">
              ${
                posterImage
                  ? `<img class="detail-poster" src="${escapeHtml(posterImage.url)}" alt="${escapeHtml(audiobook.title)}">`
                  : `<div class="detail-poster-placeholder">
                    <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
                      <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"></path>
                      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"></path>
                    </svg>
                  </div>`
              }
            </div>

            <div class="header-info">
              <h1 class="audiobook-title">${escapeHtml(audiobook.title)}</h1>
              <div class="meta-row">
                ${audiobook.author ? `<span class="meta-item">by ${escapeHtml(audiobook.author)}</span>` : ''}
                ${audiobook.narrator ? `<span class="meta-item">narrated by ${escapeHtml(audiobook.narrator)}</span>` : ''}
                <span class="meta-item">${audiobook.statistics?.chapterCount ?? 0} chapters</span>
              </div>
              ${
                audiobook.genres.length > 0
                  ? `
                <div class="genres">
                  ${audiobook.genres.map((g) => `<span class="genre-tag">${escapeHtml(g)}</span>`).join('')}
                </div>
              `
                  : ''
              }
              ${audiobook.overview ? `<p class="overview">${escapeHtml(audiobook.overview)}</p>` : ''}

              <div class="stats-row">
                <div class="stat">
                  <span class="stat-value">${this.formatSize(audiobook.statistics?.sizeOnDisk ?? 0)}</span>
                  <span class="stat-label">Size</span>
                </div>
                <div class="stat">
                  <span class="stat-value">${audiobook.statistics?.chapterFileCount ?? 0} / ${audiobook.statistics?.chapterCount ?? 0}</span>
                  <span class="stat-label">Downloaded</span>
                </div>
                <div class="stat">
                  <span class="stat-value">${audiobook.statistics?.percentOfChapters?.toFixed(0) ?? 0}%</span>
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
              <span class="info-value">${escapeHtml(audiobook.path)}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Quality Profile</span>
              <span class="info-value">${audiobook.qualityProfileId}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Monitored</span>
              <span class="info-value">${audiobook.monitored ? 'Yes' : 'No'}</span>
            </div>
            <div class="info-item">
              <span class="info-label">Added</span>
              <span class="info-value">${new Date(audiobook.added).toLocaleDateString()}</span>
            </div>
            ${audiobook.isbn ? `<div class="info-item"><span class="info-label">ISBN</span><span class="info-value">${escapeHtml(audiobook.isbn)}</span></div>` : ''}
            ${audiobook.asin ? `<div class="info-item"><span class="info-label">ASIN</span><span class="info-value">${escapeHtml(audiobook.asin)}</span></div>` : ''}
            ${audiobook.publisher ? `<div class="info-item"><span class="info-label">Publisher</span><span class="info-value">${escapeHtml(audiobook.publisher)}</span></div>` : ''}
          </div>
        </div>

        <!-- Actions -->
        <div class="actions-panel">
          <button class="action-btn" onclick="this.closest('audiobook-detail-page').handleInteractiveSearch()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            Search &amp; Download
          </button>
          <button class="action-btn primary" onclick="this.closest('audiobook-detail-page').handleRefresh()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 2v6h-6"></path>
              <path d="M3 12a9 9 0 0 1 15-6.7L21 8"></path>
              <path d="M3 22v-6h6"></path>
              <path d="M21 12a9 9 0 0 1-15 6.7L3 16"></path>
            </svg>
            Refresh
          </button>
          <button class="action-btn" onclick="this.closest('audiobook-detail-page').handleRescanFiles()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path>
              <line x1="12" y1="11" x2="12" y2="17"></line>
              <line x1="9" y1="14" x2="15" y2="14"></line>
            </svg>
            Rescan
          </button>
          <button class="action-btn danger" onclick="this.closest('audiobook-detail-page').handleDelete()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="3 6 5 6 21 6"></polyline>
              <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
            </svg>
            Delete
          </button>
        </div>

        <!-- Chapters -->
        <div class="chapters-section">
          <h2 class="section-title">Chapters (${sortedChapters.length})</h2>
          ${
            sortedChapters.length > 0
              ? html`
            <table class="chapters-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>Title</th>
                  <th>Duration</th>
                  <th>Status</th>
                </tr>
              </thead>
              <tbody>
                ${sortedChapters
                  .map(
                    (ch) => html`
                  <tr>
                    <td>${ch.chapterNumber}</td>
                    <td class="title-cell">${escapeHtml(ch.title)}</td>
                    <td>${ch.durationMs ? this.formatDuration(ch.durationMs) : '-'}</td>
                    <td>
                      <span class="file-badge ${ch.hasFile ? 'yes' : 'no'}">
                        ${ch.hasFile ? 'Downloaded' : ch.monitored ? 'Missing' : 'Unmonitored'}
                      </span>
                    </td>
                  </tr>
                `,
                  )
                  .join('')}
              </tbody>
            </table>
          `
              : html`<p class="no-chapters">No chapters found</p>`
          }
        </div>
      </div>

      <release-search-modal></release-search-modal>

      ${this.styles()}
    `;
  }

  private styles(): string {
    return html`
      <style>
        .audiobook-detail {
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

        .header-content { display: flex; gap: 1.5rem; }

        .detail-poster {
          width: 180px;
          aspect-ratio: 2/3;
          object-fit: cover;
          border-radius: 0.5rem;
          box-shadow: 0 4px 20px rgba(0,0,0,0.3);
          flex-shrink: 0;
        }

        .detail-poster-placeholder {
          width: 180px;
          aspect-ratio: 2/3;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--bg-card-center);
          border-radius: 0.5rem;
          color: var(--text-color-muted);
          flex-shrink: 0;
        }

        .header-info { flex: 1; display: flex; flex-direction: column; gap: 0.75rem; }

        .audiobook-title { font-size: 1.75rem; font-weight: 700; margin: 0; }

        .meta-row { display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap; }
        .meta-item { color: var(--text-color-muted); font-size: 0.875rem; }

        .genres { display: flex; gap: 0.375rem; flex-wrap: wrap; }
        .genre-tag {
          padding: 0.125rem 0.5rem;
          background: var(--bg-card-center);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .overview { color: var(--text-color-muted); font-size: 0.875rem; line-height: 1.5; margin: 0; }

        .stats-row { display: flex; gap: 1.5rem; margin-top: 0.5rem; }
        .stat { display: flex; flex-direction: column; gap: 0.125rem; }
        .stat-value { font-size: 1.125rem; font-weight: 600; }
        .stat-label { font-size: 0.75rem; color: var(--text-color-muted); text-transform: uppercase; letter-spacing: 0.05em; }

        .info-panel { padding: 1.25rem; background: var(--bg-card); border: 1px solid var(--border-glass); border-radius: 0.75rem; }
        .info-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(250px, 1fr)); gap: 1rem; }
        .info-item { display: flex; flex-direction: column; gap: 0.25rem; }
        .info-label { font-size: 0.75rem; color: var(--text-color-muted); text-transform: uppercase; letter-spacing: 0.05em; }
        .info-value { font-size: 0.875rem; word-break: break-all; }

        .actions-panel { display: flex; gap: 0.75rem; padding: 1rem 1.25rem; background: var(--bg-card); border: 1px solid var(--border-glass); border-radius: 0.75rem; }

        .action-btn {
          display: flex; align-items: center; gap: 0.375rem; padding: 0.5rem 0.875rem;
          border: 1px solid var(--border-input); border-radius: 0.5rem; background: var(--bg-input);
          color: var(--text-color); cursor: pointer; font-size: 0.875rem; transition: all var(--transition-normal);
        }
        .action-btn:hover { border-color: var(--pir9-blue); color: var(--pir9-blue); }
        .action-btn.primary { background-color: var(--btn-primary-bg); border-color: var(--btn-primary-bg); color: white; }
        .action-btn.primary:hover { background-color: var(--btn-primary-bg-hover); border-color: var(--btn-primary-bg-hover); color: white; }
        .action-btn.danger:hover { border-color: var(--color-danger); color: var(--color-danger); }

        .chapters-section { padding: 1.25rem; background: var(--bg-card); border: 1px solid var(--border-glass); border-radius: 0.75rem; }
        .section-title { font-size: 1.125rem; font-weight: 600; margin: 0 0 1rem 0; }

        .chapters-table { width: 100%; border-collapse: collapse; font-size: 0.875rem; }
        .chapters-table th, .chapters-table td { padding: 0.75rem 1rem; text-align: left; border-bottom: 1px solid var(--border-color-light); }
        .chapters-table th { font-weight: 600; font-size: 0.75rem; text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-color-muted); }
        .title-cell { font-weight: 500; }

        .file-badge { display: inline-block; padding: 0.2rem 0.5rem; border-radius: 0.25rem; font-size: 0.75rem; font-weight: 600; }
        .file-badge.yes { background: rgba(39, 174, 96, 0.15); color: var(--color-success); }
        .file-badge.no { background: rgba(220, 53, 69, 0.15); color: var(--color-danger); }

        .no-chapters { color: var(--text-color-muted); text-align: center; padding: 2rem; }
        .error-container { display: flex; flex-direction: column; align-items: center; gap: 1rem; padding: 6rem 2rem; text-align: center; }

        @media (max-width: 640px) {
          .header-content { flex-direction: column; align-items: center; text-align: center; }
          .meta-row, .genres, .stats-row { justify-content: center; }
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

  private formatDuration(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000);
    const hours = Math.floor(totalSeconds / 3600);
    const minutes = Math.floor((totalSeconds % 3600) / 60);
    const seconds = totalSeconds % 60;
    if (hours > 0) {
      return `${hours}h ${minutes}m`;
    }
    return `${minutes}m ${seconds}s`;
  }

  // Event handlers
  handleBack(): void {
    navigate('/audiobooks');
  }

  handleInteractiveSearch(): void {
    const audiobook = this.audiobookQuery?.data.value;
    if (!audiobook) return;

    const modal = this.querySelector('release-search-modal') as ReleaseSearchModal | null;
    if (modal) {
      modal.open({
        query: `${audiobook.title} ${audiobook.author ?? ''} audiobook`.trim(),
        queryTitle: audiobook.title,
      });
    }
  }

  async handleRefresh(): Promise<void> {
    const id = this.audiobookId.value;
    if (!id) return;

    try {
      await http.post(`/audiobook/${id}/refresh`, {});
      showSuccess('Refreshing audiobook metadata...');

      setTimeout(() => {
        invalidateQueries(['/audiobook', id]);
        invalidateQueries(['/audiobook']);
        invalidateQueries(['/audiobook', id, 'chapters']);
        this.audiobookQuery?.refetch();
        this.chaptersQuery?.refetch();
      }, 3000);
    } catch {
      showError('Failed to refresh audiobook metadata');
    }
  }

  async handleRescanFiles(): Promise<void> {
    const id = this.audiobookId.value;
    if (!id) return;

    try {
      await http.post(`/audiobook/${id}/rescan`, {});
      showInfo('Scanning for audiobook files...');
    } catch {
      showError('Failed to scan files');
    }
  }

  async handleDelete(): Promise<void> {
    const audiobook = this.audiobookQuery?.data.value;
    if (!audiobook) return;

    if (!confirm(`Are you sure you want to delete "${audiobook.title}"?`)) return;

    try {
      await http.delete(`/audiobook/${audiobook.id}`, { params: { deleteFiles: false } });
      showSuccess(`Deleted "${audiobook.title}"`);
      invalidateQueries(['/audiobook']);
      navigate('/audiobooks');
    } catch {
      showError('Failed to delete audiobook');
    }
  }
}
