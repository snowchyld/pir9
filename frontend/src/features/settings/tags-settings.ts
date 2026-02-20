/**
 * Tags Settings page
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { httpV3 } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';
import { signal } from '../../core/reactive';

interface Tag {
  id: number;
  label: string;
}

interface TagDetails extends Tag {
  seriesIds: number[];
  notificationIds: number[];
  restrictionIds: number[];
  indexerIds: number[];
  downloadClientIds: number[];
  autoTagIds: number[];
  importListIds: number[];
}

type DialogMode = 'closed' | 'add' | 'edit';

@customElement('tags-settings')
export class TagsSettings extends BaseComponent {
  private tagsQuery = createQuery({
    queryKey: ['/tag/detail'],
    queryFn: () => httpV3.get<TagDetails[]>('/tag/detail'),
  });

  private deleteMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/tag/${id}`),
    onSuccess: () => {
      invalidateQueries(['/tag/detail']);
      showSuccess('Tag deleted');
    },
    onError: () => {
      showError('Failed to delete tag');
    },
  });

  // Dialog state
  private dialogMode = signal<DialogMode>('closed');
  private editingId = signal<number | null>(null);
  private isSaving = signal(false);
  private labelInput = signal('');

  protected onInit(): void {
    this.watch(this.tagsQuery.data);
    this.watch(this.tagsQuery.isLoading);
    this.watch(this.dialogMode);
    this.watch(this.labelInput);
    this.watch(this.isSaving);
  }

  protected template(): string {
    const tags = this.tagsQuery.data.value ?? [];
    const isLoading = this.tagsQuery.isLoading.value;
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
          <h2 class="section-title">Tags</h2>
          <button class="add-btn" onclick="this.closest('tags-settings').handleAdd()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Tag
          </button>
        </div>

        ${tags.length === 0 ? html`
          <div class="empty-state">
            <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" color="var(--text-color-muted)">
              <path d="M20.59 13.41l-7.17 7.17a2 2 0 0 1-2.83 0L2 12V2h10l8.59 8.59a2 2 0 0 1 0 2.82z"></path>
              <line x1="7" y1="7" x2="7.01" y2="7"></line>
            </svg>
            <p>No tags created</p>
            <p class="hint">Create tags to organize series, restrict indexers, and more</p>
          </div>
        ` : html`
          <table class="tags-table">
            <thead>
              <tr>
                <th>Tag</th>
                <th class="num-col">Series</th>
                <th class="num-col">Notifications</th>
                <th class="num-col">Indexers</th>
                <th class="num-col">Download Clients</th>
                <th class="num-col">Import Lists</th>
                <th class="action-col"></th>
              </tr>
            </thead>
            <tbody>
              ${tags.map((tag) => html`
                <tr onclick="this.closest('tags-settings').handleEdit(${tag.id})">
                  <td>
                    <span class="tag-badge">${escapeHtml(tag.label)}</span>
                  </td>
                  <td class="num-col">${tag.seriesIds?.length || 0}</td>
                  <td class="num-col">${tag.notificationIds?.length || 0}</td>
                  <td class="num-col">${tag.indexerIds?.length || 0}</td>
                  <td class="num-col">${tag.downloadClientIds?.length || 0}</td>
                  <td class="num-col">${tag.importListIds?.length || 0}</td>
                  <td class="action-col">
                    <button
                      class="action-btn danger"
                      onclick="event.stopPropagation(); this.closest('tags-settings').handleDelete(${tag.id})"
                      title="Delete"
                    >
                      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                        <polyline points="3 6 5 6 21 6"></polyline>
                        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                      </svg>
                    </button>
                  </td>
                </tr>
              `).join('')}
            </tbody>
          </table>
        `}
      </div>

      ${mode !== 'closed' ? this.renderDialog() : ''}

      ${safeHtml(this.styles())}
    `;
  }

  private renderDialog(): string {
    const mode = this.dialogMode.value;
    const label = this.labelInput.value;
    const isSaving = this.isSaving.value;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('tags-settings').closeDialog()">
        <div class="dialog dialog-small">
          <div class="dialog-header">
            <h2>${mode === 'add' ? 'Add' : 'Edit'} Tag</h2>
            <button class="close-btn" onclick="this.closest('tags-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            <form class="tag-form" onsubmit="event.preventDefault(); this.closest('tags-settings').handleSave()">
              <div class="form-group">
                <label for="tag-label">Label</label>
                <input
                  type="text"
                  id="tag-label"
                  value="${escapeHtml(label)}"
                  onchange="this.closest('tags-settings').updateLabel(this.value)"
                  oninput="this.closest('tags-settings').updateLabel(this.value)"
                  placeholder="Enter tag name"
                  autofocus
                />
                <p class="help-text">A short, descriptive name for this tag</p>
              </div>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('tags-settings').closeDialog()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('tags-settings').handleSave()"
              ${isSaving || !label.trim() ? 'disabled' : ''}
            >
              ${isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>
    `;
  }

  handleAdd(): void {
    this.labelInput.set('');
    this.editingId.set(null);
    this.dialogMode.set('add');
  }

  async handleEdit(id: number): Promise<void> {
    const tags = this.tagsQuery.data.value ?? [];
    const tag = tags.find(t => t.id === id);
    if (!tag) return;

    this.labelInput.set(tag.label);
    this.editingId.set(id);
    this.dialogMode.set('edit');
  }

  updateLabel(value: string): void {
    this.labelInput.set(value);
  }

  async handleSave(): Promise<void> {
    const label = this.labelInput.value.trim();
    if (!label) return;

    this.isSaving.set(true);

    try {
      const payload: Tag = {
        id: this.editingId.value || 0,
        label,
      };

      const id = this.editingId.value;
      if (id) {
        await httpV3.put(`/tag/${id}`, payload);
      } else {
        await httpV3.post('/tag', payload);
      }

      invalidateQueries(['/tag/detail']);
      showSuccess('Tag saved');
      this.closeDialog();
    } catch {
      showError('Failed to save tag');
    } finally {
      this.isSaving.set(false);
    }
  }

  handleDelete(id: number): void {
    const tags = this.tagsQuery.data.value ?? [];
    const tag = tags.find(t => t.id === id);

    // Check if tag is in use
    if (tag) {
      const usageCount =
        (tag.seriesIds?.length || 0) +
        (tag.notificationIds?.length || 0) +
        (tag.indexerIds?.length || 0) +
        (tag.downloadClientIds?.length || 0) +
        (tag.importListIds?.length || 0);

      if (usageCount > 0) {
        if (!confirm(`This tag is in use by ${usageCount} item(s). Are you sure you want to delete it?`)) {
          return;
        }
      } else if (!confirm('Are you sure you want to delete this tag?')) {
        return;
      }
    }

    this.deleteMutation.mutate(id);
  }

  closeDialog(): void {
    this.dialogMode.set('closed');
    this.editingId.set(null);
    this.labelInput.set('');
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

      .tags-table {
        width: 100%;
        border-collapse: collapse;
        font-size: 0.875rem;
      }

      .tags-table th,
      .tags-table td {
        padding: 0.75rem;
        text-align: left;
        border-bottom: 1px solid var(--border-color);
      }

      .tags-table th {
        font-weight: 600;
        color: var(--text-color-muted);
        background-color: var(--bg-card-alt);
      }

      .tags-table tbody tr {
        cursor: pointer;
        transition: background-color 0.15s;
      }

      .tags-table tbody tr:hover {
        background-color: var(--bg-card-alt);
      }

      .num-col {
        text-align: center !important;
        width: 100px;
      }

      .action-col {
        width: 50px;
        text-align: center !important;
      }

      .tag-badge {
        display: inline-flex;
        padding: 0.25rem 0.5rem;
        background-color: var(--color-primary);
        color: var(--color-white);
        border-radius: 0.25rem;
        font-size: 0.75rem;
        font-weight: 500;
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
        max-width: 400px;
        max-height: 90vh;
        display: flex;
        flex-direction: column;
        box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
      }

      .dialog-small {
        max-width: 350px;
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

      .form-group {
        display: flex;
        flex-direction: column;
        gap: 0.375rem;
      }

      .form-group label {
        font-size: 0.875rem;
        font-weight: 500;
      }

      .form-group input[type="text"] {
        padding: 0.5rem 0.75rem;
        background-color: var(--bg-input);
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        color: var(--text-color);
        font-size: 0.875rem;
      }

      .form-group input:focus {
        outline: none;
        border-color: var(--color-primary);
        box-shadow: 0 0 0 2px rgba(93, 156, 236, 0.2);
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
