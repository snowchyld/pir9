/**
 * Profiles Settings page - Quality Profiles & Release Profiles
 */

import { BaseComponent, customElement, html, escapeHtml, safeHtml } from '../../core/component';
import { createQuery, createMutation, invalidateQueries } from '../../core/query';
import { httpV3 } from '../../core/http';
import { showSuccess, showError } from '../../stores/app.store';
import { signal } from '../../core/reactive';

interface QualityItem {
  id?: number;
  name?: string;
  quality?: { id: number; name: string; source: string; resolution: number };
  items: QualityItem[];
  allowed: boolean;
}

interface QualityProfile {
  id: number;
  name: string;
  upgradeAllowed: boolean;
  cutoff: number;
  items: QualityItem[];
  minFormatScore: number;
  cutoffFormatScore: number;
  formatItems: Array<{ format: number; name: string; score: number }>;
}

interface ReleaseProfile {
  id: number;
  name: string | null;
  enabled: boolean;
  required: string[];
  ignored: string[];
  indexerId: number;
  tags: number[];
}

type DialogMode = 'closed' | 'quality-edit' | 'release-edit';

@customElement('profiles-settings')
export class ProfilesSettings extends BaseComponent {
  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => httpV3.get<QualityProfile[]>('/qualityprofile'),
  });

  private releaseProfilesQuery = createQuery({
    queryKey: ['/releaseprofile'],
    queryFn: () => httpV3.get<ReleaseProfile[]>('/releaseprofile'),
  });

  private deleteQualityMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/qualityprofile/${id}`),
    onSuccess: () => {
      invalidateQueries(['/qualityprofile']);
      showSuccess('Quality profile deleted');
    },
    onError: () => showError('Failed to delete quality profile'),
  });

  private deleteReleaseMutation = createMutation({
    mutationFn: (id: number) => httpV3.delete<void>(`/releaseprofile/${id}`),
    onSuccess: () => {
      invalidateQueries(['/releaseprofile']);
      showSuccess('Release profile deleted');
    },
    onError: () => showError('Failed to delete release profile'),
  });

  // Dialog state
  private dialogMode = signal<DialogMode>('closed');
  private editingId = signal<number | null>(null);
  private isSaving = signal(false);

  // Quality profile form
  private qualityFormData = signal<{
    name: string;
    upgradeAllowed: boolean;
    cutoff: number;
    items: QualityItem[];
  }>({
    name: '',
    upgradeAllowed: true,
    cutoff: 1,
    items: [],
  });

  // Release profile form
  private releaseFormData = signal<{
    name: string;
    enabled: boolean;
    required: string;
    ignored: string;
  }>({
    name: '',
    enabled: true,
    required: '',
    ignored: '',
  });

  protected onInit(): void {
    this.watch(this.qualityProfilesQuery.data);
    this.watch(this.qualityProfilesQuery.isLoading);
    this.watch(this.releaseProfilesQuery.data);
    this.watch(this.releaseProfilesQuery.isLoading);
    this.watch(this.dialogMode);
    this.watch(this.qualityFormData);
    this.watch(this.releaseFormData);
    this.watch(this.isSaving);
  }

  protected template(): string {
    const qualityProfiles = this.qualityProfilesQuery.data.value ?? [];
    const releaseProfiles = this.releaseProfilesQuery.data.value ?? [];
    const isLoading = this.qualityProfilesQuery.isLoading.value || this.releaseProfilesQuery.isLoading.value;
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
          <h2 class="section-title">Quality Profiles</h2>
          <button class="add-btn" onclick="this.closest('profiles-settings').handleAddQualityProfile()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Profile
          </button>
        </div>

        <div class="profiles-grid">
          ${qualityProfiles.length === 0 ? html`
            <div class="empty-state">
              <p>No quality profiles configured</p>
            </div>
          ` : qualityProfiles.map((profile) => html`
            <div class="profile-card">
              <div class="profile-content" onclick="this.closest('profiles-settings').handleEditQualityProfile(${profile.id})">
                <div class="profile-name">${escapeHtml(profile.name)}</div>
                <div class="profile-meta">
                  <span class="meta-item">
                    ${profile.upgradeAllowed ? '✓ Upgrades allowed' : '✗ No upgrades'}
                  </span>
                  <span class="meta-item">${this.countAllowedQualities(profile.items)} qualities</span>
                </div>
              </div>
              <div class="profile-actions">
                <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('profiles-settings').handleDeleteQualityProfile(${profile.id})" title="Delete">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="3 6 5 6 21 6"></polyline>
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                  </svg>
                </button>
              </div>
            </div>
          `).join('')}
        </div>
      </div>

      <div class="settings-section">
        <div class="section-header">
          <h2 class="section-title">Release Profiles</h2>
          <button class="add-btn" onclick="this.closest('profiles-settings').handleAddReleaseProfile()">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
            Add Profile
          </button>
        </div>

        <div class="profiles-list">
          ${releaseProfiles.length === 0 ? html`
            <div class="empty-state">
              <p>No release profiles configured</p>
              <p class="hint">Release profiles let you filter releases by terms they must or must not contain</p>
            </div>
          ` : releaseProfiles.map((profile) => html`
            <div class="release-profile-row">
              <div class="profile-content" onclick="this.closest('profiles-settings').handleEditReleaseProfile(${profile.id})">
                <div class="profile-info">
                  <span class="profile-name">${escapeHtml(profile.name || 'Unnamed')}</span>
                  <span class="profile-status ${profile.enabled ? 'enabled' : 'disabled'}">
                    ${profile.enabled ? 'Enabled' : 'Disabled'}
                  </span>
                </div>
                <div class="profile-terms">
                  ${profile.required && profile.required.length > 0 ? html`
                    <span class="term-badge required">Must contain: ${profile.required.length} term${profile.required.length !== 1 ? 's' : ''}</span>
                  ` : ''}
                  ${profile.ignored && profile.ignored.length > 0 ? html`
                    <span class="term-badge ignored">Must not contain: ${profile.ignored.length} term${profile.ignored.length !== 1 ? 's' : ''}</span>
                  ` : ''}
                </div>
              </div>
              <div class="profile-actions">
                <button class="action-btn danger" onclick="event.stopPropagation(); this.closest('profiles-settings').handleDeleteReleaseProfile(${profile.id})" title="Delete">
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <polyline points="3 6 5 6 21 6"></polyline>
                    <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"></path>
                  </svg>
                </button>
              </div>
            </div>
          `).join('')}
        </div>
      </div>

      ${mode === 'quality-edit' ? this.renderQualityDialog() : ''}
      ${mode === 'release-edit' ? this.renderReleaseDialog() : ''}

      ${safeHtml(this.styles())}
    `;
  }

  private countAllowedQualities(items: QualityItem[]): number {
    let count = 0;
    for (const item of items) {
      if (item.quality && item.allowed) {
        count++;
      }
      if (item.items && item.items.length > 0) {
        count += this.countAllowedQualities(item.items);
      }
    }
    return count;
  }

  private renderQualityDialog(): string {
    const data = this.qualityFormData.value;
    const isSaving = this.isSaving.value;
    const isEditing = this.editingId.value !== null;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('profiles-settings').closeDialog()">
        <div class="dialog dialog-wide">
          <div class="dialog-header">
            <h2>${isEditing ? 'Edit' : 'Add'} Quality Profile</h2>
            <button class="close-btn" onclick="this.closest('profiles-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            <form class="quality-form" onsubmit="event.preventDefault()">
              <div class="form-row">
                <div class="form-group">
                  <label for="qp-name">Name</label>
                  <input
                    type="text"
                    id="qp-name"
                    value="${escapeHtml(data.name)}"
                    onchange="this.closest('profiles-settings').updateQualityField('name', this.value)"
                    placeholder="Profile name"
                  />
                </div>
              </div>

              <div class="form-row">
                <div class="form-group form-group-checkbox">
                  <label>
                    <input
                      type="checkbox"
                      ${data.upgradeAllowed ? 'checked' : ''}
                      onchange="this.closest('profiles-settings').updateQualityField('upgradeAllowed', this.checked)"
                    />
                    <span>Upgrade Allowed</span>
                  </label>
                  <p class="help-text">If upgrades are disabled, qualities will be considered equal</p>
                </div>
              </div>

              ${data.upgradeAllowed ? html`
                <div class="form-row">
                  <div class="form-group">
                    <label for="qp-cutoff">Upgrade Until</label>
                    <select
                      id="qp-cutoff"
                      onchange="this.closest('profiles-settings').updateQualityField('cutoff', parseInt(this.value))"
                    >
                      ${this.renderCutoffOptions(data.items, data.cutoff)}
                    </select>
                    <p class="help-text">Once this quality is reached, no further upgrades will be attempted</p>
                  </div>
                </div>
              ` : ''}

              <div class="qualities-section">
                <h3>Qualities</h3>
                <p class="help-text">Check the qualities you want to download. Drag to reorder priority (top = highest).</p>
                <div class="qualities-list">
                  ${this.renderQualityItems(data.items)}
                </div>
              </div>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('profiles-settings').closeDialog()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('profiles-settings').handleSaveQuality()"
              ${isSaving ? 'disabled' : ''}
            >
              ${isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>
    `;
  }

  private renderCutoffOptions(items: QualityItem[], currentCutoff: number): string {
    const options: string[] = [];

    const addOptions = (items: QualityItem[], depth: number = 0) => {
      for (const item of items) {
        if (item.allowed) {
          if (item.quality) {
            const prefix = depth > 0 ? '&nbsp;&nbsp;' : '';
            options.push(html`
              <option value="${item.quality.id}" ${item.quality.id === currentCutoff ? 'selected' : ''}>
                ${prefix}${escapeHtml(item.quality.name)}
              </option>
            `);
          }
          if (item.name && item.items && item.items.length > 0) {
            options.push(html`
              <option value="${item.id || 0}" ${item.id === currentCutoff ? 'selected' : ''}>
                ${escapeHtml(item.name)} (Group)
              </option>
            `);
          }
        }
        if (item.items && item.items.length > 0) {
          addOptions(item.items, depth + 1);
        }
      }
    };

    addOptions(items);
    return options.join('');
  }

  private renderQualityItems(items: QualityItem[]): string {
    return items.map((item, index) => {
      const name = item.quality?.name || item.name || 'Unknown';
      const isGroup = item.items && item.items.length > 0 && !item.quality;

      return html`
        <div class="quality-item ${isGroup ? 'quality-group' : ''}">
          <div class="quality-row">
            <label class="quality-checkbox">
              <input
                type="checkbox"
                ${item.allowed ? 'checked' : ''}
                onchange="this.closest('profiles-settings').toggleQuality(${index}, this.checked)"
              />
              <span class="quality-name">${escapeHtml(name)}</span>
              ${item.quality ? html`
                <span class="quality-resolution">${item.quality.resolution}p</span>
              ` : ''}
            </label>
          </div>
          ${isGroup ? html`
            <div class="quality-group-items">
              ${item.items.map((subItem, subIndex) => html`
                <div class="quality-row sub-item">
                  <label class="quality-checkbox">
                    <input
                      type="checkbox"
                      ${subItem.allowed ? 'checked' : ''}
                      onchange="this.closest('profiles-settings').toggleSubQuality(${index}, ${subIndex}, this.checked)"
                    />
                    <span class="quality-name">${escapeHtml(subItem.quality?.name || 'Unknown')}</span>
                    ${subItem.quality ? html`
                      <span class="quality-resolution">${subItem.quality.resolution}p</span>
                    ` : ''}
                  </label>
                </div>
              `).join('')}
            </div>
          ` : ''}
        </div>
      `;
    }).join('');
  }

  private renderReleaseDialog(): string {
    const data = this.releaseFormData.value;
    const isSaving = this.isSaving.value;
    const isEditing = this.editingId.value !== null;

    return html`
      <div class="dialog-backdrop" onclick="if(event.target.classList.contains('dialog-backdrop')) this.closest('profiles-settings').closeDialog()">
        <div class="dialog">
          <div class="dialog-header">
            <h2>${isEditing ? 'Edit' : 'Add'} Release Profile</h2>
            <button class="close-btn" onclick="this.closest('profiles-settings').closeDialog()">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
          <div class="dialog-body">
            <form class="release-form" onsubmit="event.preventDefault()">
              <div class="form-group">
                <label for="rp-name">Name</label>
                <input
                  type="text"
                  id="rp-name"
                  value="${escapeHtml(data.name)}"
                  onchange="this.closest('profiles-settings').updateReleaseField('name', this.value)"
                  placeholder="Profile name"
                />
              </div>

              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.enabled ? 'checked' : ''}
                    onchange="this.closest('profiles-settings').updateReleaseField('enabled', this.checked)"
                  />
                  <span>Enabled</span>
                </label>
              </div>

              <div class="form-group">
                <label for="rp-required">Must Contain</label>
                <textarea
                  id="rp-required"
                  rows="4"
                  onchange="this.closest('profiles-settings').updateReleaseField('required', this.value)"
                  placeholder="Enter terms that releases must contain (one per line)"
                >${escapeHtml(data.required)}</textarea>
                <p class="help-text">Releases must contain at least one of these terms (one per line, regex supported)</p>
              </div>

              <div class="form-group">
                <label for="rp-ignored">Must Not Contain</label>
                <textarea
                  id="rp-ignored"
                  rows="4"
                  onchange="this.closest('profiles-settings').updateReleaseField('ignored', this.value)"
                  placeholder="Enter terms that releases must NOT contain (one per line)"
                >${escapeHtml(data.ignored)}</textarea>
                <p class="help-text">Releases will be rejected if they contain any of these terms (one per line, regex supported)</p>
              </div>
            </form>
          </div>
          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('profiles-settings').closeDialog()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('profiles-settings').handleSaveRelease()"
              ${isSaving ? 'disabled' : ''}
            >
              ${isSaving ? 'Saving...' : 'Save'}
            </button>
          </div>
        </div>
      </div>
    `;
  }

  // Quality Profile handlers
  async handleAddQualityProfile(): Promise<void> {
    try {
      const schema = await httpV3.get<QualityProfile>('/qualityprofile/schema');
      this.qualityFormData.set({
        name: '',
        upgradeAllowed: true,
        cutoff: schema.cutoff,
        items: schema.items,
      });
      this.editingId.set(null);
      this.dialogMode.set('quality-edit');
    } catch {
      showError('Failed to load quality profile schema');
    }
  }

  async handleEditQualityProfile(id: number): Promise<void> {
    try {
      const profile = await httpV3.get<QualityProfile>(`/qualityprofile/${id}`);
      this.qualityFormData.set({
        name: profile.name,
        upgradeAllowed: profile.upgradeAllowed,
        cutoff: profile.cutoff,
        items: profile.items,
      });
      this.editingId.set(id);
      this.dialogMode.set('quality-edit');
    } catch {
      showError('Failed to load quality profile');
    }
  }

  updateQualityField(field: string, value: unknown): void {
    const current = this.qualityFormData.value;
    this.qualityFormData.set({ ...current, [field]: value });
  }

  toggleQuality(index: number, allowed: boolean): void {
    const current = this.qualityFormData.value;
    const items = [...current.items];
    items[index] = { ...items[index], allowed };
    // Also toggle children if it's a group
    if (items[index].items && items[index].items.length > 0) {
      items[index].items = items[index].items.map(sub => ({ ...sub, allowed }));
    }
    this.qualityFormData.set({ ...current, items });
  }

  toggleSubQuality(parentIndex: number, subIndex: number, allowed: boolean): void {
    const current = this.qualityFormData.value;
    const items = [...current.items];
    const subItems = [...items[parentIndex].items];
    subItems[subIndex] = { ...subItems[subIndex], allowed };
    items[parentIndex] = { ...items[parentIndex], items: subItems };
    // Update parent group state based on children
    const anyAllowed = subItems.some(s => s.allowed);
    items[parentIndex].allowed = anyAllowed;
    this.qualityFormData.set({ ...current, items });
  }

  async handleSaveQuality(): Promise<void> {
    this.isSaving.set(true);

    try {
      const data = this.qualityFormData.value;
      const payload: QualityProfile = {
        id: this.editingId.value || 0,
        name: data.name,
        upgradeAllowed: data.upgradeAllowed,
        cutoff: data.cutoff,
        items: data.items,
        minFormatScore: 0,
        cutoffFormatScore: 0,
        formatItems: [],
      };

      const id = this.editingId.value;
      if (id) {
        await httpV3.put(`/qualityprofile/${id}`, payload);
      } else {
        await httpV3.post('/qualityprofile', payload);
      }

      invalidateQueries(['/qualityprofile']);
      showSuccess('Quality profile saved');
      this.closeDialog();
    } catch {
      showError('Failed to save quality profile');
    } finally {
      this.isSaving.set(false);
    }
  }

  handleDeleteQualityProfile(id: number): void {
    if (confirm('Are you sure you want to delete this quality profile?')) {
      this.deleteQualityMutation.mutate(id);
    }
  }

  // Release Profile handlers
  handleAddReleaseProfile(): void {
    this.releaseFormData.set({
      name: '',
      enabled: true,
      required: '',
      ignored: '',
    });
    this.editingId.set(null);
    this.dialogMode.set('release-edit');
  }

  async handleEditReleaseProfile(id: number): Promise<void> {
    try {
      const profile = await httpV3.get<ReleaseProfile>(`/releaseprofile/${id}`);
      this.releaseFormData.set({
        name: profile.name || '',
        enabled: profile.enabled,
        required: (profile.required || []).join('\n'),
        ignored: (profile.ignored || []).join('\n'),
      });
      this.editingId.set(id);
      this.dialogMode.set('release-edit');
    } catch {
      showError('Failed to load release profile');
    }
  }

  updateReleaseField(field: string, value: unknown): void {
    const current = this.releaseFormData.value;
    this.releaseFormData.set({ ...current, [field]: value });
  }

  async handleSaveRelease(): Promise<void> {
    this.isSaving.set(true);

    try {
      const data = this.releaseFormData.value;
      const payload: ReleaseProfile = {
        id: this.editingId.value || 0,
        name: data.name || null,
        enabled: data.enabled,
        required: data.required.split('\n').map(s => s.trim()).filter(Boolean),
        ignored: data.ignored.split('\n').map(s => s.trim()).filter(Boolean),
        indexerId: 0,
        tags: [],
      };

      const id = this.editingId.value;
      if (id) {
        await httpV3.put(`/releaseprofile/${id}`, payload);
      } else {
        await httpV3.post('/releaseprofile', payload);
      }

      invalidateQueries(['/releaseprofile']);
      showSuccess('Release profile saved');
      this.closeDialog();
    } catch {
      showError('Failed to save release profile');
    } finally {
      this.isSaving.set(false);
    }
  }

  handleDeleteReleaseProfile(id: number): void {
    if (confirm('Are you sure you want to delete this release profile?')) {
      this.deleteReleaseMutation.mutate(id);
    }
  }

  closeDialog(): void {
    this.dialogMode.set('closed');
    this.editingId.set(null);
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
        margin-bottom: 2rem;
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

      .profiles-grid {
        display: grid;
        grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
        gap: 1rem;
      }

      .profile-card {
        display: flex;
        align-items: stretch;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        transition: border-color 0.15s;
      }

      .profile-card:hover {
        border-color: var(--color-primary);
      }

      .profile-content {
        flex: 1;
        padding: 1rem;
        cursor: pointer;
      }

      .profile-name {
        font-weight: 500;
        margin-bottom: 0.5rem;
      }

      .profile-meta {
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
        font-size: 0.75rem;
        color: var(--text-color-muted);
      }

      .profiles-list {
        display: flex;
        flex-direction: column;
        gap: 0.5rem;
      }

      .release-profile-row {
        display: flex;
        align-items: center;
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
        transition: border-color 0.15s;
      }

      .release-profile-row:hover {
        border-color: var(--color-primary);
      }

      .release-profile-row .profile-content {
        display: flex;
        align-items: center;
        justify-content: space-between;
        flex: 1;
        padding: 0.75rem 1rem;
        cursor: pointer;
      }

      .profile-info {
        display: flex;
        align-items: center;
        gap: 0.75rem;
      }

      .profile-status {
        font-size: 0.75rem;
        padding: 0.125rem 0.5rem;
        border-radius: 9999px;
      }

      .profile-status.enabled {
        background-color: var(--color-success);
        color: var(--color-white);
      }

      .profile-status.disabled {
        background-color: var(--color-warning);
        color: var(--color-white);
      }

      .profile-terms {
        display: flex;
        gap: 0.5rem;
      }

      .term-badge {
        font-size: 0.75rem;
        padding: 0.125rem 0.5rem;
        border-radius: 0.25rem;
      }

      .term-badge.required {
        background-color: var(--color-success);
        color: var(--color-white);
      }

      .term-badge.ignored {
        background-color: var(--color-danger);
        color: var(--color-white);
      }

      .profile-actions {
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

      .empty-state {
        padding: 2rem;
        text-align: center;
        color: var(--text-color-muted);
      }

      .empty-state .hint {
        font-size: 0.875rem;
        margin-top: 0.5rem;
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
        max-width: 700px;
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

      .form-row {
        margin-bottom: 1rem;
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

      .form-group input[type="text"],
      .form-group input[type="number"],
      .form-group select,
      .form-group textarea {
        padding: 0.5rem 0.75rem;
        background-color: var(--bg-input);
        border: 1px solid var(--border-color);
        border-radius: 0.25rem;
        color: var(--text-color);
        font-size: 0.875rem;
        font-family: inherit;
      }

      .form-group textarea {
        resize: vertical;
        min-height: 80px;
      }

      .form-group input:focus,
      .form-group select:focus,
      .form-group textarea:focus {
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

      .qualities-section {
        margin-top: 1.5rem;
        padding-top: 1rem;
        border-top: 1px solid var(--border-color);
      }

      .qualities-section h3 {
        margin: 0 0 0.5rem;
        font-size: 1rem;
        font-weight: 600;
      }

      .qualities-list {
        margin-top: 1rem;
        max-height: 300px;
        overflow-y: auto;
        border: 1px solid var(--border-color);
        border-radius: 0.375rem;
      }

      .quality-item {
        border-bottom: 1px solid var(--border-color);
      }

      .quality-item:last-child {
        border-bottom: none;
      }

      .quality-row {
        padding: 0.5rem 0.75rem;
      }

      .quality-row.sub-item {
        padding-left: 2rem;
        background-color: var(--bg-card-alt);
      }

      .quality-checkbox {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        cursor: pointer;
      }

      .quality-checkbox input[type="checkbox"] {
        width: 1rem;
        height: 1rem;
        accent-color: var(--color-primary);
      }

      .quality-name {
        flex: 1;
      }

      .quality-resolution {
        font-size: 0.75rem;
        color: var(--text-color-muted);
        padding: 0.125rem 0.375rem;
        background-color: var(--bg-card);
        border-radius: 0.25rem;
      }

      .quality-group-items {
        border-top: 1px solid var(--border-color);
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
