/**
 * Application entry point
 * Initializes all core systems and mounts the app
 */

// Import styles first
import './styles/base.css';

// Core systems
import { initializeRouter, installLinkHandler } from './router';
import { initializeWebSocket } from './core/websocket';

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

// Primitive components
import './components/primitives/progress-ring';
import './components/primitives/stats-card';

// Modal components
import './components/release-search-modal';

// Feature pages
// Series
import './features/series/series-index-page';
import './features/series/series-detail-page';

// Add Series
import './features/add-series/add-series-page';
import './features/add-series/import-series-page';

// Calendar
import './features/calendar/calendar-page';

// Activity
import './features/activity/queue-page';
import './features/activity/history-page';
import './features/activity/blocklist-page';

// Wanted
import './features/wanted/missing-page';
import './features/wanted/cutoff-unmet-page';

// Settings
import './features/settings/settings-page';
import './features/settings/media-management-settings';
import './features/settings/profiles-settings';
import './features/settings/quality-settings';
import './features/settings/custom-formats-settings';
import './features/settings/indexers-settings';
import './features/settings/download-clients-settings';
import './features/settings/import-lists-settings';
import './features/settings/connect-settings';
import './features/settings/metadata-settings';
import './features/settings/tags-settings';
import './features/settings/general-settings';
import './features/settings/ui-settings';
import './features/settings/imdb-settings';

// System
import './features/system/system-status-page';
import './features/system/system-tasks-page';
import './features/system/system-backup-page';
import './features/system/system-updates-page';
import './features/system/system-events-page';
import './features/system/system-logs-page';

// Not Found
import './features/not-found-page';

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
