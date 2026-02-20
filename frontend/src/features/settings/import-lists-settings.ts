/**
 * Import Lists Settings page
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { httpV3 } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';
import { signal } from '../../core/reactive';
import type { ImportListSchema, ProviderField } from './provider-types';

interface ImportList {
  id: number;
  name: string;
  implementation: string;
  implementationName: string;
  configContract: string;
  fields: ProviderField[];
  tags: number[];
  enableAutomaticAdd: boolean;
  searchForMissingEpisodes: boolean;
  shouldMonitor: string;
  rootFolderPath: string | null;
  qualityProfileId: number;
  seriesType: string;
  seasonFolder: boolean;
  listType: string;
  listOrder: number;
  minRefreshInterval: string;
}

interface QualityProfile {
  id: number;
  name: string;
}

interface RootFolder {
  id: number;
  path: string;
  accessible: boolean;
  freeSpace: number;
}

type DialogMode = 'closed' | 'select' | 'edit';

@customElement('import-lists-settings')
export class ImportListsSettings extends BaseComponent {
  private listsQuery = createQuery({
    queryKey: ['/importlist'],
    queryFn: () => httpV3.get<ImportList[]>('/importlist'),
  });

  private profilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => httpV3.get<QualityProfile[]>('/qualityprofile'),
  });

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder'],
    queryFn: () => httpV3.get<RootFolder[]>('/rootfolder'),
  });

  private deleteMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/importlist/${id}`),
    onSuccess: () => {
      invalidateQueries(['/importlist']);
      showSuccess('Import list deleted');
    },
    onError: () => {
      showError('Failed to delete import list');
    },
  });

  // Dialog state
  private dialogMode = signal<DialogMode>('closed');
  private schemas = signal<ImportListSchema[]>([]);
  private schemasLoading = signal(false);
  private selectedSchema = signal<ImportListSchema | null>(null);
  private editingId = signal<number | null>(null);
  private formData = signal<Record<string, unknown>>({});
  private isSaving = signal(false);
  private isTesting = signal(false);
  private testResult = signal<{ success: boolean; message: string } | null>(null);

  protected onInit(): void {
    this.watch(this.listsQuery.data);
    this.watch(this.listsQuery.isLoading);
    this.watch(this.profilesQuery.data);
    this.watch(this.rootFoldersQuery.data);
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
    const lists = this.listsQuery.data.value ?? [];
    const isLoading = this.listsQuery.isLoading.value;
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
          <h2 class="section-title">Import Lists</h2>
          <button class="add-btn" onclick="this.closest('import-lists-settings').handleAdd()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add List
          </button>
        </div>

        ${lists.length === 0 ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <line x1="8" y1="6" x2="21" y2="6"></line>
              <line x1="8" y1="12" x2="21" y2="12"></line>
              <line x1="8" y1="18" x2="21" y2="18"></line>
              <line x1="3" y1="6" x2="3.01" y2="6"></line>
              <line x1="3" y1="12" x2="3.01" y2="12"></line>
              <line x1="3" y1="18" x2="3.01" y2="18"></line>
            </svg>
            <p>No import lists configured</p>
            <p class="hint">Add lists from Trakt, IMDb, or other sources to automatically import series</p>
          </div>
        ` : html`
          <div class="lists-grid">
            ${lists.map((list) => html`
              <div class="list-card ${list.enableAutomaticAdd ? '' : 'disabled'}">
                <div class="list-header">
                  <div class="list-icon">
                    ${this.getListTypeIcon(list.listType || list.implementation)}
                  </div>
                  <div class="list-info">
                    <div class="list-name">${escapeHtml(list.name)}</div>
                    <div class="list-type">${escapeHtml(list.implementationName)}</div>
                  </div>
                </div>
                <div class="list-features">
                  ${list.enableAutomaticAdd ? html`
                    <span class="feature enabled">Auto Add</span>
                  ` : html`
                    <span class="feature disabled">Disabled</span>
                  `}
                  ${list.searchForMissingEpisodes ? html`
                    <span class="feature enabled">Search</span>
                  ` : ''}
                </div>
                <div class="list-actions">
                  <button class="action-btn" onclick="event.stopPropagation(); this.closest('import-lists-settings').handleTest(${list.id})" title="Test">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <polygon points="5 3 19 12 5 21 5 3"></polygon>
                    </svg>
                  </button>
                  <button class="action-btn" onclick="event.stopPropagation(); this.closest('import-lists-settings').handleEdit(${list.id})" title="Edit">
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"></path>
                      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"></path>
                    </svg>
                  </button>
                  <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('import-lists-settings').handleDelete(${list.id})" title="Delete">
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

    // Group schemas by listType
    const grouped = schemas.reduce((acc, schema) => {
      const type = schema.listType || 'other';
      if (!acc[type]) acc[type] = [];
      acc[type].push(schema);
      return acc;
    }, {} as Record<string, ImportListSchema[]>);

    const groupLabels: Record<string, string> = {
      trakt: 'Trakt',
      imdb: 'IMDb',
      plex: 'Plex',
      pir9: 'pir9',
      simkl: 'Simkl',
      other: 'Other',
    };

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('import-lists-settings').closeDialog()">
        <div class="dialog">
          <div class="dialog-header">
            <h2>Add Import List</h2>
            <button class="close-btn" onclick="this.closest('import-lists-settings').closeDialog()">
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
                <p>Loading available list types...</p>
              </div>
            ` : html`
              <div class="info-box">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="16" x2="12" y2="12"></line>
                  <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
                <span>Select a list source to configure</span>
              </div>

              ${Object.entries(grouped).map(([type, typeSchemas]) => html`
                <div class="provider-section">
                  <h3 class="provider-section-title">${groupLabels[type] || type}</h3>
                  <div class="provider-grid">
                    ${typeSchemas.map((schema) => this.renderSchemaCard(schema)).join('')}
                  </div>
                </div>
              `).join('')}
            `}
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('import-lists-settings').closeDialog()">
              Cancel
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderSchemaCard(schema: ImportListSchema): string {
    return html`
      <button class="provider-card" onclick="this.closest('import-lists-settings').selectSchema('${escapeHtml(schema.implementation)}')">
        <div class="provider-icon">
          ${this.getListTypeIcon(schema.listType || schema.implementation)}
        </div>
        <span class="provider-name">${escapeHtml(schema.implementationName)}</span>
      </button>
    `;
  }

  private getListTypeIcon(listType: string): string {
    const type = listType.toLowerCase();

    if (type.includes('trakt')) {
      return `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M12 24C5.385 24 0 18.615 0 12S5.385 0 12 0s12 5.385 12 12-5.385 12-12 12zm0-22.557C6.18 1.443 1.443 6.18 1.443 12S6.18 22.557 12 22.557 22.557 17.82 22.557 12 17.82 1.443 12 1.443zm-.755 4.93l-6.066 6.066a.72.72 0 0 0 0 1.02l6.066 6.066a.72.72 0 0 0 1.02 0l6.066-6.066a.72.72 0 0 0 0-1.02l-6.066-6.066a.72.72 0 0 0-1.02 0zm.51 10.123L6.5 12l5.255-4.496 5.255 4.496-5.255 4.496z"/></svg>`;
    }

    if (type.includes('imdb')) {
      return `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M14.31 9.588v.005c-.077-.048-.227-.07-.42-.07v4.815c.27 0 .44-.06.5-.165.062-.104.095-.405.095-.9v-2.61c0-.405-.025-.67-.078-.795-.053-.12-.147-.215-.3-.28h.003zm-1.932-.09H12v5h.378v-4.25l-.45 4.25h.27l.432-4.25V14.5h.338v-5h-.377c-.053.075-.15.474-.285 1.19l-.22-1.19h-.27l-.22 1.19c-.135-.716-.232-1.115-.285-1.19h-.377v5h.338V9.498l.432 4.25h.27l-.432-4.25zm7.222-.32l-.003-.003H22l-.003.003V14.5h-2.405l.003.003V9.175l.005-.003zm-1.35 4.065c-.133.15-.33.235-.534.23h-.27v-3.9h.27c.204-.005.4.08.534.23.143.1.215.36.215.77v1.925c0 .405-.072.66-.215.755v-.01zm-3.76-4.545v7.99c.82.01 1.64 0 2.457 0 .17 0 .343 0 .51-.005.28-.005.548-.084.783-.23.26-.155.46-.38.58-.645.12-.26.18-.63.18-1.11V11.05c0-.48-.06-.85-.18-1.11-.12-.26-.32-.49-.58-.645-.235-.146-.503-.225-.783-.23-.225 0-.45 0-.67-.005-.56 0-1.12 0-1.68-.006-.205 0-.41.006-.617.006V8.7zm-6.64-.002v7.989h1.47V12.62h1.14v4.065h1.48V8.699H9.82v3h-1.14V8.7H7.214l-.003-.002zm-4.37 0l.89 7.987h1.83l.88-7.989H4.72l-.4 4.68-.39-4.68H2.846l-.002.002z"/></svg>`;
    }

    if (type.includes('plex')) {
      return `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M11.643 0H4.68l7.679 12L4.68 24h6.963l7.677-12z"/></svg>`;
    }

    if (type.includes('pir9')) {
      return `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/></svg>`;
    }

    if (type.includes('simkl')) {
      return `<svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor"><path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm4.64 6.8c-.15 1.58-.8 5.42-1.13 7.19-.14.75-.42 1-.68 1.03-.58.05-1.02-.38-1.58-.75-.88-.58-1.38-.94-2.23-1.5-.99-.65-.35-1.01.22-1.59.15-.15 2.71-2.48 2.76-2.69a.2.2 0 0 0-.05-.18c-.06-.05-.14-.03-.21-.02-.09.02-1.49.95-4.22 2.79-.4.27-.76.41-1.08.4-.36-.01-1.04-.2-1.55-.37-.63-.2-1.12-.31-1.08-.66.02-.18.27-.36.74-.55 2.92-1.27 4.86-2.11 5.83-2.51 2.78-1.16 3.35-1.36 3.73-1.36.08 0 .27.02.39.12.1.08.13.19.14.27-.01.06.01.24 0 .38z"/></svg>`;
    }

    // Default list icon
    return `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
      <line x1="8" y1="6" x2="21" y2="6"></line>
      <line x1="8" y1="12" x2="21" y2="12"></line>
      <line x1="8" y1="18" x2="21" y2="18"></line>
      <line x1="3" y1="6" x2="3.01" y2="6"></line>
      <line x1="3" y1="12" x2="3.01" y2="12"></line>
      <line x1="3" y1="18" x2="3.01" y2="18"></line>
    </svg>`;
  }

  private renderEditDialog(): string {
    const schema = this.selectedSchema.value;
    if (!schema) return '';

    const data = this.formData.value;
    const isSaving = this.isSaving.value;
    const isTesting = this.isTesting.value;
    const testResult = this.testResult.value;
    const isEditing = this.editingId.value !== null;
    const profiles = this.profilesQuery.data.value ?? [];
    const rootFolders = this.rootFoldersQuery.data.value ?? [];

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('import-lists-settings').closeDialog()">
        <div class="dialog dialog-form">
          <div class="dialog-header">
            <h2>${isEditing ? 'Edit' : 'Add'} - ${escapeHtml(schema.implementationName)}</h2>
            <button class="close-btn" onclick="this.closest('import-lists-settings').closeDialog()">
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
                  onchange="this.closest('import-lists-settings').updateField('name', this.value)"
                />
              </div>

              ${schema.fields
                .filter(f => f.hidden !== 'hidden')
                .sort((a, b) => a.order - b.order)
                .map((field) => this.renderField(field, data))
                .join('')}

              <fieldset class="form-fieldset">
                <legend>Series Settings</legend>

                <div class="form-group">
                  <label for="qualityProfileId">Quality Profile</label>
                  <select id="qualityProfileId" onchange="this.closest('import-lists-settings').updateField('qualityProfileId', parseInt(this.value))">
                    ${profiles.map(p => html`
                      <option value="${p.id}" ${data.qualityProfileId === p.id ? 'selected' : ''}>${escapeHtml(p.name)}</option>
                    `).join('')}
                  </select>
                </div>

                <div class="form-group">
                  <label for="rootFolderPath">Root Folder</label>
                  <select id="rootFolderPath" onchange="this.closest('import-lists-settings').updateField('rootFolderPath', this.value)">
                    ${rootFolders.map(f => html`
                      <option value="${f.path}" ${data.rootFolderPath === f.path ? 'selected' : ''}>${escapeHtml(f.path)}</option>
                    `).join('')}
                  </select>
                </div>

                <div class="form-group">
                  <label for="seriesType">Series Type</label>
                  <select id="seriesType" onchange="this.closest('import-lists-settings').updateField('seriesType', this.value)">
                    <option value="standard" ${data.seriesType === 'standard' ? 'selected' : ''}>Standard</option>
                    <option value="daily" ${data.seriesType === 'daily' ? 'selected' : ''}>Daily</option>
                    <option value="anime" ${data.seriesType === 'anime' ? 'selected' : ''}>Anime</option>
                  </select>
                </div>

                <div class="form-group">
                  <label for="shouldMonitor">Monitor</label>
                  <select id="shouldMonitor" onchange="this.closest('import-lists-settings').updateField('shouldMonitor', this.value)">
                    <option value="all" ${data.shouldMonitor === 'all' ? 'selected' : ''}>All Episodes</option>
                    <option value="future" ${data.shouldMonitor === 'future' ? 'selected' : ''}>Future Episodes</option>
                    <option value="missing" ${data.shouldMonitor === 'missing' ? 'selected' : ''}>Missing Episodes</option>
                    <option value="existing" ${data.shouldMonitor === 'existing' ? 'selected' : ''}>Existing Episodes</option>
                    <option value="firstSeason" ${data.shouldMonitor === 'firstSeason' ? 'selected' : ''}>First Season</option>
                    <option value="lastSeason" ${data.shouldMonitor === 'lastSeason' ? 'selected' : ''}>Last Season</option>
                    <option value="latestSeason" ${data.shouldMonitor === 'latestSeason' ? 'selected' : ''}>Latest Season</option>
                    <option value="pilot" ${data.shouldMonitor === 'pilot' ? 'selected' : ''}>Pilot Episode</option>
                    <option value="none" ${data.shouldMonitor === 'none' ? 'selected' : ''}>None</option>
                  </select>
                </div>

                <div class="form-group form-group-checkbox">
                  <label>
                    <input type="checkbox" ${data.seasonFolder ? 'checked' : ''} onchange="this.closest('import-lists-settings').updateField('seasonFolder', this.checked)" />
                    <span>Season Folder</span>
                  </label>
                  <p class="help-text">Use season folders for series from this list</p>
                </div>
              </fieldset>

              <fieldset class="form-fieldset">
                <legend>Import Options</legend>

                <div class="form-group form-group-checkbox">
                  <label>
                    <input type="checkbox" ${data.enableAutomaticAdd ? 'checked' : ''} onchange="this.closest('import-lists-settings').updateField('enableAutomaticAdd', this.checked)" />
                    <span>Enable Automatic Add</span>
                  </label>
                  <p class="help-text">Automatically add series from this list</p>
                </div>

                <div class="form-group form-group-checkbox">
                  <label>
                    <input type="checkbox" ${data.searchForMissingEpisodes ? 'checked' : ''} onchange="this.closest('import-lists-settings').updateField('searchForMissingEpisodes', this.checked)" />
                    <span>Search for Missing Episodes</span>
                  </label>
                  <p class="help-text">Search for missing episodes when series are added</p>
                </div>
              </fieldset>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-default" onclick="this.closest('import-lists-settings').handleTestConnection()" ${isTesting ? 'disabled' : ''}>
              ${isTesting ? 'Testing...' : 'Test'}
            </button>
            <div class="footer-spacer"></div>
            <button class="btn btn-secondary" onclick="this.closest('import-lists-settings').closeDialog()">
              Cancel
            </button>
            <button class="btn btn-primary" onclick="this.closest('import-lists-settings').handleSave()" ${isSaving ? 'disabled' : ''}>
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
            <input type="text" id="${fieldId}" value="${escapeHtml(String(value ?? ''))}" placeholder="${escapeHtml(field.placeholder || '')}" onchange="this.closest('import-lists-settings').updateField('${field.name}', this.value)" />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'password':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input type="password" id="${fieldId}" value="${escapeHtml(String(value ?? ''))}" onchange="this.closest('import-lists-settings').updateField('${field.name}', this.value)" />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'number':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input type="number" id="${fieldId}" value="${value ?? ''}" onchange="this.closest('import-lists-settings').updateField('${field.name}', ${field.isFloat ? 'parseFloat(this.value)' : 'parseInt(this.value)'})" />
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'checkbox':
        return html`
          <div class="form-group form-group-checkbox">
            <label>
              <input type="checkbox" ${value ? 'checked' : ''} onchange="this.closest('import-lists-settings').updateField('${field.name}', this.checked)" />
              <span>${escapeHtml(field.label)}</span>
            </label>
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      case 'select':
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <select id="${fieldId}" onchange="this.closest('import-lists-settings').updateField('${field.name}', this.value)">
              ${(field.selectOptions || []).map((opt) => html`
                <option value="${opt.value}" ${String(value) === String(opt.value) ? 'selected' : ''}>${escapeHtml(opt.name)}</option>
              `).join('')}
            </select>
            ${field.helpText ? html`<p class="help-text">${escapeHtml(field.helpText)}</p>` : ''}
          </div>
        `;

      default:
        return html`
          <div class="form-group">
            <label for="${fieldId}">${escapeHtml(field.label)}</label>
            <input type="text" id="${fieldId}" value="${escapeHtml(String(value ?? ''))}" onchange="this.closest('import-lists-settings').updateField('${field.name}', this.value)" />
          </div>
        `;
    }
  }

  // Public methods called from template
  async handleAdd(): Promise<void> {
    this.dialogMode.set('select');
    this.schemasLoading.set(true);

    try {
      const schemas = await httpV3.get<ImportListSchema[]>('/importlist/schema');
      this.schemas.set(schemas);
    } catch {
      showError('Failed to load list types');
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

    const profiles = this.profilesQuery.data.value ?? [];
    const rootFolders = this.rootFoldersQuery.data.value ?? [];

    // Initialize form data with defaults
    const data: Record<string, unknown> = {
      name: schema.implementationName,
      enableAutomaticAdd: true,
      searchForMissingEpisodes: true,
      shouldMonitor: 'all',
      qualityProfileId: profiles[0]?.id ?? 1,
      rootFolderPath: rootFolders[0]?.path ?? '',
      seriesType: 'standard',
      seasonFolder: true,
    };
    schema.fields.forEach(f => {
      data[f.name] = f.value;
    });
    this.formData.set(data);

    this.dialogMode.set('edit');
  }

  async handleEdit(id: number): Promise<void> {
    try {
      const list = await httpV3.get<ImportList>(`/importlist/${id}`);
      const schemas = await httpV3.get<ImportListSchema[]>('/importlist/schema');
      const schema = schemas.find(s => s.implementation === list.implementation);

      if (!schema) {
        showError('Unknown list type');
        return;
      }

      // Merge schema field definitions with list values
      const mergedSchema: ImportListSchema = {
        ...schema,
        fields: schema.fields.map(f => ({
          ...f,
          value: list.fields.find(lf => lf.name === f.name)?.value ?? f.value,
        })),
      };

      this.schemas.set(schemas);
      this.selectedSchema.set(mergedSchema);
      this.editingId.set(id);
      this.testResult.set(null);

      // Initialize form data from list
      const data: Record<string, unknown> = {
        name: list.name,
        enableAutomaticAdd: list.enableAutomaticAdd,
        searchForMissingEpisodes: list.searchForMissingEpisodes,
        shouldMonitor: list.shouldMonitor,
        qualityProfileId: list.qualityProfileId,
        rootFolderPath: list.rootFolderPath,
        seriesType: list.seriesType,
        seasonFolder: list.seasonFolder,
      };
      mergedSchema.fields.forEach(f => {
        data[f.name] = f.value;
      });
      this.formData.set(data);

      this.dialogMode.set('edit');
    } catch {
      showError('Failed to load import list');
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
      const response = await fetch('/api/v3/importlist/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        this.testResult.set({ success: true, message: result.message || 'Connection successful!' });
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
      const url = id ? `/api/v3/importlist/${id}` : '/api/v3/importlist';

      const response = await fetch(url, {
        method,
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const error = await response.json();
        throw new Error(error.message || 'Failed to save');
      }

      invalidateQueries(['/importlist']);
      showSuccess('Import list saved');
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
      implementation: schema.implementation,
      implementationName: schema.implementationName,
      configContract: schema.configContract,
      fields,
      tags: [],
      enableAutomaticAdd: data.enableAutomaticAdd,
      searchForMissingEpisodes: data.searchForMissingEpisodes,
      shouldMonitor: data.shouldMonitor,
      qualityProfileId: data.qualityProfileId,
      rootFolderPath: data.rootFolderPath,
      seriesType: data.seriesType,
      seasonFolder: data.seasonFolder,
      listType: schema.listType || '',
      listOrder: 1,
      minRefreshInterval: 'PT12H',
    };
  }

  async handleTest(id: number): Promise<void> {
    try {
      const list = await httpV3.get<ImportList>(`/importlist/${id}`);
      const response = await fetch('/api/v3/importlist/test', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(list),
      });

      const result = await response.json();

      if (response.ok && result.isValid !== false) {
        showSuccess(result.message || 'Connection successful!');
      } else {
        showError(result.message || 'Test failed');
      }
    } catch {
      showError('Test failed');
    }
  }

  handleDelete(id: number): void {
    if (confirm('Are you sure you want to delete this import list?')) {
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

      .lists-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
        gap: 1rem;
      }

      .list-card {
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
        padding: 1rem;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        transition: border-color 0.15s;
      }

      .list-card:hover {
        border-color: var(--color-primary);
      }

      .list-card.disabled {
        opacity: 0.6;
      }

      .list-header {
        display: flex;
        align-items: center;
        gap: 0.75rem;
      }

      .list-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 40px;
        height: 40px;
        background-color: var(--bg-card);
        border-radius: 0.375rem;
        color: var(--color-primary);
      }

      .list-icon svg {
        width: 24px;
        height: 24px;
      }

      .list-info {
        flex: 1;
      }

      .list-name {
        font-weight: 500;
        margin-bottom: 0.125rem;
      }

      .list-type {
        font-size: 0.75rem;
        color: var(--text-color-muted);
      }

      .list-features {
        display: flex;
        flex-wrap: wrap;
        gap: 0.25rem;
      }

      .feature {
        font-size: 0.625rem;
        padding: 0.125rem 0.375rem;
        border-radius: 0.25rem;
        text-transform: uppercase;
        font-weight: 500;
      }

      .feature.enabled {
        background-color: var(--color-success);
        color: var(--color-white);
      }

      .feature.disabled {
        background-color: var(--color-warning);
        color: var(--color-white);
      }

      .list-actions {
        display: flex;
        gap: 0.25rem;
        justify-content: flex-end;
        padding-top: 0.5rem;
        border-top: 1px solid var(--border-color);
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

      .provider-section {
        margin-bottom: 1.5rem;
      }

      .provider-section-title {
        font-size: 0.875rem;
        font-weight: 500;
        color: var(--text-color-muted);
        margin: 0 0 0.75rem 0;
        text-transform: uppercase;
        letter-spacing: 0.05em;
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
