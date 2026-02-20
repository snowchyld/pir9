/**
 * Provider edit dialog - for configuring a provider's settings
 * Renders dynamic form fields based on the schema
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { signal } from '../../core/reactive';
import type { ProviderSchema, ProviderField } from './provider-types';

export interface ProviderEditDialogConfig {
  title: string;
  apiEndpoint: string; // e.g., '/downloadclient'
  schema: ProviderSchema;
  existingId?: number; // If editing existing
  onSave: () => void;
  onClose: () => void;
  extraFields?: ExtraFieldConfig[]; // Additional fields beyond schema
}

export interface ExtraFieldConfig {
  section: string;
  fields: {
    name: string;
    label: string;
    type: 'checkbox' | 'number' | 'text';
    value: unknown;
    helpText?: string;
    min?: number;
    max?: number;
  }[];
}

@customElement('provider-edit-dialog')
export class ProviderEditDialog extends BaseComponent {
  private config = signal<ProviderEditDialogConfig | null>(null);
  private formData = signal<Record<string, unknown>>({});
  private isSaving = signal(false);
  private isTesting = signal(false);
  private testResult = signal<{ success: boolean; message: string } | null>(null);
  private errors = signal<string[]>([]);

  protected onInit(): void {
    this.watch(this.config);
    this.watch(this.formData);
    this.watch(this.isSaving);
    this.watch(this.isTesting);
    this.watch(this.testResult);
    this.watch(this.errors);
  }

  open(config: ProviderEditDialogConfig): void {
    this.config.set(config);
    this.testResult.set(null);
    this.errors.set([]);

    // Initialize form data from schema
    const data: Record<string, unknown> = {
      name: config.schema.name || config.schema.implementationName,
      enable: config.schema.enable ?? true,
      implementation: config.schema.implementation,
      implementationName: config.schema.implementationName,
      configContract: config.schema.configContract,
      tags: config.schema.tags || [],
    };

    // Add protocol-specific fields
    if ('protocol' in config.schema) {
      data.protocol = config.schema.protocol;
    }
    if ('priority' in config.schema) {
      data.priority = config.schema.priority ?? 1;
    }
    if ('removeCompletedDownloads' in config.schema) {
      data.removeCompletedDownloads = config.schema.removeCompletedDownloads ?? true;
    }
    if ('removeFailedDownloads' in config.schema) {
      data.removeFailedDownloads = config.schema.removeFailedDownloads ?? true;
    }

    // Initialize field values
    config.schema.fields.forEach((field) => {
      data[field.name] = field.value;
    });

    this.formData.set(data);
  }

  close(): void {
    this.config.value?.onClose();
    this.config.set(null);
    this.formData.set({});
    this.testResult.set(null);
    this.errors.set([]);
  }

  private updateField(name: string, value: unknown): void {
    const current = this.formData.value;
    this.formData.set({ ...current, [name]: value });
    this.testResult.set(null); // Clear test result on change
  }

  private async handleTest(): Promise<void> {
    const config = this.config.value;
    if (!config) return;

    this.isTesting.set(true);
    this.testResult.set(null);

    try {
      const payload = this.buildPayload();
      const response = await fetch(`/api/v3${config.apiEndpoint}/test`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        this.testResult.set({
          success: true,
          message: result.message || 'Connection successful!'
        });
      } else {
        this.testResult.set({
          success: false,
          message: result.message || 'Connection test failed'
        });
      }
    } catch (err) {
      this.testResult.set({
        success: false,
        message: err instanceof Error ? err.message : 'Test failed'
      });
    } finally {
      this.isTesting.set(false);
    }
  }

  private async handleSave(): Promise<void> {
    const config = this.config.value;
    if (!config) return;

    this.isSaving.set(true);
    this.errors.set([]);

    try {
      const payload = this.buildPayload();
      const method = config.existingId ? 'PUT' : 'POST';
      const url = config.existingId
        ? `/api/v3${config.apiEndpoint}/${config.existingId}`
        : `/api/v3${config.apiEndpoint}`;

      const response = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to save');
      }

      config.onSave();
      this.close();
    } catch (err) {
      this.errors.set([err instanceof Error ? err.message : 'Failed to save']);
    } finally {
      this.isSaving.set(false);
    }
  }

  private buildPayload(): Record<string, unknown> {
    const config = this.config.value;
    if (!config) return {};

    const data = this.formData.value;

    // Build fields array from form data
    const fields = config.schema.fields.map((field) => ({
      name: field.name,
      value: data[field.name],
    }));

    return {
      id: config.existingId || 0,
      name: data.name,
      enable: data.enable,
      implementation: data.implementation,
      implementationName: data.implementationName,
      configContract: data.configContract,
      fields,
      tags: data.tags || [],
      protocol: data.protocol,
      priority: data.priority,
      removeCompletedDownloads: data.removeCompletedDownloads,
      removeFailedDownloads: data.removeFailedDownloads,
    };
  }

  protected template(): string {
    const config = this.config.value;
    if (!config) return '';

    const data = this.formData.value;
    const isSaving = this.isSaving.value;
    const isTesting = this.isTesting.value;
    const testResult = this.testResult.value;
    const errors = this.errors.value;

    return html`
      <div class="dialog-backdrop" onclick="this.querySelector('provider-edit-dialog').handleBackdropClick(event)">
        <div class="dialog" role="dialog" aria-modal="true">
          <div class="dialog-header">
            <h2>${config.existingId ? 'Edit' : 'Add'} ${escapeHtml(config.schema.implementationName)}</h2>
            <button class="close-btn" onclick="this.closest('provider-edit-dialog').close()" aria-label="Close">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="dialog-body">
            ${errors.length > 0 ? html`
              <div class="error-box">
                ${errors.map((e) => html`<p>${escapeHtml(e)}</p>`).join('')}
              </div>
            ` : ''}

            ${testResult ? html`
              <div class="test-result ${testResult.success ? 'success' : 'error'}">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  ${testResult.success
                    ? '<polyline points="20 6 9 17 4 12"></polyline>'
                    : '<line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line>'}
                </svg>
                <span>${escapeHtml(testResult.message)}</span>
              </div>
            ` : ''}

            <form class="provider-form" onsubmit="event.preventDefault()">
              <!-- Name field -->
              <div class="form-group">
                <label for="name">Name</label>
                <input
                  type="text"
                  id="name"
                  value="${escapeHtml(String(data.name || ''))}"
                  onchange="this.closest('provider-edit-dialog').handleFieldChange('name', this.value)"
                />
              </div>

              <!-- Enable field -->
              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.enable ? 'checked' : ''}
                    onchange="this.closest('provider-edit-dialog').handleFieldChange('enable', this.checked)"
                  />
                  <span>Enable</span>
                </label>
              </div>

              <!-- Dynamic fields from schema -->
              ${config.schema.fields
                .filter((f) => f.hidden !== 'hidden')
                .sort((a, b) => a.order - b.order)
                .map((field) => this.renderField(field, data))
                .join('')}

              <!-- Priority (if applicable) -->
              ${'priority' in data ? html`
                <div class="form-group">
                  <label for="priority">Priority</label>
                  <input
                    type="number"
                    id="priority"
                    min="1"
                    max="50"
                    value="${data.priority}"
                    onchange="this.closest('provider-edit-dialog').handleFieldChange('priority', parseInt(this.value))"
                  />
                  <p class="help-text">Lower values are higher priority</p>
                </div>
              ` : ''}

              <!-- Download handling options (for download clients) -->
              ${'removeCompletedDownloads' in data ? html`
                <fieldset class="form-fieldset">
                  <legend>Completed Download Handling</legend>

                  <div class="form-group form-group-checkbox">
                    <label>
                      <input
                        type="checkbox"
                        ${data.removeCompletedDownloads ? 'checked' : ''}
                        onchange="this.closest('provider-edit-dialog').handleFieldChange('removeCompletedDownloads', this.checked)"
                      />
                      <span>Remove Completed</span>
                    </label>
                    <p class="help-text">Remove imported downloads from download client history</p>
                  </div>

                  <div class="form-group form-group-checkbox">
                    <label>
                      <input
                        type="checkbox"
                        ${data.removeFailedDownloads ? 'checked' : ''}
                        onchange="this.closest('provider-edit-dialog').handleFieldChange('removeFailedDownloads', this.checked)"
                      />
                      <span>Remove Failed</span>
                    </label>
                    <p class="help-text">Remove failed downloads from download client history</p>
                  </div>
                </fieldset>
              ` : ''}
            </form>
          </div>

          <div class="dialog-footer">
            <button
              class="btn btn-default"
              onclick="this.closest('provider-edit-dialog').handleTest()"
              ${isTesting ? 'disabled' : ''}
            >
              ${isTesting ? html`
                <span class="btn-spinner"></span>
                Testing...
              ` : 'Test'}
            </button>

            <div class="footer-spacer"></div>

            <button class="btn btn-secondary" onclick="this.closest('provider-edit-dialog').close()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('provider-edit-dialog').handleSave()"
              ${isSaving ? 'disabled' : ''}
            >
              ${isSaving ? html`
                <span class="btn-spinner"></span>
                Saving...
              ` : 'Save'}
            </button>
          </div>
        </div>
      </div>

      ${safeHtml(this.styles())}
    `;
  }

  private renderField(field: ProviderField, data: Record<string, unknown>): string {
    const value = data[field.name];
    const fieldId = `field-${field.name}`;

    switch (field.type) {
      case 'textbox':
      case 'url':
      case 'path':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input
              type="text"
              id="${fieldId}"
              value="${escapeHtml(String(value ?? ''))}"
              placeholder="${escapeHtml(field.placeholder || '')}"
              onchange="this.closest('provider-edit-dialog').handleFieldChange('${field.name}', this.value)"
            />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'password':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input
              type="password"
              id="${fieldId}"
              value="${escapeHtml(String(value ?? ''))}"
              placeholder="${escapeHtml(field.placeholder || '')}"
              onchange="this.closest('provider-edit-dialog').handleFieldChange('${field.name}', this.value)"
            />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'number':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input
              type="number"
              id="${fieldId}"
              value="${value ?? ''}"
              onchange="this.closest('provider-edit-dialog').handleFieldChange('${field.name}', ${field.isFloat ? 'parseFloat(this.value)' : 'parseInt(this.value)'})"
            />
            ${field.unit ? html`<span class="field-unit">${escapeHtml(field.unit)}</span>` : ''}
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'checkbox':
        return html`
          <div class="form-group form-group-checkbox">
            <label>
              <input
                type="checkbox"
                ${value ? 'checked' : ''}
                onchange="this.closest('provider-edit-dialog').handleFieldChange('${field.name}', this.checked)"
              />
              <span>${escapeHtml(field.label)}</span>
            </label>
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'select':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <select
              id="${fieldId}"
              onchange="this.closest('provider-edit-dialog').handleFieldChange('${field.name}', this.value)"
            >
              ${(field.selectOptions || []).map((opt) => html`
                <option value="${opt.value}" ${String(value) === String(opt.value) ? 'selected' : ''}>
                  ${escapeHtml(opt.name)}
                </option>
              `).join('')}
            </select>
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      default:
        // Fallback to text input
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input
              type="text"
              id="${fieldId}"
              value="${escapeHtml(String(value ?? ''))}"
              onchange="this.closest('provider-edit-dialog').handleFieldChange('${field.name}', this.value)"
            />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;
    }
  }

  handleFieldChange(name: string, value: unknown): void {
    this.updateField(name, value);
  }

  handleBackdropClick(e: Event): void {
    if ((e.target as HTMLElement).classList.contains('dialog-backdrop')) {
      this.close();
    }
  }

  private styles(): string {
    return `<style>
      provider-edit-dialog {
        display: contents;
      }

      .dialog-backdrop {
        position: fixed;
        inset: 0;
        background-color: rgba(0, 0, 0, 0.6);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
        padding: 1rem;
      }

      .dialog {
        background-color: var(--bg-card);
        border: 1px solid var(--border-color);
        border-radius: 0.5rem;
        width: 100%;
        max-width: 500px;
        max-height: 90vh;
        display: flex;
        flex-direction: column;
        box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
      }

      .dialog-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 1rem 1.5rem;
        border-bottom: 1px solid var(--border-color);
      }

      .dialog-header h2 {
        margin: 0;
        font-size: 1.125rem;
        font-weight: 600;
      }

      .close-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 0.25rem;
        background: transparent;
        border: none;
        border-radius: 0.25rem;
        color: var(--text-color-muted);
        cursor: pointer;
      }

      .close-btn:hover {
        color: var(--text-color);
        background-color: var(--bg-input-hover);
      }

      .dialog-body {
        flex: 1;
        overflow-y: auto;
        padding: 1.5rem;
      }

      .dialog-footer {
        display: flex;
        align-items: center;
        gap: 0.75rem;
        padding: 1rem 1.5rem;
        border-top: 1px solid var(--border-color);
      }

      .footer-spacer {
        flex: 1;
      }

      .error-box {
        padding: 0.75rem 1rem;
        background-color: rgba(240, 80, 80, 0.1);
        border: 1px solid rgba(240, 80, 80, 0.3);
        border-radius: 0.375rem;
        color: var(--color-danger);
        margin-bottom: 1rem;
      }

      .error-box p {
        margin: 0;
        font-size: 0.875rem;
      }

      .test-result {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.75rem 1rem;
        border-radius: 0.375rem;
        margin-bottom: 1rem;
        font-size: 0.875rem;
      }

      .test-result.success {
        background-color: rgba(39, 174, 96, 0.1);
        border: 1px solid rgba(39, 174, 96, 0.3);
        color: var(--color-success);
      }

      .test-result.error {
        background-color: rgba(240, 80, 80, 0.1);
        border: 1px solid rgba(240, 80, 80, 0.3);
        color: var(--color-danger);
      }

      .provider-form {
        display: flex;
        flex-direction: column;
        gap: 1rem;
      }

      .form-group {
        display: flex;
        flex-direction: column;
        gap: 0.375rem;
      }

      .form-group label {
        font-size: 0.875rem;
        font-weight: 500;
        color: var(--text-color);
      }

      .form-group input[type="text"],
      .form-group input[type="password"],
      .form-group input[type="number"],
      .form-group select {
        padding: 0.5rem 0.75rem;
        background-color: var(--bg-input);
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        color: var(--text-color);
        font-size: 0.875rem;
      }

      .form-group input:focus,
      .form-group select:focus {
        outline: none;
        border-color: var(--color-primary);
        box-shadow: 0 0 0 2px rgba(93, 156, 236, 0.2);
      }

      .form-group-checkbox {
        flex-direction: row;
        align-items: flex-start;
      }

      .form-group-checkbox label {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        cursor: pointer;
        font-weight: 400;
      }

      .form-group-checkbox input[type="checkbox"] {
        width: 1rem;
        height: 1rem;
        accent-color: var(--color-primary);
      }

      .help-text {
        font-size: 0.75rem;
        color: var(--text-color-muted);
        margin: 0.25rem 0 0;
      }

      .field-unit {
        font-size: 0.75rem;
        color: var(--text-color-muted);
      }

      .form-fieldset {
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        padding: 1rem;
        margin: 0.5rem 0;
      }

      .form-fieldset legend {
        padding: 0 0.5rem;
        font-size: 0.875rem;
        font-weight: 500;
        color: var(--text-color-muted);
      }

      .btn {
        display: inline-flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.5rem 1rem;
        border-radius: 0.25rem;
        font-size: 0.875rem;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.15s ease;
      }

      .btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
      }

      .btn-default {
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        color: var(--text-color);
      }

      .btn-default:hover:not(:disabled) {
        background-color: var(--bg-input-hover);
      }

      .btn-secondary {
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        color: var(--text-color);
      }

      .btn-secondary:hover:not(:disabled) {
        background-color: var(--bg-input-hover);
      }

      .btn-primary {
        background-color: var(--btn-primary-bg);
        border: 1px solid var(--btn-primary-border);
        color: var(--color-white);
      }

      .btn-primary:hover:not(:disabled) {
        background-color: var(--btn-primary-bg-hover);
      }

      .btn-spinner {
        width: 14px;
        height: 14px;
        border: 2px solid currentColor;
        border-top-color: transparent;
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
      }

      @keyframes spin {
        to { transform: rotate(360deg); }
      }
    </style>`;
  }
}
