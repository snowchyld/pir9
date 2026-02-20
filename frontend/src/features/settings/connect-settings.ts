/**
 * Connect Settings page - Notifications
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { httpV3 } from '../../core/http';
import { createMutation, createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { showError, showSuccess } from '../../stores/app.store';
import type { NotificationSchema, ProviderField } from './provider-types';

interface Notification {
  id: number;
  name: string;
  implementation: string;
  implementationName: string;
  configContract: string;
  fields: ProviderField[];
  tags: number[];
  onGrab: boolean;
  onDownload: boolean;
  onUpgrade: boolean;
  onImportComplete: boolean;
  onRename: boolean;
  onSeriesAdd: boolean;
  onSeriesDelete: boolean;
  onEpisodeFileDelete: boolean;
  onEpisodeFileDeleteForUpgrade: boolean;
  onHealthIssue: boolean;
  includeHealthWarnings: boolean;
  onHealthRestored: boolean;
  onApplicationUpdate: boolean;
  onManualInteractionRequired: boolean;
}

type DialogMode = 'closed' | 'select' | 'edit';

@customElement('connect-settings')
export class ConnectSettings extends BaseComponent {
  private notificationsQuery = createQuery({
    queryKey: ['/notification'],
    queryFn: () => httpV3.get<Notification[]>('/notification'),
  });

  private deleteMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/notification/${id}`),
    onSuccess: () => {
      invalidateQueries(['/notification']);
      showSuccess('Connection deleted');
    },
    onError: () => {
      showError('Failed to delete connection');
    },
  });

  // Dialog state
  private dialogMode = signal<DialogMode>('closed');
  private schemas = signal<NotificationSchema[]>([]);
  private schemasLoading = signal(false);
  private selectedSchema = signal<NotificationSchema | null>(null);
  private editingId = signal<number | null>(null);
  private formData = signal<Record<string, unknown>>({});
  private isSaving = signal(false);
  private isTesting = signal(false);
  private testResult = signal<{ success: boolean; message: string } | null>(null);

  protected onInit(): void {
    this.watch(this.notificationsQuery.data);
    this.watch(this.notificationsQuery.isLoading);
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
    const notifications = this.notificationsQuery.data.value ?? [];
    const isLoading = this.notificationsQuery.isLoading.value;
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
          <h2 class="section-title">Connections</h2>
          <button class="add-btn" onclick="this.closest('connect-settings').handleAdd()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Connection
          </button>
        </div>

        ${
          notifications.length === 0
            ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"></path>
              <path d="M13.73 21a2 2 0 0 1-3.46 0"></path>
            </svg>
            <p>No connections configured</p>
            <p class="hint">Add connections to get notified about grabs, downloads, and more</p>
          </div>
        `
            : html`
          <div class="connections-list">
            ${notifications
              .map(
                (n) => html`
              <div class="connection-card">
                <div class="connection-info">
                  <div class="connection-name">${escapeHtml(n.name)}</div>
                  <div class="connection-type">${escapeHtml(n.implementation)}</div>
                </div>
                <div class="connection-triggers">
                  ${n.onGrab ? '<span class="trigger">Grab</span>' : ''}
                  ${n.onDownload ? '<span class="trigger">Download</span>' : ''}
                  ${n.onUpgrade ? '<span class="trigger">Upgrade</span>' : ''}
                  ${n.onSeriesAdd ? '<span class="trigger">Series Add</span>' : ''}
                  ${n.onHealthIssue ? '<span class="trigger">Health</span>' : ''}
                </div>
                <div class="connection-actions">
                  <button class="action-btn" onclick="event.stopPropagation(); this.closest('connect-settings').handleTest(${n.id})" title="Test">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polygon points="5 3 19 12 5 21 5 3"></polygon>
                    </svg>
                  </button>
                  <button class="action-btn" onclick="event.stopPropagation(); this.closest('connect-settings').handleEdit(${n.id})" title="Edit">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
                  <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('connect-settings').handleDelete(${n.id})" title="Delete">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polyline points="3 6 5 6 21 6"></polyline>
                      <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                    </svg>
                  </button>
                </div>
              </div>
            `,
              )
              .join('')}
          </div>
        `
        }
      </div>

      ${mode === 'select' ? this.renderSelectDialog() : ''}
      ${mode === 'edit' ? this.renderEditDialog() : ''}

      ${safeHtml(this.styles())}
    `;
  }

  private renderSelectDialog(): string {
    const schemas = this.schemas.value;
    const loading = this.schemasLoading.value;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('connect-settings').closeDialog()">
        <div class="dialog">
          <div class="dialog-header">
            <h2>Add Connection</h2>
            <button class="close-btn" onclick="this.closest('connect-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            ${
              loading
                ? html`
              <div class="loading-center">
                <div class="spinner"></div>
                <p>Loading available connections...</p>
              </div>
            `
                : html`
              <div class="info-box">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="16" x2="12" y2="12"></line>
                  <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
                <span>Select a connection type to configure</span>
              </div>

              <div class="provider-grid">
                ${schemas.map((schema) => this.renderSchemaCard(schema)).join('')}
              </div>
            `
            }
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('connect-settings').closeDialog()">
              Cancel
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderSchemaCard(schema: NotificationSchema): string {
    return html`
      <button class="provider-card" onclick="this.closest('connect-settings').selectSchema('${escapeHtml(schema.implementation)}')">
        <div class="provider-icon">
          ${this.getProviderIcon(schema.implementation)}
        </div>
        <span class="provider-name">${escapeHtml(schema.implementationName)}</span>
      </button>
    `;
  }

  private getProviderIcon(implementation: string): string {
    // Different icons for different notification types
    const icons: Record<string, string> = {
      Discord: `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M20.317 4.492c-1.53-.69-3.17-1.2-4.885-1.49a.075.075 0 00-.079.036c-.21.369-.444.85-.608 1.23a18.566 18.566 0 00-5.487 0 12.36 12.36 0 00-.617-1.23A.077.077 0 008.562 3c-1.714.29-3.354.8-4.885 1.491a.07.07 0 00-.032.027C.533 9.093-.32 13.555.099 17.961a.08.08 0 00.031.055 20.03 20.03 0 005.993 2.98.078.078 0 00.084-.026c.462-.62.874-1.275 1.226-1.963.021-.04.001-.088-.041-.104a13.201 13.201 0 01-1.872-.878.075.075 0 01-.008-.125c.126-.093.252-.19.372-.287a.075.075 0 01.078-.01c3.927 1.764 8.18 1.764 12.061 0a.075.075 0 01.079.009c.12.098.245.195.372.288a.075.075 0 01-.006.125c-.598.344-1.22.635-1.873.877a.075.075 0 00-.041.105c.36.687.772 1.341 1.225 1.962a.077.077 0 00.084.028 19.963 19.963 0 006.002-2.981.076.076 0 00.032-.054c.5-5.094-.838-9.52-3.549-13.442a.06.06 0 00-.031-.028zM8.02 15.278c-1.182 0-2.157-1.069-2.157-2.38 0-1.312.956-2.38 2.157-2.38 1.21 0 2.176 1.077 2.157 2.38 0 1.312-.956 2.38-2.157 2.38zm7.975 0c-1.183 0-2.157-1.069-2.157-2.38 0-1.312.955-2.38 2.157-2.38 1.21 0 2.176 1.077 2.157 2.38 0 1.312-.946 2.38-2.157 2.38z"/></svg>`,
      Telegram: `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M11.944 0A12 12 0 000 12a12 12 0 0012 12 12 12 0 0012-12A12 12 0 0012 0a12 12 0 00-.056 0zm4.962 7.224c.1-.002.321.023.465.14a.506.506 0 01.171.325c.016.093.036.306.02.472-.18 1.898-.962 6.502-1.36 8.627-.168.9-.499 1.201-.82 1.23-.696.065-1.225-.46-1.9-.902-1.056-.693-1.653-1.124-2.678-1.8-1.185-.78-.417-1.21.258-1.91.177-.184 3.247-2.977 3.307-3.23.007-.032.014-.15-.056-.212s-.174-.041-.249-.024c-.106.024-1.793 1.14-5.061 3.345-.48.33-.913.49-1.302.48-.428-.008-1.252-.241-1.865-.44-.752-.245-1.349-.374-1.297-.789.027-.216.325-.437.893-.663 3.498-1.524 5.83-2.529 6.998-3.014 3.332-1.386 4.025-1.627 4.476-1.635z"/></svg>`,
      Email: `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M4 4h16c1.1 0 2 .9 2 2v12c0 1.1-.9 2-2 2H4c-1.1 0-2-.9-2-2V6c0-1.1.9-2 2-2z"></path><polyline points="22,6 12,13 2,6"></polyline></svg>`,
      Webhook: `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path></svg>`,
      Pushover: `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"></path><path d="M13.73 21a2 2 0 0 1-3.46 0"></path></svg>`,
    };
    return (
      icons[implementation] ||
      `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"></path><path d="M13.73 21a2 2 0 0 1-3.46 0"></path></svg>`
    );
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
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('connect-settings').closeDialog()">
        <div class="dialog dialog-form">
          <div class="dialog-header">
            <h2>${isEditing ? 'Edit' : 'Add'} ${escapeHtml(schema.implementationName)}</h2>
            <button class="close-btn" onclick="this.closest('connect-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            ${
              testResult
                ? html`
              <div class="test-result ${testResult.success ? 'success' : 'error'}">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  ${
                    testResult.success
                      ? '<polyline points="20 6 9 17 4 12"></polyline>'
                      : '<line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line>'
                  }
                </svg>
                <span>${escapeHtml(testResult.message)}</span>
              </div>
            `
                : ''
            }

            <form class="provider-form" onsubmit="event.preventDefault()">
              <div class="form-group">
                <label for="name">Name</label>
                <input
                  type="text"
                  id="name"
                  value="${escapeHtml(String(data.name || ''))}"
                  onchange="this.closest('connect-settings').updateField('name', this.value)"
                />
              </div>

              ${schema.fields
                .filter((f) => f.hidden !== 'hidden')
                .sort((a, b) => a.order - b.order)
                .map((field) => this.renderField(field, data))
                .join('')}

              <fieldset class="form-fieldset">
                <legend>Notification Triggers</legend>
                ${
                  schema.supportsOnGrab
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onGrab ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onGrab', this.checked)" />
                      <span>On Grab</span>
                    </label>
                    <p class="help-text">Notify when episodes are grabbed</p>
                  </div>
                `
                    : ''
                }
                ${
                  schema.supportsOnDownload
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onDownload ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onDownload', this.checked)" />
                      <span>On Download</span>
                    </label>
                    <p class="help-text">Notify when episodes are downloaded</p>
                  </div>
                `
                    : ''
                }
                ${
                  schema.supportsOnUpgrade
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onUpgrade ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onUpgrade', this.checked)" />
                      <span>On Upgrade</span>
                    </label>
                    <p class="help-text">Notify when episodes are upgraded</p>
                  </div>
                `
                    : ''
                }
                ${
                  schema.supportsOnSeriesAdd
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onSeriesAdd ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onSeriesAdd', this.checked)" />
                      <span>On Series Add</span>
                    </label>
                    <p class="help-text">Notify when series are added</p>
                  </div>
                `
                    : ''
                }
                ${
                  schema.supportsOnSeriesDelete
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onSeriesDelete ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onSeriesDelete', this.checked)" />
                      <span>On Series Delete</span>
                    </label>
                    <p class="help-text">Notify when series are deleted</p>
                  </div>
                `
                    : ''
                }
                ${
                  schema.supportsOnHealthIssue
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onHealthIssue ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onHealthIssue', this.checked)" />
                      <span>On Health Issue</span>
                    </label>
                    <p class="help-text">Notify on health check failures</p>
                  </div>
                `
                    : ''
                }
                ${
                  schema.supportsOnApplicationUpdate
                    ? html`
                  <div class="form-group form-group-checkbox">
                    <label>
                      <input type="checkbox" ${data.onApplicationUpdate ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('onApplicationUpdate', this.checked)" />
                      <span>On Application Update</span>
                    </label>
                    <p class="help-text">Notify when pir9 updates</p>
                  </div>
                `
                    : ''
                }
              </fieldset>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-default" onclick="this.closest('connect-settings').handleTestConnection()" ${isTesting ? 'disabled' : ''}>
              ${isTesting ? 'Testing...' : 'Test'}
            </button>
            <div class="footer-spacer"></div>
            <button class="btn btn-secondary" onclick="this.closest('connect-settings').closeDialog()">
              Cancel
            </button>
            <button class="btn btn-primary" onclick="this.closest('connect-settings').handleSave()" ${isSaving ? 'disabled' : ''}>
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
            <input type="text" id="${fieldId}" value="${escapeHtml(String(value ?? ''))}" placeholder="${escapeHtml(field.placeholder || '')}" onchange="this.closest('connect-settings').updateField('${field.name}', this.value)" />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'password':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input type="password" id="${fieldId}" value="${escapeHtml(String(value ?? ''))}" onchange="this.closest('connect-settings').updateField('${field.name}', this.value)" />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'number':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input type="number" id="${fieldId}" value="${value ?? ''}" onchange="this.closest('connect-settings').updateField('${field.name}', ${field.isFloat ? 'parseFloat(this.value)' : 'parseInt(this.value)'})" />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'checkbox':
        return html`
          <div class="form-group form-group-checkbox">
            <label>
              <input type="checkbox" ${value ? 'checked' : ''} onchange="this.closest('connect-settings').updateField('${field.name}', this.checked)" />
              <span>${escapeHtml(field.label)}</span>
            </label>
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'select':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <select id="${fieldId}" onchange="this.closest('connect-settings').updateField('${field.name}', this.value)">
              ${(field.selectOptions || [])
                .map(
                  (opt) => html`
                <option value="${opt.value}" ${String(value) === String(opt.value) ? 'selected' : ''}>${escapeHtml(opt.name)}</option>
              `,
                )
                .join('')}
            </select>
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      default:
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input type="text" id="${fieldId}" value="${escapeHtml(String(value ?? ''))}" onchange="this.closest('connect-settings').updateField('${field.name}', this.value)" />
          </div>
        `;
    }
  }

  // Public methods called from template
  async handleAdd(): Promise<void> {
    this.dialogMode.set('select');
    this.schemasLoading.set(true);

    try {
      const schemas = await httpV3.get<NotificationSchema[]>('/notification/schema');
      this.schemas.set(schemas);
    } catch {
      showError('Failed to load connection types');
      this.dialogMode.set('closed');
    } finally {
      this.schemasLoading.set(false);
    }
  }

  selectSchema(implementation: string): void {
    const schema = this.schemas.value.find((s) => s.implementation === implementation);
    if (!schema) return;

    this.selectedSchema.set(schema);
    this.editingId.set(null);
    this.testResult.set(null);

    // Initialize form data with defaults
    const data: Record<string, unknown> = {
      name: schema.implementationName,
      onGrab: false,
      onDownload: true,
      onUpgrade: true,
      onImportComplete: false,
      onRename: false,
      onSeriesAdd: false,
      onSeriesDelete: false,
      onEpisodeFileDelete: false,
      onEpisodeFileDeleteForUpgrade: false,
      onHealthIssue: false,
      includeHealthWarnings: true,
      onHealthRestored: false,
      onApplicationUpdate: false,
      onManualInteractionRequired: false,
    };
    schema.fields.forEach((f) => {
      data[f.name] = f.value;
    });
    this.formData.set(data);

    this.dialogMode.set('edit');
  }

  async handleEdit(id: number): Promise<void> {
    try {
      const notification = await httpV3.get<Notification>(`/notification/${id}`);
      const schemas = await httpV3.get<NotificationSchema[]>('/notification/schema');
      const schema = schemas.find((s) => s.implementation === notification.implementation);

      if (!schema) {
        showError('Unknown connection type');
        return;
      }

      // Merge schema field definitions with notification values
      const mergedSchema: NotificationSchema = {
        ...schema,
        fields: schema.fields.map((f) => ({
          ...f,
          value: notification.fields.find((nf) => nf.name === f.name)?.value ?? f.value,
        })),
      };

      this.schemas.set(schemas);
      this.selectedSchema.set(mergedSchema);
      this.editingId.set(id);
      this.testResult.set(null);

      // Initialize form data from notification
      const data: Record<string, unknown> = {
        name: notification.name,
        onGrab: notification.onGrab,
        onDownload: notification.onDownload,
        onUpgrade: notification.onUpgrade,
        onImportComplete: notification.onImportComplete,
        onRename: notification.onRename,
        onSeriesAdd: notification.onSeriesAdd,
        onSeriesDelete: notification.onSeriesDelete,
        onEpisodeFileDelete: notification.onEpisodeFileDelete,
        onEpisodeFileDeleteForUpgrade: notification.onEpisodeFileDeleteForUpgrade,
        onHealthIssue: notification.onHealthIssue,
        includeHealthWarnings: notification.includeHealthWarnings,
        onHealthRestored: notification.onHealthRestored,
        onApplicationUpdate: notification.onApplicationUpdate,
        onManualInteractionRequired: notification.onManualInteractionRequired,
      };
      mergedSchema.fields.forEach((f) => {
        data[f.name] = f.value;
      });
      this.formData.set(data);

      this.dialogMode.set('edit');
    } catch {
      showError('Failed to load connection');
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
      const response = await fetch('/api/v3/notification/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        this.testResult.set({
          success: true,
          message: result.message || 'Test notification sent!',
        });
      } else {
        this.testResult.set({ success: false, message: result.message || 'Test failed' });
      }
    } catch {
      this.testResult.set({ success: false, message: 'Test failed' });
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
      const url = id ? `/api/v3/notification/${id}` : '/api/v3/notification';

      const response = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to save');
      }

      invalidateQueries(['/notification']);
      showSuccess('Connection saved');
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
    const fields = schema.fields.map((f) => ({
      name: f.name,
      value: data[f.name],
    }));

    return {
      id: this.editingId.value || 0,
      name: data.name,
      implementation: schema.implementation,
      implementationName: schema.implementationName,
      configContract: schema.configContract,
      fields,
      tags: [],
      onGrab: data.onGrab,
      onDownload: data.onDownload,
      onUpgrade: data.onUpgrade,
      onImportComplete: data.onImportComplete,
      onRename: data.onRename,
      onSeriesAdd: data.onSeriesAdd,
      onSeriesDelete: data.onSeriesDelete,
      onEpisodeFileDelete: data.onEpisodeFileDelete,
      onEpisodeFileDeleteForUpgrade: data.onEpisodeFileDeleteForUpgrade,
      onHealthIssue: data.onHealthIssue,
      includeHealthWarnings: data.includeHealthWarnings,
      onHealthRestored: data.onHealthRestored,
      onApplicationUpdate: data.onApplicationUpdate,
      onManualInteractionRequired: data.onManualInteractionRequired,
    };
  }

  async handleTest(id: number): Promise<void> {
    try {
      const notification = await httpV3.get<Notification>(`/notification/${id}`);
      const response = await fetch('/api/v3/notification/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(notification),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        showSuccess(result.message || 'Test notification sent!');
      } else {
        showError(result.message || 'Test failed');
      }
    } catch {
      showError('Test failed');
    }
  }

  handleDelete(id: number): void {
    if (confirm('Are you sure you want to delete this connection?')) {
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

      .connections-list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
      }

      .connection-card {
        display: flex;
        align-items: center;
        gap: 1rem;
        padding: 1rem;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
      }

      .connection-info {
        flex: 1;
      }

      .connection-name {
        font-weight: 500;
        margin-bottom: 0.25rem;
      }

      .connection-type {
        font-size: 0.75rem;
        color: var(--text-color-muted);
      }

      .connection-triggers {
        display: flex;
        flex-wrap: wrap;
        gap: 0.25rem;
      }

      .trigger {
        font-size: 0.625rem;
        padding: 0.125rem 0.375rem;
        background-color: var(--color-primary);
        color: var(--color-white);
        border-radius: 0.25rem;
        text-transform: uppercase;
        font-weight: 500;
      }

      .connection-actions {
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
