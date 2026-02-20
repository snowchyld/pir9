/**
 * Dashboard page — system overview with stats, health, disk space, and active downloads
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import type {
  DiskSpace,
  HealthCheck,
  Movie,
  QueueItem,
  QueueResponse,
  Series,
  SystemStatus,
  UpdateInfo,
} from '../../core/http';
import {
  useDiskSpaceQuery,
  useHealthQuery,
  useMoviesQuery,
  useQueueQuery,
  useSeriesQuery,
  useSystemStatusQuery,
  useUpdateQuery,
} from '../../core/query';
import { navigate } from '../../router';

@customElement('dashboard-page')
export class DashboardPage extends BaseComponent {
  private statusQuery = useSystemStatusQuery();
  private healthQuery = useHealthQuery();
  private diskSpaceQuery = useDiskSpaceQuery();
  private queueQuery = useQueueQuery();
  private seriesQuery = useSeriesQuery();
  private moviesQuery = useMoviesQuery();
  private updateQuery = useUpdateQuery();

  protected onInit(): void {
    this.watch(this.statusQuery.data);
    this.watch(this.healthQuery.data);
    this.watch(this.diskSpaceQuery.data);
    this.watch(this.queueQuery.data);
    this.watch(this.seriesQuery.data);
    this.watch(this.moviesQuery.data);
    this.watch(this.updateQuery.data);
    this.watch(this.statusQuery.isLoading);
  }

  protected template(): string {
    const status = this.statusQuery.data.value as SystemStatus | undefined;
    const health = (this.healthQuery.data.value ?? []) as HealthCheck[];
    const disks = (this.diskSpaceQuery.data.value ?? []) as DiskSpace[];
    const queueResponse = this.queueQuery.data.value as QueueResponse | undefined;
    const queueItems = queueResponse?.records ?? [];
    const series = (this.seriesQuery.data.value ?? []) as Series[];
    const movies = (this.moviesQuery.data.value ?? []) as Movie[];
    const update = this.updateQuery.data.value as UpdateInfo | undefined;
    const isLoading = this.statusQuery.isLoading.value;

    return html`
      <div class="dashboard">
        <div class="toolbar">
          <h1 class="page-title">Dashboard</h1>
        </div>

        ${isLoading && !status ? this.renderLoading() : ''}

        <!-- Quick stats -->
        ${safeHtml(this.renderQuickStats(series, movies, queueItems, disks))}

        <!-- Health checks -->
        ${health.length > 0 ? safeHtml(this.renderHealthCards(health)) : ''}

        <!-- Disk space -->
        ${disks.length > 0 ? safeHtml(this.renderDiskSpace(disks)) : ''}

        <!-- Active downloads -->
        ${safeHtml(this.renderActiveDownloads(queueItems))}

        <!-- System info -->
        ${status ? safeHtml(this.renderSystemInfo(status, update)) : ''}
      </div>

      <style>
        .dashboard {
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
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          -webkit-backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
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

        .loading-container {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 4rem;
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

        /* Quick stats grid */
        .stats-grid {
          display: grid;
          grid-template-columns: repeat(4, 1fr);
          gap: 1rem;
        }

        @media (max-width: 768px) {
          .stats-grid { grid-template-columns: repeat(2, 1fr); }
        }

        .stat-card {
          padding: 1.25rem;
          background: var(--bg-card);
          backdrop-filter: blur(var(--glass-blur));
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .stat-card:hover {
          transform: translateY(-2px);
          box-shadow: var(--shadow-card-hover);
          border-color: rgba(93, 156, 236, 0.3);
        }

        .stat-label {
          font-size: 0.75rem;
          text-transform: uppercase;
          letter-spacing: 0.05em;
          color: var(--text-color-muted);
          margin-bottom: 0.5rem;
        }

        .stat-value {
          font-size: 2rem;
          font-weight: 700;
          line-height: 1;
        }

        .stat-value.blue { color: var(--pir9-blue); }
        .stat-value.green { color: var(--color-success); }
        .stat-value.orange { color: var(--color-warning); }

        /* Section headers */
        .section-header {
          font-size: 1rem;
          font-weight: 600;
          color: var(--text-color-muted);
          text-transform: uppercase;
          letter-spacing: 0.05em;
          margin: 0.5rem 0 0 0;
        }

        /* Health cards */
        .health-section {
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .health-grid {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .health-card {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.875rem 1rem;
          border-radius: 0.625rem;
          border: 1px solid;
        }

        .health-card.error {
          background: rgba(240, 80, 80, 0.1);
          border-color: rgba(240, 80, 80, 0.3);
        }

        .health-card.warning {
          background: rgba(255, 144, 43, 0.1);
          border-color: rgba(255, 144, 43, 0.3);
        }

        .health-card.notice {
          background: rgba(93, 156, 236, 0.1);
          border-color: rgba(93, 156, 236, 0.3);
        }

        .health-card.ok {
          background: rgba(39, 194, 76, 0.1);
          border-color: rgba(39, 194, 76, 0.3);
        }

        .health-icon {
          flex-shrink: 0;
          width: 20px;
          height: 20px;
        }

        .health-icon.error { color: var(--color-danger); }
        .health-icon.warning { color: var(--color-warning); }
        .health-icon.notice { color: var(--pir9-blue); }
        .health-icon.ok { color: var(--color-success); }

        .health-source {
          font-weight: 600;
          font-size: 0.875rem;
          min-width: 8rem;
        }

        .health-message {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          flex: 1;
        }

        /* Disk space */
        .disk-section {
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .disk-grid {
          display: grid;
          grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
          gap: 1rem;
        }

        .disk-card {
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        .disk-label {
          font-size: 0.875rem;
          font-weight: 500;
          margin-bottom: 0.625rem;
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
        }

        .disk-bar {
          height: 8px;
          background: var(--bg-progress);
          border-radius: 4px;
          overflow: hidden;
          margin-bottom: 0.5rem;
        }

        .disk-fill {
          height: 100%;
          border-radius: 4px;
          transition: width 0.5s ease;
        }

        .disk-fill.ok { background: var(--color-success); }
        .disk-fill.warn { background: var(--color-warning); }
        .disk-fill.critical { background: var(--color-danger); }

        .disk-text {
          display: flex;
          justify-content: space-between;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        /* Active downloads */
        .downloads-section {
          display: flex;
          flex-direction: column;
          gap: 0.75rem;
        }

        .downloads-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
        }

        .view-all-link {
          font-size: 0.875rem;
          color: var(--pir9-blue);
          cursor: pointer;
          text-decoration: none;
        }

        .view-all-link:hover {
          text-decoration: underline;
        }

        .download-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .download-item {
          display: grid;
          grid-template-columns: 1fr 200px 80px 80px;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem 1rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.625rem;
          font-size: 0.875rem;
        }

        @media (max-width: 768px) {
          .download-item {
            grid-template-columns: 1fr;
            gap: 0.5rem;
          }
        }

        .dl-title {
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          font-weight: 500;
        }

        .dl-progress {
          display: flex;
          flex-direction: column;
          gap: 0.25rem;
        }

        .dl-bar {
          height: 6px;
          background: var(--bg-progress);
          border-radius: 3px;
          overflow: hidden;
        }

        .dl-fill {
          height: 100%;
          background: var(--color-primary);
          border-radius: 3px;
          transition: width 0.3s;
        }

        .dl-pct {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-align: right;
        }

        .dl-speed, .dl-eta {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          text-align: right;
        }

        .empty-downloads {
          padding: 2rem;
          text-align: center;
          color: var(--text-color-muted);
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
        }

        /* System info */
        .system-info {
          display: flex;
          flex-wrap: wrap;
          gap: 0.75rem;
          padding: 1rem 1.25rem;
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          border-radius: 0.75rem;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
        }

        .info-item {
          display: flex;
          align-items: center;
          gap: 0.375rem;
        }

        .info-label {
          font-weight: 600;
          color: var(--text-color);
        }

        .badge {
          display: inline-flex;
          padding: 0.125rem 0.5rem;
          font-size: 0.75rem;
          font-weight: 500;
          border-radius: 9999px;
        }

        .badge.docker {
          background: rgba(93, 156, 236, 0.15);
          border: 1px solid rgba(93, 156, 236, 0.3);
          color: var(--pir9-blue);
        }

        .badge.update {
          background: rgba(39, 194, 76, 0.15);
          border: 1px solid rgba(39, 194, 76, 0.3);
          color: var(--color-success);
        }

        .info-separator {
          width: 1px;
          height: 1rem;
          background: var(--border-color);
        }
      </style>
    `;
  }

  private renderLoading(): string {
    return html`
      <div class="loading-container">
        <div class="loading-spinner"></div>
      </div>
    `;
  }

  private renderQuickStats(
    series: Series[],
    movies: Movie[],
    queueItems: QueueItem[],
    disks: DiskSpace[],
  ): string {
    const totalFree = disks.reduce((sum, d) => sum + d.freeSpace, 0);

    return html`
      <div class="stats-grid">
        <div class="stat-card" onclick="this.closest('dashboard-page').handleNav('/series')">
          <div class="stat-label">Series</div>
          <div class="stat-value blue">${series.length}</div>
        </div>
        <div class="stat-card" onclick="this.closest('dashboard-page').handleNav('/movies')">
          <div class="stat-label">Movies</div>
          <div class="stat-value blue">${movies.length}</div>
        </div>
        <div class="stat-card" onclick="this.closest('dashboard-page').handleNav('/activity/queue')">
          <div class="stat-label">Queue</div>
          <div class="stat-value ${queueItems.length > 0 ? 'orange' : 'green'}">${queueItems.length}</div>
        </div>
        <div class="stat-card">
          <div class="stat-label">Disk Free</div>
          <div class="stat-value green">${this.formatSize(totalFree)}</div>
        </div>
      </div>
    `;
  }

  private renderHealthCards(checks: HealthCheck[]): string {
    const iconMap: Record<string, string> = {
      error:
        '<svg class="health-icon error" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>',
      warning:
        '<svg class="health-icon warning" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
      notice:
        '<svg class="health-icon notice" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>',
      ok: '<svg class="health-icon ok" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"/><polyline points="22 4 12 14.01 9 11.01"/></svg>',
    };

    return html`
      <div class="health-section">
        <h2 class="section-header">Health</h2>
        <div class="health-grid">
          ${checks
            .map(
              (c) => html`
              <div class="health-card ${c.type}">
                ${safeHtml(iconMap[c.type] ?? iconMap.notice)}
                <span class="health-source">${escapeHtml(c.source)}</span>
                <span class="health-message">${escapeHtml(c.message)}</span>
              </div>
            `,
            )
            .join('')}
        </div>
      </div>
    `;
  }

  private renderDiskSpace(disks: DiskSpace[]): string {
    return html`
      <div class="disk-section">
        <h2 class="section-header">Disk Space</h2>
        <div class="disk-grid">
          ${disks.map((d) => this.renderDiskCard(d)).join('')}
        </div>
      </div>
    `;
  }

  private renderDiskCard(disk: DiskSpace): string {
    const used = disk.totalSpace - disk.freeSpace;
    const pct = disk.totalSpace > 0 ? (used / disk.totalSpace) * 100 : 0;
    const fillClass = pct > 90 ? 'critical' : pct > 75 ? 'warn' : 'ok';
    const label = disk.label || disk.path;

    return html`
      <div class="disk-card">
        <div class="disk-label" title="${escapeHtml(disk.path)}">${escapeHtml(label)}</div>
        <div class="disk-bar">
          <div class="disk-fill ${fillClass}" style="width: ${pct.toFixed(1)}%"></div>
        </div>
        <div class="disk-text">
          <span>${this.formatSize(used)} used</span>
          <span>${this.formatSize(disk.freeSpace)} free / ${this.formatSize(disk.totalSpace)}</span>
        </div>
      </div>
    `;
  }

  private renderActiveDownloads(items: QueueItem[]): string {
    const active = items.filter((i) => i.status.toLowerCase() !== 'completed').slice(0, 10);

    return html`
      <div class="downloads-section">
        <div class="downloads-header">
          <h2 class="section-header">Active Downloads</h2>
          ${items.length > 0 ? `<a class="view-all-link" href="/activity/queue" onclick="event.preventDefault(); this.closest('dashboard-page').handleNav('/activity/queue')">View All (${items.length})</a>` : ''}
        </div>
        ${
          active.length > 0
            ? html`<div class="download-list">${active.map((i) => this.renderDownloadItem(i)).join('')}</div>`
            : html`<div class="empty-downloads">No active downloads</div>`
        }
      </div>
    `;
  }

  private renderDownloadItem(item: QueueItem): string {
    const pct = item.size > 0 ? ((item.size - item.sizeleft) / item.size) * 100 : 0;
    const isMovie = item.contentType === 'movie' || (item.movieId != null && item.movieId > 0);
    const title = isMovie ? (item.movie?.title ?? item.title) : (item.series?.title ?? item.title);

    return html`
      <div class="download-item">
        <div class="dl-title" title="${escapeHtml(title)}">${escapeHtml(title)}</div>
        <div class="dl-progress">
          <div class="dl-bar">
            <div class="dl-fill" style="width: ${pct}%"></div>
          </div>
        </div>
        <div class="dl-pct">${pct.toFixed(1)}%</div>
        <div class="dl-eta">${item.timeleft ?? '-'}</div>
      </div>
    `;
  }

  private renderSystemInfo(status: SystemStatus, update?: UpdateInfo): string {
    const uptime = status.startTime ? this.formatUptime(status.startTime) : '-';

    return html`
      <div class="system-info">
        <div class="info-item">
          <span class="info-label">Version</span>
          <span>${escapeHtml(status.version)}</span>
        </div>
        <div class="info-separator"></div>
        <div class="info-item">
          <span class="info-label">Uptime</span>
          <span>${uptime}</span>
        </div>
        <div class="info-separator"></div>
        <div class="info-item">
          <span class="info-label">DB</span>
          <span>${escapeHtml(status.databaseType ?? 'SQLite')}</span>
        </div>
        ${
          status.isDocker
            ? html`
              <div class="info-separator"></div>
              <div class="info-item">
                <span class="badge docker">Docker</span>
              </div>
            `
            : ''
        }
        ${
          update?.updateAvailable
            ? html`
              <div class="info-separator"></div>
              <div class="info-item">
                <span class="badge update">${escapeHtml(update.version)} available</span>
              </div>
            `
            : ''
        }
      </div>
    `;
  }

  private formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }

  private formatUptime(startTime: string): string {
    const start = new Date(startTime);
    const now = new Date();
    const diffMs = now.getTime() - start.getTime();
    const hours = Math.floor(diffMs / 3600000);
    const days = Math.floor(hours / 24);
    const remainHours = hours % 24;

    if (days > 0) return `${days}d ${remainHours}h`;
    if (hours > 0) return `${hours}h`;
    const minutes = Math.floor(diffMs / 60000);
    return `${minutes}m`;
  }

  handleNav(path: string): void {
    navigate(path);
  }
}
