/**
 * System Logs page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createQuery } from '../../core/query';
import { signal } from '../../core/reactive';

interface LogFile {
  id: number;
  filename: string;
  lastWriteTime: string;
  contentsUrl: string;
  downloadUrl: string;
}

interface LogEntry {
  time: string;
  level: 'trace' | 'debug' | 'info' | 'warn' | 'error' | 'fatal';
  logger: string;
  message: string;
  exception?: string;
}

interface LogResponse {
  page: number;
  pageSize: number;
  totalRecords: number;
  records: LogEntry[];
}

@customElement('system-logs-page')
export class SystemLogsPage extends BaseComponent {
  private selectedLevel = signal<string>('all');

  private logsQuery = createQuery({
    queryKey: ['/log'],
    queryFn: () =>
      http.get<LogResponse>('/log', {
        params: { pageSize: 100 },
      }),
  });

  private logFilesQuery = createQuery({
    queryKey: ['/log/file'],
    queryFn: () => http.get<LogFile[]>('/log/file'),
  });

  protected onInit(): void {
    this.watch(this.logsQuery.data);
    this.watch(this.logsQuery.isLoading);
    this.watch(this.logFilesQuery.data);
    this.watch(this.selectedLevel);
  }

  protected template(): string {
    const response = this.logsQuery.data.value;
    const logs = response?.records ?? [];
    const logFiles = this.logFilesQuery.data.value ?? [];
    const isLoading = this.logsQuery.isLoading.value;
    const level = this.selectedLevel.value;

    const filteredLogs = level === 'all' ? logs : logs.filter((log) => log.level === level);

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="logs-page">
        <div class="toolbar">
          <h1 class="page-title">Logs</h1>
          <div class="toolbar-right">
            <select
              class="level-select"
              onchange="this.closest('system-logs-page').handleLevelChange(this.value)"
            >
              <option value="all" ${level === 'all' ? 'selected' : ''}>All Levels</option>
              <option value="trace" ${level === 'trace' ? 'selected' : ''}>Trace</option>
              <option value="debug" ${level === 'debug' ? 'selected' : ''}>Debug</option>
              <option value="info" ${level === 'info' ? 'selected' : ''}>Info</option>
              <option value="warn" ${level === 'warn' ? 'selected' : ''}>Warn</option>
              <option value="error" ${level === 'error' ? 'selected' : ''}>Error</option>
              <option value="fatal" ${level === 'fatal' ? 'selected' : ''}>Fatal</option>
            </select>
            <button
              class="refresh-btn"
              onclick="this.closest('system-logs-page').handleRefresh()"
              title="Refresh"
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="23 4 23 10 17 10"></polyline>
                <polyline points="1 20 1 14 7 14"></polyline>
                <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
              </svg>
            </button>
          </div>
        </div>

        <div class="logs-section">
          <div class="log-entries">
            ${
              filteredLogs.length === 0
                ? html`
              <div class="empty-state">No log entries found</div>
            `
                : filteredLogs
                    .map(
                      (log) => html`
              <div class="log-entry ${log.level}">
                <div class="log-header">
                  <span class="log-level ${log.level}">${log.level.toUpperCase()}</span>
                  <span class="log-time">${new Date(log.time).toLocaleTimeString()}</span>
                  <span class="log-logger">${escapeHtml(log.logger)}</span>
                </div>
                <div class="log-message">${escapeHtml(log.message)}</div>
                ${
                  log.exception
                    ? html`
                  <pre class="log-exception">${escapeHtml(log.exception)}</pre>
                `
                    : ''
                }
              </div>
            `,
                    )
                    .join('')
            }
          </div>
        </div>

        ${
          logFiles.length > 0
            ? html`
          <div class="files-section">
            <h2 class="section-title">Log Files</h2>
            <div class="files-list">
              ${logFiles
                .map(
                  (file) => html`
                <div class="file-item">
                  <span class="file-name">${escapeHtml(file.filename)}</span>
                  <span class="file-date">${new Date(file.lastWriteTime).toLocaleString()}</span>
                  <a
                    class="download-btn"
                    href="${file.downloadUrl}"
                    download
                    title="Download"
                  >
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                      <polyline points="7 10 12 15 17 10"></polyline>
                      <line x1="12" y1="15" x2="12" y2="3"></line>
                    </svg>
                  </a>
                </div>
              `,
                )
                .join('')}
            </div>
          </div>
        `
            : ''
        }
      </div>

      <style>
        .logs-page {
          display: flex;
          flex-direction: column;
          gap: 1.5rem;
        }

        .toolbar {
          display: flex;
          align-items: center;
          justify-content: space-between;
        }

        .page-title {
          font-size: 1.5rem;
          font-weight: 600;
          margin: 0;
        }

        .toolbar-right {
          display: flex;
          gap: 0.5rem;
        }

        .level-select {
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .refresh-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.5rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          cursor: pointer;
        }

        .refresh-btn:hover {
          background-color: var(--btn-default-bg-hover);
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

        .logs-section, .files-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1rem;
          font-weight: 600;
          margin: 0 0 1rem 0;
        }

        .log-entries {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
          max-height: 600px;
          overflow-y: auto;
        }

        .empty-state {
          padding: 2rem;
          text-align: center;
          color: var(--text-color-muted);
        }

        .log-entry {
          padding: 0.75rem;
          background-color: var(--bg-card-alt);
          border-radius: 0.375rem;
          border-left: 3px solid;
        }

        .log-entry.trace { border-left-color: var(--text-color-muted); }
        .log-entry.debug { border-left-color: var(--text-color-muted); }
        .log-entry.info { border-left-color: var(--color-primary); }
        .log-entry.warn { border-left-color: var(--color-warning); }
        .log-entry.error { border-left-color: var(--color-danger); }
        .log-entry.fatal { border-left-color: var(--color-danger); }

        .log-header {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          margin-bottom: 0.5rem;
          font-size: 0.75rem;
        }

        .log-level {
          padding: 0.125rem 0.375rem;
          border-radius: 0.25rem;
          font-weight: 600;
          text-transform: uppercase;
        }

        .log-level.trace, .log-level.debug { background-color: var(--bg-card); color: var(--text-color-muted); }
        .log-level.info { background-color: var(--color-primary); color: var(--color-white); }
        .log-level.warn { background-color: var(--color-warning); color: var(--color-white); }
        .log-level.error, .log-level.fatal { background-color: var(--color-danger); color: var(--color-white); }

        .log-time {
          color: var(--text-color-muted);
        }

        .log-logger {
          color: var(--text-color-muted);
        }

        .log-message {
          font-size: 0.875rem;
          word-break: break-word;
        }

        .log-exception {
          margin: 0.5rem 0 0 0;
          padding: 0.5rem;
          font-size: 0.75rem;
          background-color: var(--bg-card);
          border-radius: 0.25rem;
          overflow-x: auto;
          color: var(--color-danger);
        }

        .files-list {
          display: flex;
          flex-direction: column;
          gap: 0.5rem;
        }

        .file-item {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem;
          background-color: var(--bg-card-alt);
          border-radius: 0.375rem;
        }

        .file-name {
          flex: 1;
          font-family: monospace;
          font-size: 0.875rem;
        }

        .file-date {
          font-size: 0.75rem;
          color: var(--text-color-muted);
        }

        .download-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          color: var(--text-color-muted);
          text-decoration: none;
        }

        .download-btn:hover {
          color: var(--color-primary);
        }
      </style>
    `;
  }

  handleLevelChange(level: string): void {
    this.selectedLevel.set(level);
  }

  handleRefresh(): void {
    this.logsQuery.refetch();
  }
}
