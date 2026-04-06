/**
 * Application entry point
 * Initializes all core systems and mounts the app
 */

// Import styles first
import './styles/base.css';

import { initializeWebSocket } from './core/websocket';
// Core systems
import { initializeRouter, installLinkHandler } from './router';

// Stores (side effects run on import)
import './stores/theme.store';
import './stores/app.store';

// Root component
import './app';

// Layout components
import './components/layout/app-sidebar';
import './components/layout/app-header';
import './components/layout/toast-container';
import './components/layout/modal-container';
import './components/layout/router-outlet';

// Primitive components (shared, keep eager)
import './components/primitives/progress-ring';
import './components/primitives/stats-card';

// All feature page components are lazy-loaded by the router via dynamic import().

/**
 * Initialize the application
 */
function init(): void {
  console.log('[App] Initializing...');

  // Install global link click handler for SPA navigation
  installLinkHandler();

  // Initialize WebSocket connection
  initializeWebSocket();

  // Initialize router (must be last - starts rendering)
  initializeRouter();

  console.log('[App] Initialized');
}

// Wait for DOM ready
if (document.readyState === 'loading') {
  document.addEventListener('DOMContentLoaded', init);
} else {
  init();
}
