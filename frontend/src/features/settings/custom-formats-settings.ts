/**
 * Custom Formats Settings page
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { httpV3 } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';
import { signal } from '../../core/reactive';

interface SelectOption {
  value: unknown;
  name: string;
  order: number;
}

interface FieldResource {
  order: number;
  name: string;
  label: string;
  helpText?: string;
  value?: unknown;
  type: string;
  selectOptions?: SelectOption[];
}

interface Specification {
  id: number;
  name: string;
  implementation: string;
  implementationName: string;
  negate: boolean;
  required: boolean;
  fields: FieldResource[];
}

interface CustomFormat {
  id: number;
  name: string;
  includeCustomFormatWhenRenaming: boolean;
  specifications: Specification[];
}

type DialogMode = 'closed' | 'select-spec' | 'edit' | 'edit-spec';

@customElement('custom-formats-settings')
export class CustomFormatsSettings extends BaseComponent {
  private formatsQuery = createQuery({
    queryKey: ['/customformat'],
    queryFn: () => httpV3.get<CustomFormat[]>('/customformat'),
  });

  private deleteMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/customformat/${id}`),
    onSuccess: () => {
      invalidateQueries(['/customformat']);
      showSuccess('Custom format deleted');
    },
    onError: () => showError('Failed to delete custom format'),
  });

  // Dialog state
  private dialogMode = signal<DialogMode>('closed');
  private editingId = signal<number | null>(null);
  private isSaving = signal(false);
  private specSchemas = signal<Specification[]>([]);
  private editingSpecIndex = signal<number | null>(null);

  // Form data
  private formData = signal<{
    name: string;
    includeCustomFormatWhenRenaming: boolean;
    specifications: Specification[];
  }>({
    name: '',
    includeCustomFormatWhenRenaming: false,
    specifications: [],
  });

  // Spec editing form
  private specFormData = signal<Specification | null>(null);

  protected onInit(): void {
    this.watch(this.formatsQuery.data);
    this.watch(this.formatsQuery.isLoading);
    this.watch(this.dialogMode);
    this.watch(this.formData);
    this.watch(this.specSchemas);
    this.watch(this.specFormData);
    this.watch(this.isSaving);
  }

  protected template(): string {
    const formats = this.formatsQuery.data.value ?? [];
    const isLoading = this.formatsQuery.isLoading.value;
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
          <h2 class="section-title">Custom Formats</h2>
          <button class="add-btn" onclick="this.closest('custom-formats-settings').handleAdd()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Custom Format
          </button>
        </div>

        ${formats.length === 0 ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
              <polyline points="2 17 12 22 22 17"></polyline>
              <polyline points="2 12 12 17 22 12"></polyline>
            </svg>
            <p>No custom formats configured</p>
            <p class="hint">Custom formats allow fine-grained control over quality preferences</p>
          </div>
        ` : html`
          <div class="formats-grid">
            ${formats.map((format) => html`
              <div class="format-card">
                <div class="format-content" onclick="this.closest('custom-formats-settings').handleEdit(${format.id})">
                  <div class="format-header">
                    <span class="format-name">${escapeHtml(format.name)}</span>
                    ${format.includeCustomFormatWhenRenaming ? html`
                      <span class="rename-badge">In Rename</span>
                    ` : ''}
                  </div>
                  <div class="format-specs">
                    ${format.specifications.length} specification${format.specifications.length !== 1 ? 's' : ''}
                  </div>
                  <div class="spec-list">
                    ${format.specifications.slice(0, 3).map((spec) => html`
                      <span class="spec-badge ${spec.negate ? 'negate' : ''} ${spec.required ? 'required' : ''}">${escapeHtml(spec.name)}</span>
                    `).join('')}
                    ${format.specifications.length > 3 ? html`
                      <span class="spec-more">+${format.specifications.length - 3} more</span>
                    ` : ''}
                  </div>
                </div>
                <div class="format-actions">
                  <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('custom-formats-settings').handleDelete(${format.id})" title="Delete">
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

      ${mode === 'edit' ? this.renderEditDialog() : ''}
      ${mode === 'select-spec' ? this.renderSelectSpecDialog() : ''}
      ${mode === 'edit-spec' ? this.renderEditSpecDialog() : ''}

      ${safeHtml(this.styles())}
    `;
  }

  private renderEditDialog(): string {
    const data = this.formData.value;
    const isSaving = this.isSaving.value;
    const isEditing = this.editingId.value !== null;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('custom-formats-settings').closeDialog()">
        <div class="dialog dialog-wide">
          <div class="dialog-header">
            <h2>${isEditing ? 'Edit' : 'Add'} Custom Format</h2>
            <button class="close-btn" onclick="this.closest('custom-formats-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            <form class="custom-format-form" onsubmit="event.preventDefault()">
              <div class="form-group">
                <label for="cf-name">Name</label>
                <input
                  type="text"
                  id="cf-name"
                  value="${escapeHtml(data.name)}"
                  onchange="this.closest('custom-formats-settings').updateField('name', this.value)"
                  placeholder="Custom format name"
                />
              </div>

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.includeCustomFormatWhenRenaming ? 'checked' : ''}
                    onchange="this.closest('custom-formats-settings').updateField('includeCustomFormatWhenRenaming', this.checked)"
                  />
                  <span>Include Custom Format When Renaming</span>
                </label>
                <p class="help-text">Include the custom format name when renaming files</p>
              </div>

              <div class="specs-section">
                <div class="specs-header">
                  <h3>Specifications</h3>
                  <button type="button" class="add-spec-btn" onclick="this.closest('custom-formats-settings').handleAddSpec()">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <line x1="12" y1="5" x2="12" y2="19"></line>
                      <line x1="5" y1="12" x2="19" y2="12"></line>
                    </svg>
                    Add Specification
                  </button>
                </div>

                ${data.specifications.length === 0 ? html`
                  <div class="specs-empty">
                    <p>No specifications added</p>
                    <p class="hint">Add specifications to define matching criteria</p>
                  </div>
                ` : html`
                  <div class="specs-list">
                    ${data.specifications.map((spec, index) => html`
                      <div class="spec-row">
                        <div class="spec-info" onclick="this.closest('custom-formats-settings').handleEditSpec(${index})">
                          <span class="spec-name">${escapeHtml(spec.name || spec.implementationName)}</span>
                          <span class="spec-type">${escapeHtml(spec.implementationName)}</span>
                          <div class="spec-flags">
                            ${spec.negate ? html`<span class="flag negate">Negate</span>` : ''}
                            ${spec.required ? html`<span class="flag required">Required</span>` : ''}
                          </div>
                        </div>
                        <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('custom-formats-settings').handleRemoveSpec(${index})" title="Remove">
                          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                            <line x1="18" y1="6" x2="6" y2="18"></line>
                            <line x1="6" y1="6" x2="18" y2="18"></line>
                          </svg>
                        </button>
                      </div>
                    `).join('')}
                  </div>
                `}
              </div>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('custom-formats-settings').closeDialog()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('custom-formats-settings').handleSave()"
              ${isSaving ? 'disabled' : ''}
            >
              ${isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderSelectSpecDialog(): string {
    const schemas = this.specSchemas.value;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('custom-formats-settings').handleBackToEdit()">
        <div class="dialog">
          <div class="dialog-header">
            <h2>Add Specification</h2>
            <button class="close-btn" onclick="this.closest('custom-formats-settings').handleBackToEdit()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            <div class="info-box">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="10"></circle>
                <line x1="12" y1="16" x2="12" y2="12"></line>
                <line x1="12" y1="8" x2="12.01" y2="8"></line>
              </svg>
              <span>Select a specification type</span>
            </div>
            <div class="spec-type-grid">
              ${schemas.map((schema) => html`
                <button class="spec-type-card" onclick="this.closest('custom-formats-settings').selectSpecType('${escapeHtml(schema.implementation)}')">
                  <span class="spec-type-name">${escapeHtml(schema.implementationName)}</span>
                </button>
              `).join('')}
            </div>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('custom-formats-settings').handleBackToEdit()">
              Back
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderEditSpecDialog(): string {
    const spec = this.specFormData.value;
    if (!spec) return '';

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('custom-formats-settings').handleBackToEdit()">
        <div class="dialog">
          <div class="dialog-header">
            <h2>Edit Specification</h2>
            <button class="close-btn" onclick="this.closest('custom-formats-settings').handleBackToEdit()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            <form class="spec-form" onsubmit="event.preventDefault()">
              <div class="form-group">
                <label for="spec-name">Name</label>
                <input
                  type="text"
                  id="spec-name"
                  value="${escapeHtml(spec.name)}"
                  onchange="this.closest('custom-formats-settings').updateSpecField('name', this.value)"
                  placeholder="Specification name"
                />
              </div>

              <div class="form-group">
                <label>Type</label>
                <input type="text" value="${escapeHtml(spec.implementationName)}" disabled />
              </div>

              ${spec.fields.map((field, index) => this.renderSpecField(field, index)).join('')}

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${spec.negate ? 'checked' : ''}
                    onchange="this.closest('custom-formats-settings').updateSpecField('negate', this.checked)"
                  />
                  <span>Negate</span>
                </label>
                <p class="help-text">Negate this specification (must NOT match)</p>
              </div>

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${spec.required ? 'checked' : ''}
                    onchange="this.closest('custom-formats-settings').updateSpecField('required', this.checked)"
                  />
                  <span>Required</span>
                </label>
                <p class="help-text">All other required specifications must also match</p>
              </div>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('custom-formats-settings').handleBackToEdit()">
              Back
            </button>
            <button class="btn btn-primary" onclick="this.closest('custom-formats-settings').handleSaveSpec()">
              Confirm
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderSpecField(field: FieldResource, index: number): string {
    const fieldId = `spec-field-${index}`;

    switch (field.type) {
      case 'select':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <select
              id="${fieldId}"
              onchange="this.closest('custom-formats-settings').updateSpecFieldValue(${index}, this.value)"
            >
              ${(field.selectOptions || []).map((opt) => html`
                <option value="${String(opt.value)}" ${String(field.value) === String(opt.value) ? 'selected' : ''}>
                  ${escapeHtml(opt.name)}
                </option>
              `).join('')}
            </select>
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
              value="${field.value ?? ''}"
              onchange="this.closest('custom-formats-settings').updateSpecFieldValue(${index}, parseFloat(this.value))"
            />
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
              value="${escapeHtml(String(field.value ?? ''))}"
              onchange="this.closest('custom-formats-settings').updateSpecFieldValue(${index}, this.value)"
            />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;
    }
  }

  // Handlers
  async handleAdd(): Promise<void> {
    try {
      const schemas = await httpV3.get<Specification[]>('/customformat/schema');
      this.specSchemas.set(schemas);
    } catch {
      // Continue even if schema fails
    }

    this.formData.set({
      name: '',
      includeCustomFormatWhenRenaming: false,
      specifications: [],
    });
    this.editingId.set(null);
    this.dialogMode.set('edit');
  }

  async handleEdit(id: number): Promise<void> {
    try {
      const format = await httpV3.get<CustomFormat>(`/customformat/${id}`);
      const schemas = await httpV3.get<Specification[]>('/customformat/schema');

      this.specSchemas.set(schemas);
      this.formData.set({
        name: format.name,
        includeCustomFormatWhenRenaming: format.includeCustomFormatWhenRenaming,
        specifications: format.specifications,
      });
      this.editingId.set(id);
      this.dialogMode.set('edit');
    } catch {
      showError('Failed to load custom format');
    }
  }

  updateField(field: string, value: unknown): void {
    const current = this.formData.value;
    this.formData.set({ ...current, [field]: value });
  }

  async handleAddSpec(): Promise<void> {
    if (this.specSchemas.value.length === 0) {
      try {
        const schemas = await httpV3.get<Specification[]>('/customformat/schema');
        this.specSchemas.set(schemas);
      } catch {
        showError('Failed to load specification types');
        return;
      }
    }
    this.dialogMode.set('select-spec');
  }

  selectSpecType(implementation: string): void {
    const schema = this.specSchemas.value.find(s => s.implementation === implementation);
    if (!schema) return;

    const newSpec: Specification = {
      id: 0,
      name: '',
      implementation: schema.implementation,
      implementationName: schema.implementationName,
      negate: false,
      required: false,
      fields: schema.fields.map(f => ({ ...f })),
    };

    this.specFormData.set(newSpec);
    this.editingSpecIndex.set(null); // Adding new spec
    this.dialogMode.set('edit-spec');
  }

  handleEditSpec(index: number): void {
    const specs = this.formData.value.specifications;
    const spec = specs[index];
    if (!spec) return;

    // Merge with schema to get full field definitions
    const schema = this.specSchemas.value.find(s => s.implementation === spec.implementation);
    if (schema) {
      spec.fields = schema.fields.map(f => ({
        ...f,
        value: spec.fields.find(sf => sf.name === f.name)?.value ?? f.value,
      }));
    }

    this.specFormData.set({ ...spec });
    this.editingSpecIndex.set(index);
    this.dialogMode.set('edit-spec');
  }

  handleRemoveSpec(index: number): void {
    const current = this.formData.value;
    const specs = [...current.specifications];
    specs.splice(index, 1);
    this.formData.set({ ...current, specifications: specs });
  }

  updateSpecField(field: string, value: unknown): void {
    const current = this.specFormData.value;
    if (!current) return;
    this.specFormData.set({ ...current, [field]: value });
  }

  updateSpecFieldValue(fieldIndex: number, value: unknown): void {
    const current = this.specFormData.value;
    if (!current) return;
    const fields = [...current.fields];
    fields[fieldIndex] = { ...fields[fieldIndex], value };
    this.specFormData.set({ ...current, fields });
  }

  handleSaveSpec(): void {
    const spec = this.specFormData.value;
    if (!spec) return;

    const current = this.formData.value;
    const specs = [...current.specifications];
    const editIndex = this.editingSpecIndex.value;

    if (editIndex !== null) {
      specs[editIndex] = spec;
    } else {
      specs.push(spec);
    }

    this.formData.set({ ...current, specifications: specs });
    this.specFormData.set(null);
    this.editingSpecIndex.set(null);
    this.dialogMode.set('edit');
  }

  handleBackToEdit(): void {
    this.specFormData.set(null);
    this.editingSpecIndex.set(null);
    this.dialogMode.set('edit');
  }

  async handleSave(): Promise<void> {
    this.isSaving.set(true);

    try {
      const data = this.formData.value;
      const payload: CustomFormat = {
        id: this.editingId.value || 0,
        name: data.name,
        includeCustomFormatWhenRenaming: data.includeCustomFormatWhenRenaming,
        specifications: data.specifications,
      };

      const id = this.editingId.value;
      if (id) {
        await httpV3.put(`/customformat/${id}`, payload);
      } else {
        await httpV3.post('/customformat', payload);
      }

      invalidateQueries(['/customformat']);
      showSuccess('Custom format saved');
      this.closeDialog();
    } catch {
      showError('Failed to save custom format');
    } finally {
      this.isSaving.set(false);
    }
  }

  handleDelete(id: number): void {
    if (confirm('Are you sure you want to delete this custom format?')) {
      this.deleteMutation.mutate(id);
    }
  }

  closeDialog(): void {
    this.dialogMode.set('closed');
    this.editingId.set(null);
    this.specFormData.set(null);
    this.editingSpecIndex.set(null);
  }

  private styles(): string {
    return `<style>
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

      .formats-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: 1rem;
      }

      .format-card {
        display: flex;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        transition: border-color 0.15s;
      }

      .format-card:hover {
        border-color: var(--color-primary);
      }

      .format-content {
        flex: 1;
        padding: 1rem;
        cursor: pointer;
      }

      .format-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 0.5rem;
      }

      .format-name {
        font-weight: 500;
      }

      .rename-badge {
        font-size: 0.625rem;
        padding: 0.125rem 0.375rem;
        background-color: var(--color-primary);
        color: var(--color-white);
        border-radius: 0.25rem;
        font-weight: 500;
      }

      .format-specs {
        font-size: 0.75rem;
        color: var(--text-color-muted);
        margin-bottom: 0.75rem;
      }

      .spec-list {
        display: flex;
        flex-wrap: wrap;
        gap: 0.25rem;
      }

      .spec-badge {
        font-size: 0.625rem;
        padding: 0.125rem 0.375rem;
        background-color: var(--bg-card);
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        color: var(--text-color-muted);
      }

      .spec-badge.required {
        background-color: var(--color-success);
        border-color: var(--color-success);
        color: var(--color-white);
      }

      .spec-badge.negate {
        background-color: var(--color-danger);
        border-color: var(--color-danger);
        color: var(--color-white);
      }

      .spec-more {
        font-size: 0.625rem;
        color: var(--text-color-muted);
      }

      .format-actions {
        display: flex;
        align-items: center;
        padding: 0.5rem;
        border-left: 1px solid var(--border-color);
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
        max-width: 500px;
        max-height: 90vh;
        display: flex;
        flex-direction: column;
        box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
      }

      .dialog-wide {
        max-width: 600px;
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
        justify-content: flex-end;
        gap: 0.75rem;
        padding: 1rem 1.5rem;
        border-top: 1px solid var(--border-color);
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
        margin-bottom: 1rem;
      }

      .form-group {
        display: flex;
        flex-direction: column;
        gap: 0.375rem;
        margin-bottom: 1rem;
      }

      .form-group:last-child {
        margin-bottom: 0;
      }

      .form-group label {
        font-size: 0.875rem;
        font-weight: 500;
      }

      .form-group input[type="text"],
      .form-group input[type="number"],
      .form-group select {
        padding: 0.5rem 0.75rem;
        background-color: var(--bg-input);
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        color: var(--text-color);
        font-size: 0.875rem;
      }

      .form-group input:disabled {
        opacity: 0.6;
        cursor: not-allowed;
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

      .specs-section {
        margin-top: 1.5rem;
        padding-top: 1rem;
        border-top: 1px solid var(--border-color);
      }

      .specs-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        margin-bottom: 1rem;
      }

      .specs-header h3 {
        margin: 0;
        font-size: 1rem;
        font-weight: 600;
      }

      .add-spec-btn {
        display: flex;
        align-items: center;
        gap: 0.375rem;
        padding: 0.375rem 0.75rem;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        color: var(--text-color);
        font-size: 0.8125rem;
        cursor: pointer;
      }

      .add-spec-btn:hover {
        background-color: var(--bg-input-hover);
      }

      .specs-empty {
        padding: 1.5rem;
        text-align: center;
        color: var(--text-color-muted);
        background-color: var(--bg-card-alt);
        border: 1px dashed var(--border-color);
        border-radius: 0.375rem;
      }

      .specs-empty .hint {
        font-size: 0.875rem;
        margin-top: 0.25rem;
      }

      .specs-list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
      }

      .spec-row {
        display: flex;
        align-items: center;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
      }

      .spec-info {
        flex: 1;
        display: flex;
        align-items: center;
        gap: 0.75rem;
        padding: 0.75rem 1rem;
        cursor: pointer;
      }

      .spec-info:hover {
        background-color: var(--bg-input-hover);
      }

      .spec-name {
        font-weight: 500;
      }

      .spec-type {
        font-size: 0.75rem;
        color: var(--text-color-muted);
      }

      .spec-flags {
        display: flex;
        gap: 0.25rem;
        margin-left: auto;
      }

      .flag {
        font-size: 0.625rem;
        padding: 0.125rem 0.375rem;
        border-radius: 0.25rem;
        font-weight: 500;
      }

      .flag.negate {
        background-color: var(--color-danger);
        color: var(--color-white);
      }

      .flag.required {
        background-color: var(--color-success);
        color: var(--color-white);
      }

      .spec-row .action-btn {
        margin: 0 0.5rem;
      }

      .spec-type-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
        gap: 0.5rem;
      }

      .spec-type-card {
        padding: 0.75rem;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        cursor: pointer;
        text-align: center;
        transition: all 0.15s ease;
      }

      .spec-type-card:hover {
        border-color: var(--color-primary);
        background-color: var(--bg-input-hover);
      }

      .spec-type-name {
        font-size: 0.875rem;
        font-weight: 500;
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
    </style>`;
  }
}
