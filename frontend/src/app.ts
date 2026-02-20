/**
 * Root application component
 * Contains the main layout structure
 */

import { BaseComponent, customElement, html, safeHtml } from './core/component';
import { wsManager } from './core/websocket';
import { sidebarCollapsed, mobileMenuOpen, closeMobileMenu } from './stores/app.store';

@customElement('app-root')
export class AppRoot extends BaseComponent {
  protected onInit(): void {
    // Watch sidebar state
    this.watch(sidebarCollapsed);
    this.watch(mobileMenuOpen);

    // Close mobile menu on route change
    window.addEventListener('popstate', closeMobileMenu);
  }

  protected onMount(): void {
    // Connection state indicator
    this.watch(wsManager.connectionState, (state) => {
      const indicator = this.$('.connection-indicator');
      if (indicator) {
        indicator.className = `connection-indicator ${state}`;
      }
    });
  }

  protected onDestroy(): void {
    window.removeEventListener('popstate', closeMobileMenu);
  }

  protected template(): string {
    const collapsed = sidebarCollapsed.value;
    const mobileOpen = mobileMenuOpen.value;

    return html`
      <div class="app-layout ${collapsed ? 'sidebar-collapsed' : ''} ${mobileOpen ? 'mobile-menu-open' : ''}">
        <!-- Mobile overlay -->
        <div
          class="mobile-overlay fixed inset-0 bg-black/50 z-40 lg:hidden ${mobileOpen ? '' : 'hidden'}"
          onclick="this.closest('app-root').handleOverlayClick()"
        ></div>

        <!-- Sidebar -->
        <app-sidebar class="app-sidebar"></app-sidebar>

        <!-- Main content area -->
        <div class="app-main">
          <!-- Header -->
          <app-header class="app-header"></app-header>

          <!-- Page content -->
          <main class="app-content">
            <router-outlet></router-outlet>
          </main>

          <!-- Footer -->
          <footer class="app-footer">
            <div class="flex items-center justify-between px-4 py-2 text-sm text-[var(--text-color-muted)]">
              <div class="flex items-center gap-2">
                <span class="connection-indicator ${wsManager.connectionState.value}"></span>
                <span>pir9</span>
              </div>
              <div>
                <a href="https://github.com/pir9/pir9" target="_blank" rel="noopener">
                  GitHub
                </a>
              </div>
            </div>
          </footer>
        </div>

        <!-- Toast container -->
        <toast-container></toast-container>

        <!-- Modal container -->
        <modal-container></modal-container>
      </div>

      <style>
        .app-layout {
          display: flex;
          min-height: 100vh;
          background: var(--bg-page-gradient);
          position: relative;
        }

        /* Subtle animated gradient orbs in background */
        .app-layout::before {
          content: '';
          position: fixed;
          top: -50%;
          left: -50%;
          width: 100%;
          height: 100%;
          background: radial-gradient(
            circle at center,
            rgba(93, 156, 236, 0.08) 0%,
            transparent 50%
          );
          animation: float 20s ease-in-out infinite;
          pointer-events: none;
          z-index: 0;
        }

        .app-layout::after {
          content: '';
          position: fixed;
          bottom: -50%;
          right: -50%;
          width: 100%;
          height: 100%;
          background: radial-gradient(
            circle at center,
            rgba(53, 197, 244, 0.06) 0%,
            transparent 50%
          );
          animation: float 25s ease-in-out infinite reverse;
          pointer-events: none;
          z-index: 0;
        }

        @keyframes float {
          0%, 100% {
            transform: translate(0, 0);
          }
          50% {
            transform: translate(30px, 30px);
          }
        }

        .app-sidebar {
          position: fixed;
          left: 0;
          top: 0;
          bottom: 0;
          width: 220px;
          z-index: 50;
          border-right: 1px solid var(--border-glass);
          transition: all var(--transition-normal) var(--ease-out-expo);
        }

        .sidebar-collapsed .app-sidebar {
          width: 72px;
        }

        .app-main {
          flex: 1;
          margin-left: 220px;
          display: flex;
          flex-direction: column;
          min-height: 100vh;
          position: relative;
          z-index: 1;
          transition: margin-left var(--transition-normal) var(--ease-out-expo);
        }

        .sidebar-collapsed .app-main {
          margin-left: 72px;
        }

        .app-header {
          position: sticky;
          top: 0;
          z-index: 30;
        }

        .app-content {
          flex: 1;
          padding: 1.5rem;
          animation: fadeIn var(--transition-normal) var(--ease-out-expo);
        }

        .app-footer {
          background: var(--bg-footer);
          backdrop-filter: blur(var(--glass-blur));
          -webkit-backdrop-filter: blur(var(--glass-blur));
          border-top: 1px solid var(--border-glass);
        }

        .app-footer a {
          transition: color var(--transition-fast);
        }

        .app-footer a:hover {
          color: var(-pir9-blue);
        }

        /* Mobile overlay with blur */
        .mobile-overlay {
          backdrop-filter: blur(4px);
          -webkit-backdrop-filter: blur(4px);
          transition: opacity var(--transition-normal) var(--ease-out-expo);
        }

        /* Mobile styles */
        @media (max-width: 1023px) {
          .app-sidebar {
            transform: translateX(-100%);
            box-shadow: none;
          }

          .mobile-menu-open .app-sidebar {
            transform: translateX(0);
            box-shadow: 10px 0 40px rgba(0, 0, 0, 0.3);
          }

          .app-main {
            margin-left: 0;
          }

          .sidebar-collapsed .app-main {
            margin-left: 0;
          }

          .app-content {
            padding: 1rem;
          }
        }

        /* Connection indicator - with glow */
        .connection-indicator {
          width: 8px;
          height: 8px;
          border-radius: 50%;
          background-color: var(--color-gray-600);
          transition: all var(--transition-normal);
        }

        .connection-indicator.connected {
          background-color: var(--color-success);
          box-shadow: 0 0 8px rgba(39, 194, 76, 0.5);
        }

        .connection-indicator.connecting {
          background-color: var(--color-warning);
          box-shadow: 0 0 8px rgba(255, 144, 43, 0.5);
          animation: pulse 1s infinite;
        }

        .connection-indicator.disconnected,
        .connection-indicator.error {
          background-color: var(--color-danger);
          box-shadow: 0 0 8px rgba(240, 80, 80, 0.5);
        }

        @keyframes pulse {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.5; }
        }

        @keyframes fadeIn {
          from { opacity: 0; }
          to { opacity: 1; }
        }
      </style>
    `;
  }

  handleOverlayClick(): void {
    closeMobileMenu();
  }
}
