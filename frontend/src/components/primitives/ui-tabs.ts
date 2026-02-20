/**
 * Tabs component
 */

import { BaseComponent, customElement, attribute, html, escapeHtml, safeHtml } from '../../core/component';

export interface Tab {
  id: string;
  label: string;
  icon?: string;
  disabled?: boolean;
}

@customElement('ui-tabs')
export class UITabs extends BaseComponent {
  @attribute() activeTab = '';

  private _tabs: Tab[] = [];

  get tabs(): Tab[] {
    return this._tabs;
  }

  set tabs(value: Tab[]) {
    this._tabs = value;
    if (!this.activeTab && value.length > 0) {
      this.activeTab = value[0].id;
    }
    if (this._isConnected) {
      this.requestUpdate();
    }
  }

  protected template(): string {
    return html`
      <div class="tabs">
        <div class="tabs-list" role="tablist">
          ${this._tabs.map((tab) => this.renderTab(tab)).join('')}
        </div>
        <div class="tabs-content">
          <slot name="${this.activeTab}"></slot>
        </div>
      </div>

      <style>
        :host {
          display: block;
        }

        .tabs {
          display: flex;
          flex-direction: column;
        }

        .tabs-list {
          display: flex;
          gap: 0.25rem;
          border-bottom: 1px solid var(--border-color);
          padding-bottom: -1px;
        }

        .tab-button {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          padding: 0.75rem 1rem;
          background: transparent;
          border: none;
          border-bottom: 2px solid transparent;
          color: var(--text-color-muted);
          font-size: 0.875rem;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.15s ease;
          margin-bottom: -1px;
        }

        .tab-button:hover:not(:disabled) {
          color: var(--text-color);
        }

        .tab-button.active {
          color: var(--color-primary);
          border-bottom-color: var(--color-primary);
        }

        .tab-button:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .tab-icon {
          width: 16px;
          height: 16px;
        }

        .tabs-content {
          padding: 1rem 0;
        }
      </style>
    `;
  }

  private renderTab(tab: Tab): string {
    const isActive = tab.id === this.activeTab;

    return html`
      <button
        class="tab-button ${isActive ? 'active' : ''}"
        role="tab"
        aria-selected="${isActive}"
        ?disabled="${tab.disabled}"
        onclick="this.closest('ui-tabs').handleTabClick('${tab.id}')"
      >
        ${tab.icon ? safeHtml(tab.icon) : ''}
        <span>${escapeHtml(tab.label)}</span>
      </button>
    `;
  }

  handleTabClick(tabId: string): void {
    const tab = this._tabs.find((t) => t.id === tabId);
    if (tab && !tab.disabled) {
      this.activeTab = tabId;
      this.emit('change', { tab: tabId });
    }
  }
}
