/**
 * Dropdown menu component
 */

import { BaseComponent, customElement, attribute, html, escapeHtml, safeHtml } from '../../core/component';

export interface MenuItem {
  id: string;
  label: string;
  icon?: string;
  disabled?: boolean;
  danger?: boolean;
  divider?: boolean;
}

@customElement('ui-dropdown-menu')
export class UIDropdownMenu extends BaseComponent {
  @attribute({ type: 'boolean' }) open = false;
  @attribute() position: 'left' | 'right' = 'left';

  private _items: MenuItem[] = [];

  get items(): MenuItem[] {
    return this._items;
  }

  set items(value: MenuItem[]) {
    this._items = value;
    if (this._isConnected) {
      this.requestUpdate();
    }
  }

  protected onInit(): void {
    this.handleClickOutside = this.handleClickOutside.bind(this);
  }

  protected onMount(): void {
    document.addEventListener('click', this.handleClickOutside);
  }

  protected onDestroy(): void {
    document.removeEventListener('click', this.handleClickOutside);
  }

  private handleClickOutside(event: MouseEvent): void {
    if (this.open && !this.contains(event.target as Node)) {
      this.open = false;
    }
  }

  protected template(): string {
    return html`
      <div class="dropdown">
        <div class="dropdown-trigger" onclick="this.closest('ui-dropdown-menu').toggle(event)">
          <slot name="trigger">
            <button class="default-trigger">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor">
                <circle cx="12" cy="5" r="2"></circle>
                <circle cx="12" cy="12" r="2"></circle>
                <circle cx="12" cy="19" r="2"></circle>
              </svg>
            </button>
          </slot>
        </div>
        ${this.open ? html`
          <div class="dropdown-menu ${this.position}">
            ${this._items.map((item) => this.renderItem(item)).join('')}
          </div>
        ` : ''}
      </div>

      <style>
        :host {
          display: inline-block;
          position: relative;
        }

        .dropdown {
          position: relative;
        }

        .dropdown-trigger {
          display: inline-flex;
        }

        .default-trigger {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          background: transparent;
          border: none;
          border-radius: 0.25rem;
          color: var(--text-color-muted);
          cursor: pointer;
          transition: color 0.15s ease, background-color 0.15s ease;
        }

        .default-trigger:hover {
          color: var(--text-color);
          background-color: var(--bg-input-hover);
        }

        .dropdown-menu {
          position: absolute;
          top: 100%;
          margin-top: 0.25rem;
          min-width: 160px;
          background-color: var(--bg-popover);
          border: 1px solid var(--border-color);
          border-radius: 0.375rem;
          box-shadow: 0 4px 12px var(--shadow-popover);
          z-index: 50;
          animation: fadeIn 0.15s ease-out;
          overflow: hidden;
        }

        .dropdown-menu.left {
          left: 0;
        }

        .dropdown-menu.right {
          right: 0;
        }

        .menu-item {
          display: flex;
          align-items: center;
          gap: 0.5rem;
          width: 100%;
          padding: 0.5rem 0.75rem;
          background: transparent;
          border: none;
          color: var(--menu-item-color);
          font-size: 0.875rem;
          text-align: left;
          cursor: pointer;
          transition: background-color 0.15s ease, color 0.15s ease;
        }

        .menu-item:hover:not(:disabled) {
          background-color: var(--menu-item-hover-bg);
          color: var(--menu-item-hover);
        }

        .menu-item:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .menu-item.danger {
          color: var(--color-danger);
        }

        .menu-item.danger:hover:not(:disabled) {
          background-color: rgba(240, 80, 80, 0.1);
        }

        .menu-icon {
          width: 16px;
          height: 16px;
          flex-shrink: 0;
        }

        .menu-divider {
          height: 1px;
          margin: 0.25rem 0;
          background-color: var(--border-color);
        }

        @keyframes fadeIn {
          from {
            opacity: 0;
            transform: translateY(-4px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      </style>
    `;
  }

  private renderItem(item: MenuItem): string {
    if (item.divider) {
      return '<div class="menu-divider"></div>';
    }

    const classes = this.cx('menu-item', item.danger && 'danger');

    return html`
      <button
        class="${classes}"
        ?disabled="${item.disabled}"
        onclick="this.closest('ui-dropdown-menu').handleItemClick('${item.id}')"
      >
        ${item.icon ? safeHtml(item.icon) : ''}
        <span>${escapeHtml(item.label)}</span>
      </button>
    `;
  }

  toggle(event: MouseEvent): void {
    event.stopPropagation();
    this.open = !this.open;
  }

  handleItemClick(itemId: string): void {
    const item = this._items.find((i) => i.id === itemId);
    if (item && !item.disabled) {
      this.open = false;
      this.emit('select', { item });
    }
  }
}
