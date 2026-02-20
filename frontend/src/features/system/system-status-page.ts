/**
 * System Status page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';

interface SystemStatus {
  appName: string;
  instanceName: string;
  version: string;
  buildTime: string;
  isDebug: boolean;
  isProduction: boolean;
  isAdmin: boolean;
  isUserInteractive: boolean;
  startupPath: string;
  appData: string;
  osName: string;
  osVersion: string;
  isNetCore: boolean;
  isLinux: boolean;
  isOsx: boolean;
  isWindows: boolean;
  isDocker: boolean;
  mode: string;
  branch: string;
  authentication: string;
  databaseType: string;
  databaseVersion: string;
  migrationVersion: number;
  urlBase: string;
  runtimeVersion: string;
  runtimeName: string;
  startTime: string;
  packageVersion: string;
  packageAuthor: string;
  packageUpdateMechanism: string;
}

interface DiskSpace {
  path: string;
  label: string;
  freeSpace: number;
  totalSpace: number;
}

interface HealthCheck {
  source: string;
  type: 'error' | 'warning' | 'notice';
  message: string;
  wikiUrl?: string;
}

interface ScanProgress {
  stage: string;
  currentFile?: string;
  filesTotal: number;
  filesProcessed: number;
  percent: number;
  detail?: string;
}

interface RunningTask {
  id: string;
  taskType: 'command' | 'scan';
  name: string;
  status: string;
  started?: string;
  message?: string;
  detail?: string;
  workerId?: string;
  progress?: ScanProgress;
}

@customElement('system-status-page')
export class SystemStatusPage extends BaseComponent {
  private statusQuery = createQuery({
    queryKey: ['/system/status'],
    queryFn: () => http.get<SystemStatus>('/system/status'),
  });

  private diskSpaceQuery = createQuery({
    queryKey: ['/diskspace'],
    queryFn: () => http.get<DiskSpace[]>('/diskspace'),
  });

  private healthQuery = createQuery({
    queryKey: ['/health'],
    queryFn: () => http.get<HealthCheck[]>('/health'),
  });

  private runningTasksQuery = createQuery({
    queryKey: ['/system/task/running'],
    queryFn: () => http.get<RunningTask[]>('/system/task/running'),
    refetchInterval: 3000,
  });

  private cancelCommandMutation = createMutation({
    mutationFn: (id: string) => http.delete<void>(`/command/${id}`),
    onSuccess: () => {
      invalidateQueries(['/system/task/running']);
      invalidateQueries(['/command']);
    },
  });

  private cancelScanMutation = createMutation({
    mutationFn: (id: string) => http.delete<void>(`/system/task/scan/${id}`),
    onSuccess: () => {
      invalidateQueries(['/system/task/running']);
    },
  });

  private elapsedTimer: number | null = null;

  protected onInit(): void {
    this.watch(this.statusQuery.data);
    this.watch(this.statusQuery.isLoading);
    this.watch(this.diskSpaceQuery.data);
    this.watch(this.healthQuery.data);
    this.watch(this.runningTasksQuery.data);
  }

  protected onMount(): void {
    this.elapsedTimer = window.setInterval(() => {
      const tasks = this.runningTasksQuery.data.value ?? [];
      if (tasks.length > 0) {
        this.requestUpdate();
      }
    }, 1000);
  }

  protected onDestroy(): void {
    if (this.elapsedTimer !== null) {
      clearInterval(this.elapsedTimer);
      this.elapsedTimer = null;
    }
  }

  protected template(): string {
    const status = this.statusQuery.data.value;
    const diskSpace = this.diskSpaceQuery.data.value ?? [];
    const health = this.healthQuery.data.value ?? [];
    const runningTasks = this.runningTasksQuery.data.value ?? [];
    const isLoading = this.statusQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="status-page">
        <h1 class="page-title">System Status</h1>

        ${runningTasks.length > 0 ? html`
          <div class="running-section">
            <h2 class="section-title">Running Tasks</h2>
            <div class="running-list">
              ${runningTasks.map((task) => html`
                <div class="running-item">
                  <div class="running-icon">
                    <svg class="spinner-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M21 12a9 9 0 1 1-6.219-8.56"></path>
                    </svg>
                  </div>
                  <div class="running-content">
                    <div class="running-name">${escapeHtml(this.formatTaskName(task))}</div>
                    ${task.progress ? html`
                      <div class="running-progress">
                        <div class="progress-bar">
                          <div class="progress-fill" style="width: ${task.progress.percent}%"></div>
                        </div>
                        <span class="progress-text">${task.progress.filesProcessed}/${task.progress.filesTotal}</span>
                        <span class="progress-pct">${task.progress.percent}%</span>
                      </div>
                      <div class="running-detail">
                        <span class="running-stage">${escapeHtml(this.formatStage(task.progress.stage))}</span>
                        ${task.started ? html`<span class="running-elapsed">${this.formatElapsed(task.started)}</span>` : ''}
                        ${task.workerId ? html`<span class="running-worker" title="Worker ID: ${escapeHtml(task.workerId)}">worker:${escapeHtml(task.workerId.substring(0, 8))}</span>` : ''}
                        ${task.progress.currentFile ? html`<span class="running-file" title="${escapeHtml(task.progress.currentFile)}">${escapeHtml(this.truncateFilename(task.progress.currentFile, 40))}</span>` : ''}
                        ${task.progress.detail && task.progress.detail !== 'unchanged' ? html`<span class="running-metadata">${escapeHtml(task.progress.detail)}</span>` : ''}
                      </div>
                    ` : html`
                      <div class="running-detail">
                        <span class="running-status">${task.status === 'queued' ? 'Queued' : 'Running'}</span>
                        ${task.started ? html`<span class="running-elapsed">${this.formatElapsed(task.started)}</span>` : ''}
                        ${task.workerId ? html`<span class="running-worker" title="Worker ID: ${escapeHtml(task.workerId)}">worker:${escapeHtml(task.workerId.substring(0, 8))}</span>` : ''}
                        ${task.detail ? html`<span class="running-message">${escapeHtml(task.detail)}</span>` : ''}
                        ${task.message ? html`<span class="running-message">${escapeHtml(task.message)}</span>` : ''}
                      </div>
                    `}
                  </div>
                  <button
                    class="cancel-btn"
                    onclick="this.closest('system-status-page').handleCancelTask('${escapeHtml(task.id)}', '${escapeHtml(task.taskType)}')"
                    title="Cancel task"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <circle cx="12" cy="12" r="10"></circle>
                      <line x1="15" y1="9" x2="9" y2="15"></line>
                      <line x1="9" y1="9" x2="15" y2="15"></line>
                    </svg>
                  </button>
                </div>
              `).join('')}
            </div>
          </div>
        ` : ''}

        ${health.length > 0 ? html`
          <div class="health-section">
            <h2 class="section-title">Health</h2>
            <div class="health-list">
              ${health
                .map(
                  (h) => html`
                <div class="health-item ${h.type}">
                  <div class="health-icon">
                    ${h.type === 'error' ? '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="15" y1="9" x2="9" y2="15"></line><line x1="9" y1="9" x2="15" y2="15"></line></svg>' : ''}
                    ${h.type === 'warning' ? '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"></path><line x1="12" y1="9" x2="12" y2="13"></line><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>' : ''}
                    ${h.type === 'notice' ? '<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="16" x2="12" y2="12"></line><line x1="12" y1="8" x2="12.01" y2="8"></line></svg>' : ''}
                  </div>
                  <div class="health-content">
                    <div class="health-source">${escapeHtml(h.source)}</div>
                    <div class="health-message">${escapeHtml(h.message)}</div>
                  </div>
                  ${
                    h.wikiUrl
                      ? html`
                    <a class="health-wiki" href="${h.wikiUrl}" target="_blank" rel="noopener">
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"></path><line x1="12" y1="17" x2="12.01" y2="17"></line></svg>
                    </a>
                  `
                      : ''
                  }
                </div>
              `,
                )
                .join('')}
            </div>
          </div>
        ` : ''}

        <div class="info-section">
          <h2 class="section-title">About</h2>
          <div class="info-grid">
            <div class="info-row">
              <span class="info-label">Version</span>
              <span class="info-value">${escapeHtml(status?.version ?? '-')}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Branch</span>
              <span class="info-value">${escapeHtml(status?.branch ?? '-')}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Instance Name</span>
              <span class="info-value">${escapeHtml(status?.instanceName ?? '-')}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Start Time</span>
              <span class="info-value">${status?.startTime ? new Date(status.startTime).toLocaleString() : '-'}</span>
            </div>
          </div>
        </div>

        <div class="info-section">
          <h2 class="section-title">Paths</h2>
          <div class="info-grid">
            <div class="info-row">
              <span class="info-label">App Data</span>
              <span class="info-value mono">${escapeHtml(status?.appData ?? '-')}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Startup Path</span>
              <span class="info-value mono">${escapeHtml(status?.startupPath ?? '-')}</span>
            </div>
          </div>
        </div>

        <div class="info-section">
          <h2 class="section-title">Environment</h2>
          <div class="info-grid">
            <div class="info-row">
              <span class="info-label">OS</span>
              <span class="info-value">${escapeHtml(status?.osName ?? '-')} ${escapeHtml(status?.osVersion ?? '')}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Runtime</span>
              <span class="info-value">${escapeHtml(status?.runtimeName ?? '-')} ${escapeHtml(status?.runtimeVersion ?? '')}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Docker</span>
              <span class="info-value">${status?.isDocker ? 'Yes' : 'No'}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Database</span>
              <span class="info-value">${escapeHtml(status?.databaseType ?? '-')} ${escapeHtml(status?.databaseVersion ?? '')}</span>
            </div>
          </div>
        </div>

        <div class="info-section">
          <h2 class="section-title">Disk Space</h2>
          <div class="disk-list">
            ${diskSpace
              .map((disk) => {
                const usedPercent = ((disk.totalSpace - disk.freeSpace) / disk.totalSpace) * 100;
                return html`
                <div class="disk-item">
                  <div class="disk-header">
                    <span class="disk-label">${escapeHtml(disk.label || disk.path)}</span>
                    <span class="disk-path">${escapeHtml(disk.path)}</span>
                  </div>
                  <div class="disk-bar">
                    <div class="disk-used" style="width: ${usedPercent}%"></div>
                  </div>
                  <div class="disk-stats">
                    <span>${this.formatBytes(disk.totalSpace - disk.freeSpace)} used</span>
                    <span>${this.formatBytes(disk.freeSpace)} free of ${this.formatBytes(disk.totalSpace)}</span>
                  </div>
                </div>
              `;
              })
              .join('')}
          </div>
        </div>
      </div>

      <style>
        .status-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .loading-container {
          display: flex;
          justify-content: center;
          padding: 4rem;
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

        .running-section, .health-section, .info-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .running-section {
          border-color: var(--color-primary);
        }

        .section-title {
          font-size: 1rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .running-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .running-item {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.75rem;
          background-color: var(--bg-card-alt);
          border-radius: 0.375rem;
        }

        .running-icon {
          display: flex;
          flex-shrink: 0;
          color: var(--color-primary);
        }

        .spinner-icon {
          animation: spin 1s linear infinite;
        }

        .running-content {
          flex: 1;
          min-width: 0;
        }

        .running-name {
          font-weight: 500;
        }

        .running-detail {
          display: flex;
          gap: 0.75rem;
          font-size: 0.8125rem;
          color: var(--text-color-muted);
          margin-top: 0.125rem;
        }

        .running-status {
          text-transform: capitalize;
        }

        .running-elapsed {
          font-family: monospace;
          font-size: 0.75rem;
        }

        .running-worker {
          font-family: monospace;
          font-size: 0.6875rem;
          padding: 0.0625rem 0.375rem;
          background: var(--bg-color-elevated, rgba(255, 255, 255, 0.05));
          border-radius: 0.1875rem;
          color: var(--text-color-muted);
        }

        .running-message {
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }

        .running-progress {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          margin-top: 0.25rem;
        }

        .progress-bar {
          flex: 1;
          height: 4px;
          background-color: var(--bg-progress, rgba(255, 255, 255, 0.1));
          border-radius: 2px;
          overflow: hidden;
        }

        .progress-fill {
          height: 100%;
          background-color: var(--color-primary);
          transition: width 0.3s ease;
          border-radius: 2px;
        }

        .progress-text, .progress-pct {
          font-size: 0.75rem;
          font-family: monospace;
          color: var(--text-color-muted);
          white-space: nowrap;
        }

        .progress-pct {
          min-width: 2.5rem;
          text-align: right;
        }

        .running-stage {
          font-weight: 500;
          text-transform: capitalize;
        }

        .running-file {
          font-family: monospace;
          font-size: 0.75rem;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
          max-width: 300px;
        }

        .running-metadata {
          font-size: 0.6875rem;
          padding: 0.0625rem 0.375rem;
          background: rgba(var(--color-success-rgb, 92, 184, 92), 0.15);
          color: var(--color-success, #5cb85c);
          border-radius: 0.1875rem;
          white-space: nowrap;
        }

        .cancel-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          background: transparent;
          border: none;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          cursor: pointer;
          flex-shrink: 0;
        }

        .cancel-btn:hover {
          color: var(--color-danger);
          background-color: rgba(var(--color-danger-rgb, 217, 83, 79), 0.1);
        }

        .health-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .health-item {
          display: flex;
          align-items: flex-start;
          gap: 0.75rem;
          padding: 0.75rem;
          border-radius: 0.375rem;
        }

        .health-item.error {
          background-color: rgba(var(--color-danger-rgb, 217, 83, 79), 0.1);
          border: 1px solid var(--color-danger);
        }

        .health-item.warning {
          background-color: rgba(var(--color-warning-rgb, 240, 173, 78), 0.1);
          border: 1px solid var(--color-warning);
        }

        .health-item.notice {
          background-color: rgba(var(--color-primary-rgb, 93, 156, 236), 0.1);
          border: 1px solid var(--color-primary);
        }

        .health-icon {
          display: flex;
          flex-shrink: 0;
        }

        .health-item.error .health-icon { color: var(--color-danger); }
        .health-item.warning .health-icon { color: var(--color-warning); }
        .health-item.notice .health-icon { color: var(--color-primary); }

        .health-content {
          flex: 1;
        }

        .health-source {
          font-weight: 500;
          margin-bottom: 0.25rem;
        }

        .health-message {
          font-size: 0.875rem;
          color: var(--text-color-muted);
        }

        .health-wiki {
          display: flex;
          color: var(--text-color-muted);
        }

        .health-wiki:hover {
          color: var(--color-primary);
        }

        .info-grid {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .info-row {
          display: flex;
          padding: 0.5rem 0;
          border-bottom: 1px solid var(--border-color);
        }

        .info-row:last-child {
          border-bottom: none;
        }

        .info-label {
          width: 150px;
          font-weight: 500;
          flex-shrink: 0;
        }

        .info-value {
          color: var(--text-color-muted);
        }

        .info-value.mono {
          font-family: monospace;
          font-size: 0.875rem;
        }

        .disk-list {
          display: flex;
          flex-direction: column;
          gap: 1rem;
        }

        .disk-item {
          padding: 1rem;
          background-color: var(--bg-card-alt);
          border-radius: 0.375rem;
        }

        .disk-header {
          display: flex;
          justify-content: space-between;
          margin-bottom: 0.5rem;
        }

        .disk-label {
          font-weight: 500;
        }

        .disk-path {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          font-family: monospace;
        }

        .disk-bar {
          height: 8px;
          background-color: var(--bg-progress);
          border-radius: 4px;
          overflow: hidden;
          margin-bottom: 0.5rem;
        }

        .disk-used {
          height: 100%;
          background-color: var(--color-primary);
        }

        .disk-stats {
          display: flex;
          justify-content: space-between;
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }
      </style>
    `;
  }

  handleCancelTask(id: string, taskType: string): void {
    if (taskType === 'scan') {
      this.cancelScanMutation.mutate(id);
    } else {
      this.cancelCommandMutation.mutate(id);
    }
  }

  private formatTaskName(task: RunningTask): string {
    if (task.taskType === 'scan') {
      return task.name;
    }
    return task.name.replace(/([A-Z])/g, ' $1').trim();
  }

  private formatElapsed(started: string): string {
    const startTime = new Date(started).getTime();
    const elapsed = Math.max(0, Date.now() - startTime);
    const seconds = Math.floor(elapsed / 1000);

    if (seconds < 60) {
      return `${seconds}s`;
    }

    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;

    if (minutes < 60) {
      return `${minutes}m ${remainingSeconds}s`;
    }

    const hours = Math.floor(minutes / 60);
    const remainingMinutes = minutes % 60;
    return `${hours}h ${remainingMinutes}m`;
  }

  private formatStage(stage: string): string {
    switch (stage) {
      case 'scanning': return 'Discovering';
      case 'probing': return 'Probing';
      case 'hashing': return 'Hashing';
      default: return stage;
    }
  }

  private truncateFilename(filename: string, maxLen: number): string {
    if (filename.length <= maxLen) return filename;
    const ext = filename.lastIndexOf('.');
    if (ext > 0 && filename.length - ext <= 6) {
      const extStr = filename.substring(ext);
      const nameLen = maxLen - extStr.length - 3;
      if (nameLen > 0) {
        return `${filename.substring(0, nameLen)}...${extStr}`;
      }
    }
    return `${filename.substring(0, maxLen - 3)}...`;
  }

  private formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return `${parseFloat((bytes / k ** i).toFixed(1))} ${sizes[i]}`;
  }
}
