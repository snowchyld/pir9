/**
 * Application sidebar with navigation
 */

import { BaseComponent, customElement, html } from '../../core/component';
import { currentRoute, isActive, navigate } from '../../router';
import { closeMobileMenu, sidebarCollapsed, toggleSidebar } from '../../stores/app.store';

interface NavItem {
  path: string;
  label: string;
  icon: string;
  exact?: boolean;
}

const NAV_ITEMS: NavItem[] = [
  { path: '/', label: 'Dashboard', icon: 'home', exact: true },
  { path: '/series', label: 'Series', icon: 'tv' },
  { path: '/anime', label: 'Anime', icon: 'anime' },
  { path: '/movies', label: 'Movies', icon: 'film' },
  { path: '/calendar', label: 'Calendar', icon: 'calendar' },
  { path: '/activity/queue', label: 'Activity', icon: 'download' },
  { path: '/wanted/missing', label: 'Wanted', icon: 'alert-circle' },
  { path: '/settings', label: 'Settings', icon: 'settings' },
  { path: '/system/status', label: 'System', icon: 'laptop' },
];

@customElement('app-sidebar')
export class AppSidebar extends BaseComponent {
  protected onInit(): void {
    this.watch(sidebarCollapsed);
    this.watch(currentRoute);
  }

  protected template(): string {
    const collapsed = sidebarCollapsed.value;

    return html`
      <nav class="sidebar-nav">
        <!-- Logo / Brand -->
        <div class="sidebar-brand">
          <a href="/" class="brand-link" onclick="event.preventDefault(); window.navigate('/')">
            <span class="brand-icon">
              <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
              </svg>
            </span>
            ${collapsed ? '' : '<span class="brand-text">pir9</span>'}
          </a>
        </div>

        <!-- Navigation items -->
        <ul class="nav-list">
          ${NAV_ITEMS.map((item) => this.renderNavItem(item, collapsed)).join('')}
        </ul>

        <!-- Collapse toggle -->
        <button
          class="collapse-toggle"
          onclick="this.closest('app-sidebar').handleToggle()"
          title="${collapsed ? 'Expand sidebar' : 'Collapse sidebar'}"
        >
          <svg
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            class="${collapsed ? 'rotate-180' : ''}"
          >
            <polyline points="15 18 9 12 15 6"></polyline>
          </svg>
        </button>
      </nav>

      <style>
        :host {
          display: block;
          height: 100%;
        }

        .sidebar-nav {
          display: flex;
          flex-direction: column;
          height: 100%;
          padding: 0.75rem;
          background: var(--bg-sidebar);
          backdrop-filter: blur(var(--glass-blur-strong)) saturate(var(--glass-saturation));
          -webkit-backdrop-filter: blur(var(--glass-blur-strong)) saturate(var(--glass-saturation));
        }

        .sidebar-brand {
          padding: 0.75rem;
          margin-bottom: 1rem;
        }

        .brand-link {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          color: var(--pir9-blue);
          text-decoration: none;
          font-weight: 600;
          font-size: 1.25rem;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .brand-link:hover {
          transform: scale(1.02);
          text-shadow: 0 0 20px rgba(53, 197, 244, 0.5);
        }

        .brand-icon {
          flex-shrink: 0;
          filter: drop-shadow(0 0 8px rgba(53, 197, 244, 0.4));
        }

        .nav-list {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 0.375rem;
        }

        .nav-item {
          list-style: none;
          animation: slideInLeft var(--transition-normal) var(--ease-out-expo) backwards;
        }

        .nav-item:nth-child(1) { animation-delay: 50ms; }
        .nav-item:nth-child(2) { animation-delay: 100ms; }
        .nav-item:nth-child(3) { animation-delay: 150ms; }
        .nav-item:nth-child(4) { animation-delay: 200ms; }
        .nav-item:nth-child(5) { animation-delay: 250ms; }
        .nav-item:nth-child(6) { animation-delay: 300ms; }
        .nav-item:nth-child(7) { animation-delay: 350ms; }
        .nav-item:nth-child(8) { animation-delay: 400ms; }
        .nav-item:nth-child(9) { animation-delay: 450ms; }

        .nav-link {
          display: flex;
          align-items: center;
          gap: 0.75rem;
          padding: 0.75rem 1rem;
          border-radius: 0.625rem;
          color: var(--sidebar-color);
          text-decoration: none;
          transition: all var(--transition-normal) var(--ease-out-expo);
          position: relative;
          overflow: hidden;
        }

        .nav-link::before {
          content: '';
          position: absolute;
          inset: 0;
          background: linear-gradient(135deg, rgba(255,255,255,0.1) 0%, transparent 100%);
          opacity: 0;
          transition: opacity var(--transition-fast);
        }

        .nav-link:hover {
          background-color: var(--sidebar-hover-bg);
          color: var(--pir9-blue);
          transform: translateX(4px);
        }

        .nav-link:hover::before {
          opacity: 1;
        }

        .nav-link.active {
          background: var(--sidebar-active-bg);
          color: var(--pir9-blue);
          box-shadow: inset 3px 0 0 var(--pir9-blue),
                      0 0 20px rgba(93, 156, 236, 0.15);
        }

        .nav-link.active .nav-icon {
          filter: drop-shadow(0 0 6px rgba(53, 197, 244, 0.5));
        }

        .nav-icon {
          flex-shrink: 0;
          width: 20px;
          height: 20px;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .nav-link:hover .nav-icon {
          transform: scale(1.1);
        }

        .nav-label {
          white-space: nowrap;
          overflow: hidden;
          font-weight: 500;
        }

        .collapse-toggle {
          display: flex;
          align-items: center;
          justify-content: center;
          padding: 0.75rem;
          margin-top: auto;
          border-radius: 0.625rem;
          color: var(--sidebar-color);
          background: var(--bg-card);
          border: 1px solid var(--border-glass);
          cursor: pointer;
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .collapse-toggle:hover {
          background-color: var(--sidebar-hover-bg);
          color: var(--pir9-blue);
          border-color: var(--pir9-blue);
          box-shadow: 0 0 15px rgba(53, 197, 244, 0.2);
        }

        .collapse-toggle svg {
          transition: transform var(--transition-normal) var(--ease-spring);
        }

        .rotate-180 {
          transform: rotate(180deg);
        }

        /* Hide labels when collapsed */
        :host-context(.sidebar-collapsed) .brand-text,
        :host-context(.sidebar-collapsed) .nav-label {
          display: none;
        }

        :host-context(.sidebar-collapsed) .nav-link {
          justify-content: center;
          padding: 0.75rem;
        }

        /* Mobile: hide collapse button */
        @media (max-width: 1023px) {
          .collapse-toggle {
            display: none;
          }
        }

        /* Keyframes for slide animation */
        @keyframes slideInLeft {
          from {
            opacity: 0;
            transform: translateX(-20px);
          }
          to {
            opacity: 1;
            transform: translateX(0);
          }
        }
      </style>
    `;
  }

  private renderNavItem(item: NavItem, collapsed: boolean): string {
    const active = isActive(item.path, item.exact);

    return html`
      <li class="nav-item">
        <a
          href="${item.path}"
          class="nav-link ${active ? 'active' : ''}"
          onclick="event.preventDefault(); this.closest('app-sidebar').handleNavClick('${item.path}')"
          title="${collapsed ? item.label : ''}"
        >
          ${this.renderIcon(item.icon)}
          ${collapsed ? '' : `<span class="nav-label">${item.label}</span>`}
        </a>
      </li>
    `;
  }

  private renderIcon(name: string): string {
    const icons: Record<string, string> = {
      home: '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path><polyline points="9 22 9 12 15 12 15 22"></polyline></svg>',
      tv: '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="7" width="20" height="15" rx="2" ry="2"></rect><polyline points="17 2 12 7 7 2"></polyline></svg>',
      calendar:
        '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="18" rx="2" ry="2"></rect><line x1="16" y1="2" x2="16" y2="6"></line><line x1="8" y1="2" x2="8" y2="6"></line><line x1="3" y1="10" x2="21" y2="10"></line></svg>',
      download:
        '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>',
      'alert-circle':
        '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"></circle><line x1="12" y1="8" x2="12" y2="12"></line><line x1="12" y1="16" x2="12.01" y2="16"></line></svg>',
      settings:
        '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>',
      anime:
        '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2L2 7l10 5 10-5-10-5z"></path><path d="M2 17l10 5 10-5"></path><path d="M2 12l10 5 10-5"></path></svg>',
      film: '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="2" width="20" height="20" rx="2.18" ry="2.18"></rect><line x1="7" y1="2" x2="7" y2="22"></line><line x1="17" y1="2" x2="17" y2="22"></line><line x1="2" y1="12" x2="22" y2="12"></line><line x1="2" y1="7" x2="7" y2="7"></line><line x1="2" y1="17" x2="7" y2="17"></line><line x1="17" y1="17" x2="22" y2="17"></line><line x1="17" y1="7" x2="22" y2="7"></line></svg>',
      laptop:
        '<svg class="nav-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="2" y1="20" x2="22" y2="20"></line></svg>',
    };

    return icons[name] || '';
  }

  handleToggle(): void {
    toggleSidebar();
  }

  handleNavClick(path: string): void {
    navigate(path);
    closeMobileMenu();
  }
}

// Make navigate available globally for onclick handlers
(window as unknown as { navigate: typeof navigate }).navigate = navigate;
