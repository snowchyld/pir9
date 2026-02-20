/**
 * Quality Settings page - Quality definitions
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';
import { createQuery } from '../../core/query';
import { http } from '../../core/http';
import { showSuccess } from '../../stores/app.store';

interface QualityDefinition {
  id: number;
  quality: {
    id: number;
    name: string;
    source: string;
    resolution: number;
  };
  title: string;
  weight: number;
  minSize: number;
  maxSize: number;
  preferredSize: number;
}

@customElement('quality-settings')
export class QualitySettings extends BaseComponent {
  private qualityQuery = createQuery({
    queryKey: ['/qualitydefinition'],
    queryFn: () => http.get<QualityDefinition[]>('/qualitydefinition'),
  });

  protected onInit(): void {
    this.watch(this.qualityQuery.data);
    this.watch(this.qualityQuery.isLoading);
  }

  protected template(): string {
    const qualities = this.qualityQuery.data.value ?? [];
    const isLoading = this.qualityQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="settings-section">
        <h2 class="section-title">Quality Definitions</h2>
        <p class="section-description">
          Configure the size limits for each quality. Sizes are in megabytes per minute.
        </p>

        <table class="quality-table">
          <thead>
            <tr>
              <th>Quality</th>
              <th>Title</th>
              <th>Min Size</th>
              <th>Preferred Size</th>
              <th>Max Size</th>
            </tr>
          </thead>
          <tbody>
            ${qualities.map((q) => html`
              <tr>
                <td>
                  <span class="quality-name">${escapeHtml(q.quality.name)}</span>
                </td>
                <td>
                  <input
                    type="text"
                    class="form-input small"
                    value="${escapeHtml(q.title)}"
                  />
                </td>
                <td>
                  <input
                    type="number"
                    class="form-input small"
                    value="${q.minSize}"
                    step="0.1"
                  />
                </td>
                <td>
                  <input
                    type="number"
                    class="form-input small"
                    value="${q.preferredSize}"
                    step="0.1"
                  />
                </td>
                <td>
                  <input
                    type="number"
                    class="form-input small"
                    value="${q.maxSize}"
                    step="0.1"
                  />
                </td>
              </tr>
            `).join('')}
          </tbody>
        </table>
      </div>

      <div class="actions">
        <button class="save-btn" onclick="this.closest('quality-settings').handleSave()">
          Save Changes
        </button>
      </div>

      <style>
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

        .settings-section {
          margin-bottom: 2rem;
          padding: 1.5rem;
          background-color: var(--bg-card);
          border: 1px solid var(--border-color);
          border-radius: 0.5rem;
        }

        .section-title {
          font-size: 1.125rem;
          font-weight: 600;
          margin: 0 0 0.5rem 0;
        }

        .section-description {
          font-size: 0.875rem;
          color: var(--text-color-muted);
          margin: 0 0 1.5rem 0;
        }

        .quality-table {
          width: 100%;
          border-collapse: collapse;
          font-size: 0.875rem;
        }

        .quality-table th,
        .quality-table td {
          padding: 0.75rem;
          text-align: left;
          border-bottom: 1px solid var(--border-color);
        }

        .quality-table th {
          font-weight: 600;
          color: var(--text-color-muted);
          background-color: var(--bg-card-alt);
        }

        .quality-name {
          font-weight: 500;
        }

        .form-input {
          padding: 0.375rem 0.5rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .form-input.small {
          width: 100px;
        }

        .form-input:focus {
          outline: none;
          border-color: var(--color-primary);
        }

        .actions {
          margin-top: 1.5rem;
        }

        .save-btn {
          padding: 0.625rem 1.25rem;
          background-color: var(--btn-primary-bg);
          border: 1px solid var(--btn-primary-border);
          border-radius: 0.25rem;
          color: var(--color-white);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
        }

        .save-btn:hover {
          background-color: var(--btn-primary-bg-hover);
        }
      </style>
    `;
  }

  handleSave(): void {
    showSuccess('Settings saved');
  }
}
