/**
 * Indexers Settings page
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { httpV3 } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';
import { signal } from '../../core/reactive';
import type { IndexerSchema, ProviderField } from './provider-types';

interface Indexer {
  id: number;
  name: string;
  protocol: 'usenet' | 'torrent';
  enableRss: boolean;
  enableAutomaticSearch: boolean;
  enableInteractiveSearch: boolean;
  supportsRss: boolean;
  supportsSearch: boolean;
  priority: number;
  implementation: string;
  implementationName: string;
  configContract: string;
  fields: ProviderField[];
  tags: number[];
  downloadClientId: number;
}

type DialogMode = 'closed' | 'select' | 'edit';

@customElement('indexers-settings')
export class IndexersSettings extends BaseComponent {
  private indexersQuery = createQuery({
    queryKey: ['/indexer'],
    queryFn: () => httpV3.get<Indexer[]>('/indexer'),
  });

  private deleteMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/indexer/${id}`),
    onSuccess: () => {
      invalidateQueries(['/indexer']);
      showSuccess('Indexer deleted');
    },
    onError: () => {
      showError('Failed to delete indexer');
    },
  });

  // Dialog state
  private dialogMode = signal<DialogMode>('closed');
  private schemas = signal<IndexerSchema[]>([]);
  private schemasLoading = signal(false);
  private selectedSchema = signal<IndexerSchema | null>(null);
  private editingId = signal<number | null>(null);
  private formData = signal<Record<string, unknown>>({});
  private isSaving = signal(false);
  private isTesting = signal(false);
  private testResult = signal<{ success: boolean; message: string } | null>(null);

  protected onInit(): void {
    this.watch(this.indexersQuery.data);
    this.watch(this.indexersQuery.isLoading);
    this.watch(this.dialogMode);
    this.watch(this.schemas);
    this.watch(this.schemasLoading);
    this.watch(this.selectedSchema);
    this.watch(this.formData);
    this.watch(this.isSaving);
    this.watch(this.isTesting);
    this.watch(this.testResult);
  }

  protected template(): string {
    const indexers = this.indexersQuery.data.value ?? [];
    const isLoading = this.indexersQuery.isLoading.value;
    const mode = this.dialogMode.value;

    if (isLoading) {
      return html`
        <div class="loading-container">
          <div class="loading-spinner"></div>
        </div>
      `;
    }

    return html`
      <div class="settings-section">
        <div class="section-header">
          <h2 class="section-title">Indexers</h2>
          <button class="add-btn" onclick="this.closest('indexers-settings').handleAdd()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Indexer
          </button>
        </div>

        ${indexers.length === 0 ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <circle cx="11" cy="11" r="8"></circle>
              <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
            </svg>
            <p>No indexers configured</p>
            <p class="hint">Add an indexer to start searching for releases</p>
          </div>
        ` : html`
          <div class="indexers-list">
            ${indexers.map((indexer) => html`
              <div class="indexer-card">
                <div class="indexer-info">
                  <div class="indexer-name">${escapeHtml(indexer.name)}</div>
                  <div class="indexer-meta">
                    <span class="protocol-badge ${indexer.protocol}">${indexer.protocol}</span>
                    <span class="implementation">${escapeHtml(indexer.implementation)}</span>
                  </div>
                </div>
                <div class="indexer-features">
                  <span class="feature ${indexer.enableRss ? 'enabled' : 'disabled'}">RSS</span>
                  <span class="feature ${indexer.enableAutomaticSearch ? 'enabled' : 'disabled'}">Auto</span>
                  <span class="feature ${indexer.enableInteractiveSearch ? 'enabled' : 'disabled'}">Interactive</span>
                </div>
                <div class="indexer-actions">
                  <button class="action-btn" onclick="event.stopPropagation(); this.closest('indexers-settings').handleTest(${indexer.id})" title="Test">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polygon points="5 3 19 12 5 21 5 3"></polygon>
                    </svg>
                  </button>
                  <button class="action-btn" onclick="event.stopPropagation(); this.closest('indexers-settings').handleEdit(${indexer.id})" title="Edit">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
                  <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('indexers-settings').handleDelete(${indexer.id})" title="Delete">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="3 6 5 6 21 6"></polyline>
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                    </svg>
                  </button>
                </div>
              </div>
            `).join('')}
          </div>
        `}
      </div>

      ${mode === 'select' ? this.renderSelectDialog() : ''}
      ${mode === 'edit' ? this.renderEditDialog() : ''}

      ${safeHtml(this.styles())}
    `;
  }

  private renderSelectDialog(): string {
    const schemas = this.schemas.value;
    const loading = this.schemasLoading.value;

    const usenetSchemas = schemas.filter(s => s.protocol === 'usenet');
    const torrentSchemas = schemas.filter(s => s.protocol === 'torrent');

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('indexers-settings').closeDialog()">
        <div class="dialog">
          <div class="dialog-header">
            <h2>Add Indexer</h2>
            <button class="close-btn" onclick="this.closest('indexers-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            ${loading ? html`
              <div class="loading-center">
                <div class="spinner"></div>
                <p>Loading available indexers...</p>
              </div>
            ` : html`
              <div class="info-box">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="16" x2="12" y2="12"></line>
                  <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
                <span>Select an indexer to configure</span>
              </div>

              ${usenetSchemas.length > 0 ? html`
                <div class="group-header"><h3>Usenet</h3></div>
                <div class="provider-grid">
                  ${usenetSchemas.map((schema) => this.renderSchemaCard(schema)).join('')}
                </div>
              ` : ''}

              ${torrentSchemas.length > 0 ? html`
                <div class="group-header"><h3>Torrents</h3></div>
                <div class="provider-grid">
                  ${torrentSchemas.map((schema) => this.renderSchemaCard(schema)).join('')}
                </div>
              ` : ''}
            `}
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('indexers-settings').closeDialog()">
              Cancel
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderSchemaCard(schema: IndexerSchema): string {
    return html`
      <button class="provider-card" onclick="this.closest('indexers-settings').selectSchema('${escapeHtml(schema.implementation)}')">
        <div class="provider-icon">
          <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
        </div>
        <span class="provider-name">${escapeHtml(schema.implementationName)}</span>
      </button>
    `;
  }

  private renderEditDialog(): string {
    const schema = this.selectedSchema.value;
    if (!schema) return '';

    const data = this.formData.value;
    const isSaving = this.isSaving.value;
    const isTesting = this.isTesting.value;
    const testResult = this.testResult.value;
    const isEditing = this.editingId.value !== null;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('indexers-settings').closeDialog()">
        <div class="dialog dialog-form">
          <div class="dialog-header">
            <h2>${isEditing ? 'Edit' : 'Add'} ${escapeHtml(schema.implementationName)}</h2>
            <button class="close-btn" onclick="this.closest('indexers-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
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
              <div class="form-group">
                <label for="name">Name</label>
                <input
                  type="text"
                  id="name"
                  value="${escapeHtml(String(data.name || ''))}"
                  onchange="this.closest('indexers-settings').updateField('name', this.value)"
                />
              </div>

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.enableRss ? 'checked' : ''}
                    onchange="this.closest('indexers-settings').updateField('enableRss', this.checked)"
                  />
                  <span>Enable RSS</span>
                </label>
                <p class="help-text">Enable RSS feed monitoring for new releases</p>
              </div>

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.enableAutomaticSearch ? 'checked' : ''}
                    onchange="this.closest('indexers-settings').updateField('enableAutomaticSearch', this.checked)"
                  />
                  <span>Enable Automatic Search</span>
                </label>
                <p class="help-text">Enable automatic searching for missing episodes</p>
              </div>

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.enableInteractiveSearch ? 'checked' : ''}
                    onchange="this.closest('indexers-settings').updateField('enableInteractiveSearch', this.checked)"
                  />
                  <span>Enable Interactive Search</span>
                </label>
                <p class="help-text">Enable manual/interactive search</p>
              </div>

              <!-- Dynamic fields from schema -->
              ${schema.fields
                .filter(f => f.hidden !== 'hidden')
                .sort((a, b) => a.order - b.order)
                .map((field) => this.renderField(field, data))
                .join('')}

              <div class="form-group">
                <label for="priority">Priority</label>
                <input
                  type="number"
                  id="priority"
                  min="1"
                  max="50"
                  value="${data.priority ?? 25}"
                  onchange="this.closest('indexers-settings').updateField('priority', parseInt(this.value))"
                />
                <p class="help-text">Lower values are higher priority</p>
              </div>
            </form>
          </div>
          <div class="dialog-footer">
            <button
              class="btn btn-default"
              onclick="this.closest('indexers-settings').handleTestConnection()"
              ${isTesting ? 'disabled' : ''}
            >
              ${isTesting ? 'Testing...' : 'Test'}
            </button>
            <div class="footer-spacer"></div>
            <button class="btn btn-secondary" onclick="this.closest('indexers-settings').closeDialog()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('indexers-settings').handleSave()"
              ${isSaving ? 'disabled' : ''}
            >
              ${isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>
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
              onchange="this.closest('indexers-settings').updateField('${field.name}', this.value)"
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
              onchange="this.closest('indexers-settings').updateField('${field.name}', this.value)"
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
              onchange="this.closest('indexers-settings').updateField('${field.name}', ${field.isFloat ? 'parseFloat(this.value)' : 'parseInt(this.value)'})"
            />
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
                onchange="this.closest('indexers-settings').updateField('${field.name}', this.checked)"
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
              onchange="this.closest('indexers-settings').updateField('${field.name}', this.value)"
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
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input
              type="text"
              id="${fieldId}"
              value="${escapeHtml(String(value ?? ''))}"
              onchange="this.closest('indexers-settings').updateField('${field.name}', this.value)"
            />
          </div>
        `;
    }
  }

  // Public methods called from template
  async handleAdd(): Promise<void> {
    this.dialogMode.set('select');
    this.schemasLoading.set(true);

    try {
      const schemas = await httpV3.get<IndexerSchema[]>('/indexer/schema');
      this.schemas.set(schemas);
    } catch {
      showError('Failed to load indexer types');
      this.dialogMode.set('closed');
    } finally {
      this.schemasLoading.set(false);
    }
  }

  selectSchema(implementation: string): void {
    const schema = this.schemas.value.find(s => s.implementation === implementation);
    if (!schema) return;

    this.selectedSchema.set(schema);
    this.editingId.set(null);
    this.testResult.set(null);

    // Initialize form data
    const data: Record<string, unknown> = {
      name: schema.implementationName,
      enableRss: schema.supportsRss ?? true,
      enableAutomaticSearch: schema.supportsSearch ?? true,
      enableInteractiveSearch: schema.supportsSearch ?? true,
      priority: 25,
    };
    schema.fields.forEach(f => {
      data[f.name] = f.value;
    });
    this.formData.set(data);

    this.dialogMode.set('edit');
  }

  async handleEdit(id: number): Promise<void> {
    try {
      const indexer = await httpV3.get<Indexer>(`/indexer/${id}`);
      const schemas = await httpV3.get<IndexerSchema[]>('/indexer/schema');
      const schema = schemas.find(s => s.implementation === indexer.implementation);

      if (!schema) {
        showError('Unknown indexer type');
        return;
      }

      // Merge schema field definitions with indexer values
      const mergedSchema: IndexerSchema = {
        ...schema,
        fields: schema.fields.map(f => ({
          ...f,
          value: indexer.fields.find(cf => cf.name === f.name)?.value ?? f.value,
        })),
      };

      this.schemas.set(schemas);
      this.selectedSchema.set(mergedSchema);
      this.editingId.set(id);
      this.testResult.set(null);

      // Initialize form data
      const data: Record<string, unknown> = {
        name: indexer.name,
        enableRss: indexer.enableRss,
        enableAutomaticSearch: indexer.enableAutomaticSearch,
        enableInteractiveSearch: indexer.enableInteractiveSearch,
        priority: indexer.priority,
      };
      mergedSchema.fields.forEach(f => {
        data[f.name] = f.value;
      });
      this.formData.set(data);

      this.dialogMode.set('edit');
    } catch {
      showError('Failed to load indexer');
    }
  }

  updateField(name: string, value: unknown): void {
    const current = this.formData.value;
    this.formData.set({ ...current, [name]: value });
    this.testResult.set(null);
  }

  async handleTestConnection(): Promise<void> {
    this.isTesting.set(true);
    this.testResult.set(null);

    try {
      const payload = this.buildPayload();
      const response = await fetch('/api/v3/indexer/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        this.testResult.set({
          success: true,
          message: result.message || 'Connection successful!',
        });
      } else {
        this.testResult.set({
          success: false,
          message: result.message || 'Connection test failed',
        });
      }
    } catch {
      this.testResult.set({
        success: false,
        message: 'Test failed',
      });
    } finally {
      this.isTesting.set(false);
    }
  }

  async handleSave(): Promise<void> {
    this.isSaving.set(true);

    try {
      const payload = this.buildPayload();
      const id = this.editingId.value;
      const method = id ? 'PUT' : 'POST';
      const url = id ? `/api/v3/indexer/${id}` : '/api/v3/indexer';

      const response = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to save');
      }

      invalidateQueries(['/indexer']);
      showSuccess('Indexer saved');
      this.closeDialog();
    } catch (err) {
      showError(err instanceof Error ? err.message : 'Failed to save');
    } finally {
      this.isSaving.set(false);
    }
  }

  private buildPayload(): Record<string, unknown> {
    const schema = this.selectedSchema.value;
    if (!schema) return {};

    const data = this.formData.value;
    const fields = schema.fields.map(f => ({
      name: f.name,
      value: data[f.name],
    }));

    return {
      id: this.editingId.value || 0,
      name: data.name,
      enableRss: data.enableRss,
      enableAutomaticSearch: data.enableAutomaticSearch,
      enableInteractiveSearch: data.enableInteractiveSearch,
      implementation: schema.implementation,
      implementationName: schema.implementationName,
      configContract: schema.configContract,
      protocol: schema.protocol,
      priority: data.priority,
      fields,
      tags: [],
    };
  }

  async handleTest(id: number): Promise<void> {
    try {
      const indexer = await httpV3.get<Indexer>(`/indexer/${id}`);
      const response = await fetch('/api/v3/indexer/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(indexer),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        showSuccess(result.message || 'Connection successful!');
      } else {
        showError(result.message || 'Connection test failed');
      }
    } catch {
      showError('Test failed');
    }
  }

  handleDelete(id: number): void {
    if (confirm('Are you sure you want to delete this indexer?')) {
      this.deleteMutation.mutate(id);
    }
  }

  closeDialog(): void {
    this.dialogMode.set('closed');
    this.selectedSchema.set(null);
    this.editingId.set(null);
    this.formData.set({});
    this.testResult.set(null);
  }

  private styles(): string {
    return `<style>
      .loading-container {
        display: flex;
        justify-content: center;
        padding: 4rem;
      }

      .loading-spinner, .spinner {
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
        padding: 1.5rem;
        background-color: var(--bg-card);
        border: 1px solid var(--border-color);
        border-radius: 0.5rem;
      }

      .section-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 1.5rem;
        padding-bottom: 0.75rem;
        border-bottom: 1px solid var(--border-color);
      }

      .section-title {
        font-size: 1.125rem;
        font-weight: 600;
        margin: 0;
      }

      .add-btn {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.5rem 1rem;
        background-color: var(--btn-primary-bg);
        border: 1px solid var(--btn-primary-border);
        border-radius: 0.25rem;
        color: var(--color-white);
        font-size: 0.875rem;
        cursor: pointer;
      }

      .add-btn:hover {
        background-color: var(--btn-primary-bg-hover);
      }

      .empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.5rem;
        padding: 3rem;
        text-align: center;
        color: var(--text-color-muted);
      }

      .empty-state .hint {
        font-size: 0.875rem;
      }

      .indexers-list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
      }

      .indexer-card {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 1rem;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
      }

      .indexer-info {
        flex: 1;
      }

      .indexer-name {
        font-weight: 500;
        margin-bottom: 0.25rem;
      }

      .indexer-meta {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        font-size: 0.75rem;
      }

      .protocol-badge {
        padding: 0.125rem 0.5rem;
        border-radius: 9999px;
        font-weight: 500;
      }

      .protocol-badge.usenet {
        background-color: var(--color-usenet, #5d9cec);
        color: var(--color-white);
      }

      .protocol-badge.torrent {
        background-color: var(--color-torrent, #f0ad4e);
        color: var(--color-white);
      }

      .implementation {
        color: var(--text-color-muted);
      }

      .indexer-features {
        display: flex;
        gap: 0.25rem;
      }

      .feature {
        font-size: 0.75rem;
        padding: 0.125rem 0.5rem;
        border-radius: 0.25rem;
      }

      .feature.enabled {
        background-color: var(--color-success);
        color: var(--color-white);
      }

      .feature.disabled {
        background-color: var(--bg-card);
        color: var(--text-color-muted);
      }

      .indexer-actions {
        display: flex;
        gap: 0.25rem;
      }

      .action-btn {
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

      .action-btn:hover {
        color: var(--color-primary);
        background-color: var(--bg-input-hover);
      }

      .action-btn.danger:hover {
        color: var(--color-danger);
      }

      /* Dialog styles */
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
        max-width: 600px;
        max-height: 90vh;
        display: flex;
        flex-direction: column;
        box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
      }

      .dialog-form {
        max-width: 500px;
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

      .loading-center {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1rem;
        padding: 2rem;
        color: var(--text-color-muted);
      }

      .info-box {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.75rem 1rem;
        background-color: rgba(93, 156, 236, 0.1);
        border: 1px solid rgba(93, 156, 236, 0.3);
        border-radius: 0.375rem;
        color: var(--color-primary);
        font-size: 0.875rem;
        margin-bottom: 1.5rem;
      }

      .group-header {
        margin-top: 1.5rem;
        margin-bottom: 0.75rem;
        padding-bottom: 0.5rem;
        border-bottom: 1px solid var(--border-color);
      }

      .group-header:first-of-type {
        margin-top: 0;
      }

      .group-header h3 {
        margin: 0;
        font-size: 0.875rem;
        font-weight: 600;
        text-transform: capitalize;
        color: var(--text-color-muted);
      }

      .provider-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
        gap: 0.75rem;
      }

      .provider-card {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.5rem;
        padding: 1rem;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        cursor: pointer;
        transition: all 0.15s ease;
      }

      .provider-card:hover {
        border-color: var(--color-primary);
        background-color: var(--bg-input-hover);
      }

      .provider-icon {
        color: var(--color-primary);
      }

      .provider-name {
        font-size: 0.8125rem;
        font-weight: 500;
        color: var(--text-color);
        text-align: center;
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

      .btn-default, .btn-secondary {
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        color: var(--text-color);
      }

      .btn-default:hover:not(:disabled),
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
    </style>`;
  }
}
