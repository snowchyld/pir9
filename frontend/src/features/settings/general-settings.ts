/**
 * General Settings page
 */

import { BaseComponent, customElement, escapeHtml, html } from '../../core/component';
import { http } from '../../core/http';
import { createQuery } from '../../core/query';
import { showSuccess } from '../../stores/app.store';

interface HostConfig {
  bindAddress: string;
  port: number;
  urlBase: string;
  instanceName: string;
  applicationUrl: string;
  enableSsl: boolean;
  sslPort: number;
  sslCertPath: string;
  sslCertPassword: string;
  launchBrowser: boolean;
}

@customElement('general-settings')
export class GeneralSettings extends BaseComponent {
  private hostQuery = createQuery({
    queryKey: ['/config/host'],
    queryFn: () => http.get<HostConfig>('/config/host'),
  });

  protected onInit(): void {
    this.watch(this.hostQuery.data);
    this.watch(this.hostQuery.isLoading);
    this.watch(this.hostQuery.isError);
  }

  protected template(): string {
    const host = this.hostQuery.data.value;
    const isLoading = this.hostQuery.isLoading.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="settings-section">
        <h2 class="section-title">Host</h2>

        <div class="form-group">
          <label class="form-label">Bind Address</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(host?.bindAddress ?? '*')}"
            placeholder="*"
          />
          <p class="form-hint">Valid IP address or '*' for all interfaces</p>
        </div>

        <div class="form-group">
          <label class="form-label">Port Number</label>
          <input
            type="number"
            class="form-input"
            value="${host?.port ?? 8989}"
          />
        </div>

        <div class="form-group">
          <label class="form-label">URL Base</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(host?.urlBase ?? '')}"
            placeholder="/pir9"
          />
          <p class="form-hint">For reverse proxy support, default is empty</p>
        </div>

        <div class="form-group">
          <label class="form-label">Instance Name</label>
          <input
            type="text"
            class="form-input"
            value="${escapeHtml(host?.instanceName ?? 'pir9')}"
          />
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input
              type="checkbox"
              ${host?.enableSsl ? 'checked' : ''}
            />
            <span>Enable SSL</span>
          </label>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Security</h2>

        <div class="form-group">
          <label class="form-label">Authentication</label>
          <select class="form-select">
            <option value="none">None</option>
            <option value="basic">Basic (Browser popup)</option>
            <option value="forms">Forms (Login page)</option>
            <option value="external">External</option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label">API Key</label>
          <div class="api-key-group">
            <input
              type="text"
              class="form-input"
              value="••••••••••••••••••••••••••••••••"
              readonly
            />
            <button class="copy-btn" onclick="this.closest('general-settings').handleCopyApiKey()">
              Copy
            </button>
            <button class="regenerate-btn" onclick="this.closest('general-settings').handleRegenerateApiKey()">
              Regenerate
            </button>
          </div>
        </div>

        <div class="form-group">
          <label class="form-label">Certificate Validation</label>
          <select class="form-select">
            <option value="enabled">Enabled</option>
            <option value="disabledForLocalAddresses">Disabled for Local Addresses</option>
            <option value="disabled">Disabled</option>
          </select>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Proxy</h2>

        <div class="form-group">
          <label class="checkbox-label">
            <input type="checkbox" />
            <span>Use Proxy</span>
          </label>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Logging</h2>

        <div class="form-group">
          <label class="form-label">Log Level</label>
          <select class="form-select">
            <option value="info">Info</option>
            <option value="debug">Debug</option>
            <option value="trace">Trace</option>
          </select>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input type="checkbox" checked />
            <span>Send Anonymous Usage Data</span>
          </label>
          <p class="form-hint">Help improve pir9 by sending anonymous usage and error information</p>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Updates</h2>

        <div class="form-group">
          <label class="form-label">Branch</label>
          <select class="form-select">
            <option value="main">main - Stable</option>
            <option value="develop">develop - Beta</option>
            <option value="nightly">nightly - Unstable</option>
          </select>
        </div>

        <div class="form-group">
          <label class="checkbox-label">
            <input type="checkbox" checked />
            <span>Automatic Updates</span>
          </label>
        </div>
      </div>

      <div class="settings-section">
        <h2 class="section-title">Backup</h2>

        <div class="form-group">
          <label class="form-label">Backup Folder</label>
          <input
            type="text"
            class="form-input"
            value="/config/Backups"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Backup Interval (days)</label>
          <input
            type="number"
            class="form-input"
            value="7"
          />
        </div>

        <div class="form-group">
          <label class="form-label">Backup Retention (days)</label>
          <input
            type="number"
            class="form-input"
            value="28"
          />
        </div>
      </div>

      <div class="actions">
        <button class="save-btn" onclick="this.closest('general-settings').handleSave()">
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
          margin: 0 0 1.5rem 0;
          padding-bottom: 0.75rem;
          border-bottom: 1px solid var(--border-color);
        }

        .form-group {
          margin-bottom: 1.25rem;
        }

        .form-group:last-child {
          margin-bottom: 0;
        }

        .form-label {
          display: block;
          font-size: 0.875rem;
          font-weight: 500;
          margin-bottom: 0.5rem;
          color: var(--text-color);
        }

        .form-input, .form-select {
          width: 100%;
          max-width: 400px;
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--bg-input);
          border: 1px solid var(--border-input);
          border-radius: 0.25rem;
          color: var(--text-color);
        }

        .form-input:focus, .form-select:focus {
          outline: none;
          border-color: var(--color-primary);
        }

        .form-hint {
          font-size: 0.75rem;
          color: var(--text-color-muted);
          margin: 0.25rem 0 0 0;
        }

        .checkbox-label {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          cursor: pointer;
        }

        .checkbox-label input[type="checkbox"] {
          width: 16px;
          height: 16px;
          accent-color: var(--color-primary);
        }

        .api-key-group {
          display: flex;
          gap: 0.5rem;
          max-width: 500px;
        }

        .api-key-group .form-input {
          flex: 1;
          font-family: monospace;
        }

        .copy-btn, .regenerate-btn {
          padding: 0.5rem 0.75rem;
          font-size: 0.875rem;
          background-color: var(--btn-default-bg);
          border: 1px solid var(--btn-default-border);
          border-radius: 0.25rem;
          color: var(--text-color);
          cursor: pointer;
          white-space: nowrap;
        }

        .copy-btn:hover, .regenerate-btn:hover {
          background-color: var(--btn-default-bg-hover);
        }

        .regenerate-btn {
          background-color: var(--btn-danger-bg);
          border-color: var(--btn-danger-border);
          color: var(--color-white);
        }

        .regenerate-btn:hover {
          background-color: var(--btn-danger-bg-hover);
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

  handleCopyApiKey(): void {
    showSuccess('API key copied to clipboard');
  }

  handleRegenerateApiKey(): void {
    if (
      confirm(
        'Are you sure you want to regenerate the API key? All existing applications will need to be updated.',
      )
    ) {
      showSuccess('API key regenerated');
    }
  }
}
