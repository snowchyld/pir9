/**
 * Application header with search and actions
 */

import { BaseComponent, customElement, html, escapeHtml } from '../../core/component';
import { toggleMobileMenu, searchQuery, setSearchQuery } from '../../stores/app.store';
import { toggleTheme, resolvedTheme } from '../../stores/theme.store';
import { useSystemStatusQuery } from '../../core/query';
import { navigate } from '../../router';

@customElement('app-header')
export class AppHeader extends BaseComponent {
  private statusQuery = useSystemStatusQuery();

  protected onInit(): void {
    this.watch(searchQuery);
    this.watch(this.statusQuery.data);
    this.watch(resolvedTheme);
  }

  protected template(): string {
    const query = searchQuery.value;
    const status = this.statusQuery.data.value;
    const theme = resolvedTheme.value;

    return html`
      <header class="header">
        <!-- Mobile menu toggle -->
        <button
          class="menu-toggle lg:hidden"
          onclick="this.closest('app-header').handleMenuToggle()"
          aria-label="Toggle menu"
        >
          <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="3" y1="12" x2="21" y2="12"></line>
            <line x1="3" y1="6" x2="21" y2="6"></line>
            <line x1="3" y1="18" x2="21" y2="18"></line>
          </svg>
        </button>

        <!-- Search bar -->
        <div class="search-container">
          <svg class="search-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"></circle>
            <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
          </svg>
          <input
            type="text"
            class="search-input"
            placeholder="Search series..."
            value="${escapeHtml(query)}"
            oninput="this.closest('app-header').handleSearch(event)"
            onkeydown="this.closest('app-header').handleSearchKeydown(event)"
          />
          ${query ? html`
            <button
              class="search-clear"
              onclick="this.closest('app-header').handleClearSearch()"
              aria-label="Clear search"
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          ` : ''}
        </div>

        <!-- Actions -->
        <div class="header-actions">
          <!-- Add Series -->
          <button
            class="action-btn"
            onclick="this.closest('app-header').handleAddSeries()"
            title="Add Series"
          >
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </button>

          <!-- Theme toggle -->
          <button
            class="action-btn"
            onclick="this.closest('app-header').handleThemeToggle()"
            title="Toggle theme"
          >
            ${theme === 'dark' ? html`
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <circle cx="12" cy="12" r="5"></circle>
                <line x1="12" y1="1" x2="12" y2="3"></line>
                <line x1="12" y1="21" x2="12" y2="23"></line>
                <line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line>
                <line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line>
                <line x1="1" y1="12" x2="3" y2="12"></line>
                <line x1="21" y1="12" x2="23" y2="12"></line>
                <line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line>
                <line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line>
              </svg>
            ` : html`
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path>
              </svg>
            `}
          </button>

          <!-- Version badge -->
          ${status ? html`
            <span class="version-badge" title="Version ${escapeHtml(status.version)}">
              ${escapeHtml(status.version)}
            </span>
          ` : ''}
        </div>
      </header>

      <style>
        .header {
          display: flex;
          align-items: center;
          gap: 1rem;
          padding: 0.75rem 1.25rem;
          background: var(--bg-header);
          backdrop-filter: blur(var(--glass-blur-strong)) saturate(var(--glass-saturation));
          -webkit-backdrop-filter: blur(var(--glass-blur-strong)) saturate(var(--glass-saturation));
          border-bottom: 1px solid var(--border-glass);
        }

        .menu-toggle {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.625rem;
          border-radius: 0.5rem;
          color: var(--text-color);
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .menu-toggle:hover {
          background-color: var(--bg-input-hover);
          color: var(--pir9-blue);
          border-color: var(--pir9-blue);
          box-shadow: 0 0 15px rgba(53, 197, 244, 0.2);
        }

        .menu-toggle:active {
          transform: scale(0.95);
        }

        .search-container {
          position: relative;
          flex: 1;
          max-width: 450px;
          animation: fadeIn var(--transition-normal) var(--ease-out-expo);
        }

        .search-icon {
          position: absolute;
          left: 1rem;
          top: 50%;
          transform: translateY(-50%);
          color: var(--text-color-muted);
          pointer-events: none;
          transition: color var(--transition-fast);
        }

        .search-container:focus-within .search-icon {
          color: var(--pir9-blue);
        }

        .search-input {
          width: 100%;
          padding: 0.625rem 1rem 0.625rem 2.75rem;
          background-color: var(--bg-input);
          backdrop-filter: blur(8px);
          -webkit-backdrop-filter: blur(8px);
          color: var(--text-color);
          border: 1px solid var(--border-input);
          border-radius: 0.75rem;
          font-size: 0.875rem;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .search-input::placeholder {
          color: var(--text-color-muted);
        }

        .search-input:hover {
          background-color: var(--bg-input-hover);
          border-color: var(--border-glass);
        }

        .search-input:focus {
          outline: none;
          background-color: var(--bg-input-focus);
          border-color: var(--border-input-focus);
          box-shadow: var(--shadow-input-focus), 0 4px 20px rgba(93, 156, 236, 0.15);
        }

        .search-clear {
          position: absolute;
          right: 0.75rem;
          top: 50%;
          transform: translateY(-50%);
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.375rem;
          border-radius: 0.375rem;
          color: var(--text-color-muted);
          background: var(--bg-card);
          border: none;
          cursor: pointer;
          transition: all var(--transition-fast) var(--ease-out-expo);
        }

        .search-clear:hover {
          color: var(--text-color);
          background: var(--bg-input-hover);
          transform: translateY(-50%) scale(1.1);
        }

        .header-actions {
          display: flex;
          align-items: center;
          gap: 0.5rem;
        }

        .action-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.625rem;
          border-radius: 0.5rem;
          color: var(--text-color);
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
          position: relative;
          overflow: hidden;
        }

        .action-btn::before {
          content: '';
          position: absolute;
          inset: 0;
          background: linear-gradient(135deg, rgba(255,255,255,0.1) 0%, transparent 100%);
          opacity: 0;
          transition: opacity var(--transition-fast);
        }

        .action-btn:hover {
          background-color: var(--bg-input-hover);
          color: var(--pir9-blue);
          border-color: var(--pir9-blue);
          box-shadow: 0 0 15px rgba(53, 197, 244, 0.2);
          transform: translateY(-1px);
        }

        .action-btn:hover::before {
          opacity: 1;
        }

        .action-btn:active {
          transform: translateY(0) scale(0.95);
        }

        .action-btn svg {
          transition: transform var(--transition-fast) var(--ease-spring);
        }

        .action-btn:hover svg {
          transform: scale(1.1);
        }

        .version-badge {
          display: none;
          padding: 0.375rem 0.75rem;
          font-size: 0.75rem;
          font-weight: 500;
          color: var(--text-color-muted);
          background: var(--bg-card);
          backdrop-filter: blur(8px);
          -webkit-backdrop-filter: blur(8px);
          border: 1px solid var(--border-glass);
          border-radius: 9999px;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .version-badge:hover {
          color: var(--pir9-blue);
          border-color: rgba(53, 197, 244, 0.3);
        }

        @media (min-width: 640px) {
          .version-badge {
            display: inline-flex;
          }
        }

        @media (min-width: 1024px) {
          .menu-toggle {
            display: none;
          }
        }
      </style>
    `;
  }

  handleMenuToggle(): void {
    toggleMobileMenu();
  }

  handleSearch(event: Event): void {
    const input = event.target as HTMLInputElement;
    setSearchQuery(input.value);
  }

  handleSearchKeydown(event: KeyboardEvent): void {
    if (event.key === 'Escape') {
      setSearchQuery('');
      (event.target as HTMLInputElement).blur();
    }
  }

  handleClearSearch(): void {
    setSearchQuery('');
    const input = this.$<HTMLInputElement>('.search-input');
    input?.focus();
  }

  handleAddSeries(): void {
    navigate('/add/new');
  }

  handleThemeToggle(): void {
    toggleTheme();
  }
}
