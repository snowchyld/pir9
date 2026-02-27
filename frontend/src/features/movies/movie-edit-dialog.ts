/**
 * Movie edit dialog - for editing movie settings
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { http, type Movie } from '../../core/http';
import { createQuery, invalidateQueries } from '../../core/query';
import { signal } from '../../core/reactive';
import { navigate } from '../../router';
import { showError, showSuccess } from '../../stores/app.store';

interface QualityProfile {
  id: number;
  name: string;
}

interface RootFolder {
  id: number;
  path: string;
  freeSpace: number;
  contentType: string;
}

interface MovieFormData {
  monitored: boolean;
  qualityProfileId: number;
  path: string;
  tags: number[];
}

@customElement('movie-edit-dialog')
export class MovieEditDialog extends BaseComponent {
  private isOpen = signal(false);
  private movie = signal<Movie | null>(null);
  private formData = signal<MovieFormData | null>(null);
  private isSaving = signal(false);
  private errors = signal<string[]>([]);

  private qualityProfilesQuery = createQuery({
    queryKey: ['/qualityprofile'],
    queryFn: () => http.get<QualityProfile[]>('/qualityprofile'),
  });

  private rootFoldersQuery = createQuery({
    queryKey: ['/rootfolder', 'movie'],
    queryFn: () => http.get<RootFolder[]>('/rootfolder', { params: { contentType: 'movie' } }),
  });

  protected onInit(): void {
    this.watch(this.isOpen);
    this.watch(this.movie);
    this.watch(this.formData);
    this.watch(this.isSaving);
    this.watch(this.errors);
    this.watch(this.qualityProfilesQuery.data);
    this.watch(this.rootFoldersQuery.data);
  }

  open(movie: Movie): void {
    this.movie.set(movie);
    this.formData.set({
      monitored: movie.monitored,
      qualityProfileId: movie.qualityProfileId,
      path: movie.path,
      tags: movie.tags ?? [],
    });
    this.errors.set([]);
    this.isOpen.set(true);
  }

  close(): void {
    this.isOpen.set(false);
    this.movie.set(null);
    this.formData.set(null);
    this.errors.set([]);
  }

  private updateField<K extends keyof MovieFormData>(name: K, value: MovieFormData[K]): void {
    const current = this.formData.value;
    if (current) {
      this.formData.set({ ...current, [name]: value });
    }
  }

  handleFieldChange(name: string, value: unknown): void {
    this.updateField(name as keyof MovieFormData, value as never);
  }

  handleBackdropClick(e: Event): void {
    if ((e.target as HTMLElement).classList.contains('dialog-backdrop')) {
      this.close();
    }
  }

  protected template(): string {
    if (!this.isOpen.value) return '';

    const movie = this.movie.value;
    const data = this.formData.value;
    if (!movie || !data) return '';

    const qualityProfiles = this.qualityProfilesQuery.data.value ?? [];
    const rootFolders = this.rootFoldersQuery.data.value ?? [];
    const isSaving = this.isSaving.value;
    const errors = this.errors.value;

    return html`
      <div class="dialog-backdrop" onclick="this.querySelector('movie-edit-dialog').handleBackdropClick(event)">
        <div class="dialog" role="dialog" aria-modal="true">
          <div class="dialog-header">
            <h2>Edit - ${escapeHtml(movie.title)}</h2>
            <button class="close-btn" onclick="this.closest('movie-edit-dialog').close()" aria-label="Close">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="dialog-body">
            ${
              errors.length > 0
                ? html`
              <div class="error-box">
                ${errors.map((e) => html`<p>${escapeHtml(e)}</p>`).join('')}
              </div>
            `
                : ''
            }

            <form class="edit-form" onsubmit="event.preventDefault()">
              <!-- Monitored -->
              <div class="form-group form-group-checkbox">
                <label>
                  <input
                    type="checkbox"
                    ${data.monitored ? 'checked' : ''}
                    onchange="this.closest('movie-edit-dialog').handleFieldChange('monitored', this.checked)"
                  />
                  <span>Monitored</span>
                </label>
                <p class="help-text">Search for and download this movie when available</p>
              </div>

              <!-- Quality Profile -->
              <div class="form-group">
                <label for="qualityProfileId">Quality Profile</label>
                <select
                  id="qualityProfileId"
                  onchange="this.closest('movie-edit-dialog').handleFieldChange('qualityProfileId', parseInt(this.value))"
                >
                  ${qualityProfiles
                    .map(
                      (p) => html`
                    <option value="${p.id}" ${data.qualityProfileId === p.id ? 'selected' : ''}>
                      ${escapeHtml(p.name)}
                    </option>
                  `,
                    )
                    .join('')}
                </select>
              </div>

              <!-- Path -->
              <div class="form-group">
                <label for="path">Path</label>
                <div class="path-input-group">
                  <input
                    type="text"
                    id="path"
                    value="${escapeHtml(data.path)}"
                    onchange="this.closest('movie-edit-dialog').handleFieldChange('path', this.value)"
                  />
                </div>
                <p class="help-text">Location of movie files on disk</p>
                ${
                  rootFolders.length > 0
                    ? html`
                  <div class="root-folder-hint">
                    <span class="hint-label">Root folders:</span>
                    ${rootFolders
                      .map(
                        (f) => html`
                      <button
                        type="button"
                        class="root-folder-btn"
                        onclick="this.closest('movie-edit-dialog').setPathFromRoot('${escapeHtml(f.path)}')"
                      >
                        ${escapeHtml(f.path)}
                      </button>
                    `,
                      )
                      .join('')}
                  </div>
                `
                    : ''
                }
              </div>
            </form>
          </div>

          <div class="dialog-footer">
            <button class="btn btn-danger" onclick="this.closest('movie-edit-dialog').handleDelete()">
              Delete
            </button>

            <div class="footer-spacer"></div>

            <button class="btn btn-secondary" onclick="this.closest('movie-edit-dialog').close()">
              Cancel
            </button>
            <button
              class="btn btn-primary"
              onclick="this.closest('movie-edit-dialog').handleSave()"
              ${isSaving ? 'disabled' : ''}
            >
              ${
                isSaving
                  ? html`
                <span class="btn-spinner"></span>
                Saving...
              `
                  : 'Save'
              }
            </button>
          </div>
        </div>
      </div>

      ${safeHtml(this.styles())}
    `;
  }

  setPathFromRoot(rootPath: string): void {
    const movie = this.movie.value;
    if (!movie) return;

    const folder = `${movie.title} (${movie.year})`;
    const newPath =
      rootPath.endsWith('/') || rootPath.endsWith('\\')
        ? `${rootPath}${folder}`
        : `${rootPath}/${folder}`;

    this.updateField('path', newPath);
  }

  async handleSave(): Promise<void> {
    const movie = this.movie.value;
    const data = this.formData.value;
    if (!movie || !data) return;

    this.isSaving.set(true);
    this.errors.set([]);

    try {
      await http.put(`/movie/${movie.id}`, {
        monitored: data.monitored,
        qualityProfileId: data.qualityProfileId,
        path: data.path,
        tags: data.tags,
      });

      invalidateQueries(['/movie']);
      showSuccess('Movie saved');
      this.close();
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Failed to save movie';
      this.errors.set([message]);
    } finally {
      this.isSaving.set(false);
    }
  }

  handleDelete(): void {
    const movie = this.movie.value;
    if (!movie) return;

    if (confirm(`Are you sure you want to delete "${movie.title}"? This cannot be undone.`)) {
      http
        .delete(`/movie/${movie.id}`, { params: { deleteFiles: false } })
        .then(() => {
          invalidateQueries(['/movie']);
          showSuccess('Movie deleted');
          this.close();
          navigate('/movies');
        })
        .catch((err) => {
          showError(err instanceof Error ? err.message : 'Failed to delete movie');
        });
    }
  }

  private styles(): string {
    return `<style>
      movie-edit-dialog { display: contents; }

      .dialog-backdrop {
        position: fixed; inset: 0;
        background-color: rgba(0, 0, 0, 0.6);
        display: flex; align-items: center; justify-content: center;
        z-index: 1000; padding: 1rem;
      }

      .dialog {
        background-color: var(--bg-card);
        border: 1px solid var(--border-color);
        border-radius: 0.5rem;
        width: 100%; max-width: 500px; max-height: 90vh;
        display: flex; flex-direction: column;
        box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.5);
      }

      .dialog-header {
        display: flex; align-items: center; justify-content: space-between;
        padding: 1rem 1.5rem;
        border-bottom: 1px solid var(--border-color);
      }
      .dialog-header h2 { margin: 0; font-size: 1.125rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

      .close-btn {
        display: flex; align-items: center; justify-content: center;
        padding: 0.25rem; background: transparent; border: none;
        border-radius: 0.25rem; color: var(--text-color-muted); cursor: pointer; flex-shrink: 0;
      }
      .close-btn:hover { color: var(--text-color); background-color: var(--bg-input-hover); }

      .dialog-body { flex: 1; overflow-y: auto; padding: 1.5rem; }

      .dialog-footer {
        display: flex; align-items: center; gap: 0.75rem;
        padding: 1rem 1.5rem; border-top: 1px solid var(--border-color);
      }
      .footer-spacer { flex: 1; }

      .error-box {
        padding: 0.75rem 1rem;
        background-color: rgba(240, 80, 80, 0.1);
        border: 1px solid rgba(240, 80, 80, 0.3);
        border-radius: 0.375rem; color: var(--color-danger); margin-bottom: 1rem;
      }
      .error-box p { margin: 0; font-size: 0.875rem; }

      .edit-form { display: flex; flex-direction: column; gap: 1.25rem; }
      .form-group { display: flex; flex-direction: column; gap: 0.375rem; }
      .form-group label { font-size: 0.875rem; font-weight: 500; color: var(--text-color); }
      .form-group input[type="text"],
      .form-group select {
        padding: 0.5rem 0.75rem; background-color: var(--bg-input);
        border: 1px solid var(--border-color); border-radius: 0.25rem;
        color: var(--text-color); font-size: 0.875rem;
      }
      .form-group input:focus, .form-group select:focus {
        outline: none; border-color: var(--color-primary);
        box-shadow: 0 0 0 2px rgba(93, 156, 236, 0.2);
      }
      .form-group-checkbox { flex-direction: row; flex-wrap: wrap; align-items: flex-start; }
      .form-group-checkbox > label { display: flex; align-items: center; gap: 0.5rem; cursor: pointer; font-weight: 400; width: 100%; }
      .form-group-checkbox input[type="checkbox"] { width: 1rem; height: 1rem; accent-color: var(--color-primary); }
      .help-text { font-size: 0.75rem; color: var(--text-color-muted); margin: 0.25rem 0 0; width: 100%; }
      .path-input-group { display: flex; gap: 0.5rem; }
      .path-input-group input { flex: 1; }
      .root-folder-hint { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem; margin-top: 0.5rem; }
      .hint-label { font-size: 0.75rem; color: var(--text-color-muted); }
      .root-folder-btn {
        padding: 0.25rem 0.5rem; font-size: 0.75rem;
        background-color: var(--bg-card-alt); border: 1px solid var(--border-color);
        border-radius: 0.25rem; color: var(--text-color-muted); cursor: pointer;
      }
      .root-folder-btn:hover { background-color: var(--bg-input-hover); color: var(--text-color); }

      .btn {
        display: inline-flex; align-items: center; gap: 0.5rem;
        padding: 0.5rem 1rem; border-radius: 0.25rem;
        font-size: 0.875rem; font-weight: 500; cursor: pointer;
        transition: all 0.15s ease;
      }
      .btn:disabled { opacity: 0.6; cursor: not-allowed; }
      .btn-secondary { background-color: var(--bg-card-alt); border: 1px solid var(--border-color); color: var(--text-color); }
      .btn-secondary:hover:not(:disabled) { background-color: var(--bg-input-hover); }
      .btn-primary { background-color: var(--btn-primary-bg); border: 1px solid var(--btn-primary-border); color: var(--color-white); }
      .btn-primary:hover:not(:disabled) { background-color: var(--btn-primary-bg-hover); }
      .btn-danger { background-color: transparent; border: 1px solid var(--color-danger); color: var(--color-danger); }
      .btn-danger:hover:not(:disabled) { background-color: var(--color-danger); color: var(--color-white); }
      .btn-spinner { width: 14px; height: 14px; border: 2px solid currentColor; border-top-color: transparent; border-radius: 50%; animation: spin 0.8s linear infinite; }
      @keyframes spin { to { transform: rotate(360deg); } }
    </style>`;
  }
}
