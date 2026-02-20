/**
 * System Tasks page
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { http } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';

interface ScheduledTask {
  id: number;
  name: string;
  taskName: string;
  interval: number;
  lastExecution: string;
  lastStartTime: string;
  nextExecution: string;
  lastDuration: string;
}

@customElement('system-tasks-page')
export class SystemTasksPage extends BaseComponent {
  private tasksQuery = createQuery({
    queryKey: ['/system/task'],
    queryFn: () => http.get<ScheduledTask[]>('/system/task'),
  });

  private runTaskMutation = createMutation({
    mutationFn: (taskName: string) =>
      http.post('/command', { name: taskName }),
    onSuccess: () => {
      invalidateQueries(['/system/task']);
      showSuccess('Task started');
    },
    onError: () => {
      showError('Failed to start task');
    },
  });

  protected onInit(): void {
    this.watch(this.tasksQuery.data);
    this.watch(this.tasksQuery.isLoading);
  }

  protected template(): string {
    const tasks = this.tasksQuery.data.value ?? [];
    const isLoading = this.tasksQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="tasks-page">
        <div class="toolbar">
          <h1 class="page-title">Scheduled Tasks</h1>
          <button
            class="refresh-btn"
            onclick="this.closest('system-tasks-page').handleRefresh()"
            title="Refresh"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="23 4 23 10 17 10"></polyline>
              <polyline points="1 20 1 14 7 14"></polyline>
              <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
            </svg>
          </button>
        </div>

        <div class="tasks-section">
          <table class="tasks-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Interval</th>
                <th>Last Execution</th>
                <th>Last Duration</th>
                <th>Next Execution</th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              ${tasks.map((task) => html`
                <tr>
                  <td class="task-name">${escapeHtml(task.name)}</td>
                  <td>${this.formatInterval(task.interval)}</td>
                  <td class="date-cell">
                    ${task.lastExecution ? this.formatDate(new Date(task.lastExecution)) : 'Never'}
                  </td>
                  <td>${escapeHtml(task.lastDuration || '-')}</td>
                  <td class="date-cell">
                    ${task.nextExecution ? this.formatDate(new Date(task.nextExecution)) : '-'}
                  </td>
                  <td>
                    <button
                      class="run-btn"
                      onclick="this.closest('system-tasks-page').handleRunTask('${task.taskName}')"
                      title="Run now"
                    >
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <polygon points="5 3 19 12 5 21 5 3"></polygon>
                      </svg>
                    </button>
                  </td>
                </tr>
              `).join('')}
            </tbody>
          </table>
        </div>
      </div>

      <style>
        .tasks-page {
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

        .tasks-section {
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .tasks-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .tasks-table th,
        .tasks-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .tasks-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          background-color: var(--bg-card-alt);
        }

        .task-name {
          font-weight: 500;
        }

        .date-cell {
          white-space: nowrap;
          color: var(--text-color-muted);
        }

        .run-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          background: transparent;
          border: none;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          cursor: pointer;
        }

        .run-btn:hover {
          color: var(--color-primary);
          background-color: var(--bg-input-hover);
        }
      </style>
    `;
  }

  private formatInterval(minutes: number): string {
    if (minutes < 60) {
      return `${minutes} min`;
    } else if (minutes < 1440) {
      return `${Math.floor(minutes / 60)} hr`;
    } else {
      return `${Math.floor(minutes / 1440)} day${minutes >= 2880 ? 's' : ''}`;
    }
  }

  private formatDate(date: Date): string {
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 0) {
      // Future date
      const futureDiff = -diff;
      if (futureDiff < 60000) return 'in < 1 min';
      if (futureDiff < 3600000) return `in ${Math.floor(futureDiff / 60000)} min`;
      if (futureDiff < 86400000) return `in ${Math.floor(futureDiff / 3600000)} hr`;
      return date.toLocaleDateString();
    }

    if (diff < 60000) return '< 1 min ago';
    if (diff < 3600000) return `${Math.floor(diff / 60000)} min ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)} hr ago`;
    return date.toLocaleDateString();
  }

  handleRefresh(): void {
    this.tasksQuery.refetch();
  }

  handleRunTask(taskName: string): void {
    this.runTaskMutation.mutate(taskName);
  }
}
