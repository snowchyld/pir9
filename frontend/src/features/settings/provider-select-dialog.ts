/**
 * Provider selection dialog - first step in adding a new provider
 * Shows available implementations grouped by protocol/type
 */

import { BaseComponent, customElement, escapeHtml, html, safeHtml } from '../../core/component';
import { signal } from '../../core/reactive';
import type { ProviderSchema } from './provider-types';

export interface ProviderSelectDialogConfig {
  title: string;
  schemaEndpoint: string;
  groupBy?: 'protocol' | 'none';
  groupLabels?: Record<string, string>;
  onSelect: (schema: ProviderSchema) => void;
  onClose: () => void;
}

@customElement('provider-select-dialog')
export class ProviderSelectDialog extends BaseComponent {
  private config = signal<ProviderSelectDialogConfig | null>(null);
  private schemas = signal<ProviderSchema[]>([]);
  private isLoading = signal(true);
  private error = signal<string | null>(null);

  protected onInit(): void {
    this.watch(this.config);
    this.watch(this.schemas);
    this.watch(this.isLoading);
    this.watch(this.error);
  }

  open(config: ProviderSelectDialogConfig): void {
    this.config.set(config);
    this.loadSchemas(config.schemaEndpoint);
  }

  close(): void {
    this.config.value?.onClose();
    this.config.set(null);
    this.schemas.set([]);
    this.error.set(null);
  }

  private async loadSchemas(endpoint: string): Promise<void> {
    this.isLoading.set(true);
    this.error.set(null);

    try {
      // Use v3 API for provider schemas
      const response = await fetch(`/api/v3${endpoint}`);
      if (!response.ok) {
        throw new Error(`Failed to load schemas: ${response.statusText}`);
      }
      const data = await response.json();
      this.schemas.set(Array.isArray(data) ? data : [data]);
    } catch (err) {
      this.error.set(err instanceof Error ? err.message : 'Failed to load schemas');
    } finally {
      this.isLoading.set(false);
    }
  }

  private handleSelect(schema: ProviderSchema): void {
    this.config.value?.onSelect(schema);
    this.close();
  }

  private groupSchemas(schemas: ProviderSchema[]): Record<string, ProviderSchema[]> {
    const config = this.config.value;
    if (!config || config.groupBy === 'none') {
      return { all: schemas };
    }

    return schemas.reduce(
      (groups, schema) => {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const key = ((schema as any)[config.groupBy!] as string) || 'other';
        if (!groups[key]) {
          groups[key] = [];
        }
        groups[key].push(schema);
        return groups;
      },
      {} as Record<string, ProviderSchema[]>,
    );
  }

  protected template(): string {
    const config = this.config.value;
    if (!config) return '';

    const isLoading = this.isLoading.value;
    const error = this.error.value;
    const schemas = this.schemas.value;
    const grouped = this.groupSchemas(schemas);

    return html`
      <div class="dialog-backdrop" onclick="this.querySelector('provider-select-dialog').handleBackdropClick(event)">
        <div class="dialog" role="dialog" aria-modal="true">
          <div class="dialog-header">
            <h2>${escapeHtml(config.title)}</h2>
            <button class="close-btn" onclick="this.closest('provider-select-dialog').close()" aria-label="Close">
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>

          <div class="dialog-body">
            ${
              isLoading
                ? html`
              <div class="loading">
                <div class="spinner"></div>
                <p>Loading available options...</p>
              </div>
            `
                : error
                  ? html`
              <div class="error-message">
                <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="8" x2="12" y2="12"></line>
                  <line x1="12" y1="16" x2="12.01" y2="16"></line>
                </svg>
                <p>${escapeHtml(error)}</p>
              </div>
            `
                  : html`
              <div class="info-box">
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="10"></circle>
                  <line x1="12" y1="16" x2="12" y2="12"></line>
                  <line x1="12" y1="8" x2="12.01" y2="8"></line>
                </svg>
                <span>Select a provider to configure</span>
              </div>

              ${Object.entries(grouped)
                .map(
                  ([group, items]) => html`
                ${
                  config.groupBy !== 'none' && Object.keys(grouped).length > 1
                    ? html`
                  <div class="group-header">
                    <h3>${escapeHtml(config.groupLabels?.[group] || group)}</h3>
                  </div>
                `
                    : ''
                }
                <div class="provider-grid">
                  ${items.map((schema) => this.renderProviderCard(schema)).join('')}
                </div>
              `,
                )
                .join('')}
            `
            }
          </div>

          <div class="dialog-footer">
            <button class="btn btn-secondary" onclick="this.closest('provider-select-dialog').close()">
              Cancel
            </button>
          </div>
        </div>
      </div>

      ${safeHtml(this.styles())}
    `;
  }

  private renderProviderCard(schema: ProviderSchema): string {
    const schemaJson = escapeHtml(JSON.stringify(schema));
    return html`
      <button
        class="provider-card"
        onclick="this.closest('provider-select-dialog').selectSchema('${schemaJson}')"
      >
        <div class="provider-icon">
          ${this.getProviderIcon(schema.implementation)}
        </div>
        <div class="provider-info">
          <span class="provider-name">${escapeHtml(schema.implementationName)}</span>
        </div>
      </button>
    `;
  }

  selectSchema(schemaJson: string): void {
    try {
      const schema = JSON.parse(schemaJson);
      this.handleSelect(schema);
    } catch {
      // Parse error - ignore
    }
  }

  private getProviderIcon(_implementation: string): string {
    // Return a generic icon - could be extended with specific icons per provider
    return `<svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
      <polyline points="7 10 12 15 17 10"></polyline>
      <line x1="12" y1="15" x2="12" y2="3"></line>
    </svg>`;
  }

  private styles(): string {
    return `<style>
      provider-select-dialog {
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
        max-width: 600px;
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
        justify-content: flex-end;
        gap: 0.75rem;
        padding: 1rem 1.5rem;
        border-top: 1px solid var(--border-color);
      }

      .loading {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 1rem;
        padding: 2rem;
        color: var(--text-color-muted);
      }

      .spinner {
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

      .error-message {
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 0.5rem;
        padding: 2rem;
        color: var(--color-danger);
        text-align: center;
      }

      .info-box {
        display: flex;
        align-items: center;
        gap: 0.5rem;
        padding: 0.75rem 1rem;
        background-color: var(--color-info-bg, rgba(93, 156, 236, 0.1));
        border: 1px solid var(--color-info-border, rgba(93, 156, 236, 0.3));
        border-radius: 0.375rem;
        color: var(--color-info, #5d9cec);
        font-size: 0.875rem;
        margin-bottom: 1.5rem;
      }

      .group-header {
        margin-top: 1.5rem;
        margin-bottom: 0.75rem;
        padding-bottom: 0.5rem;
        border-bottom: 1px solid var(--border-color);
      }

      .group-header:first-child {
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
        grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
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
        display: flex;
        align-items: center;
        justify-content: center;
        width: 48px;
        height: 48px;
        color: var(--color-primary);
      }

      .provider-info {
        text-align: center;
      }

      .provider-name {
        font-size: 0.8125rem;
        font-weight: 500;
        color: var(--text-color);
      }

      .btn {
        padding: 0.5rem 1rem;
        border-radius: 0.25rem;
        font-size: 0.875rem;
        font-weight: 500;
        cursor: pointer;
        transition: all 0.15s ease;
      }

      .btn-secondary {
        background-color: var(--bg-card-alt);
        border: 1px solid var(--border-color);
        color: var(--text-color);
      }

      .btn-secondary:hover {
        background-color: var(--bg-input-hover);
      }
    </style>`;
  }
}
