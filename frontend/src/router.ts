/**
 * Client-side router using Navigo
 * Handles navigation and route-based component rendering
 */

import Navigo from 'navigo';
import { type Signal, signal } from './core/reactive';

export interface RouteParams {
  [key: string]: string;
}

export interface RouteMatch {
  url: string;
  params: RouteParams;
  queryString: string;
}

export interface Route {
  path: string;
  component: string; // Custom element tag name
  title?: string;
}

/**
 * Application routes matching the existing React frontend
 */
export const routes: Route[] = [
  // Dashboard
  { path: '/', component: 'dashboard-page', title: 'Dashboard' },

  // Series
  { path: '/series', component: 'series-index-page', title: 'Series' },
  { path: '/series/:titleSlug', component: 'series-detail-page', title: 'Series' },

  // Anime
  { path: '/anime', component: 'anime-index-page', title: 'Anime' },

  // Movies
  { path: '/movies', component: 'movies-index-page', title: 'Movies' },
  { path: '/movies/:titleSlug', component: 'movie-detail-page', title: 'Movie' },

  // Add Series
  { path: '/add/new', component: 'add-series-page', title: 'Add Series' },
  { path: '/add/import', component: 'import-series-page', title: 'Import Series' },

  // Add Movies
  { path: '/add/movies', component: 'add-movie-page', title: 'Add Movie' },
  { path: '/add/movies/import', component: 'import-movie-page', title: 'Import Movies' },

  // Calendar
  { path: '/calendar', component: 'calendar-page', title: 'Calendar' },

  // Activity
  { path: '/activity/queue', component: 'queue-page', title: 'Queue' },
  { path: '/activity/queue/:id/import', component: 'import-preview-page', title: 'Import Preview' },
  { path: '/activity/history', component: 'history-page', title: 'History' },
  { path: '/activity/blocklist', component: 'blocklist-page', title: 'Blocklist' },

  // Wanted
  { path: '/wanted/missing', component: 'missing-page', title: 'Missing' },
  { path: '/wanted/cutoffunmet', component: 'cutoff-unmet-page', title: 'Cutoff Unmet' },

  // Settings
  { path: '/settings', component: 'settings-page', title: 'Settings' },
  {
    path: '/settings/mediamanagement',
    component: 'media-management-settings',
    title: 'Media Management',
  },
  { path: '/settings/profiles', component: 'profiles-settings', title: 'Profiles' },
  { path: '/settings/quality', component: 'quality-settings', title: 'Quality' },
  {
    path: '/settings/customformats',
    component: 'custom-formats-settings',
    title: 'Custom Formats',
  },
  { path: '/settings/indexers', component: 'indexers-settings', title: 'Indexers' },
  {
    path: '/settings/downloadclients',
    component: 'download-clients-settings',
    title: 'Download Clients',
  },
  { path: '/settings/importlists', component: 'import-lists-settings', title: 'Import Lists' },
  { path: '/settings/connect', component: 'connect-settings', title: 'Connect' },
  { path: '/settings/metadata', component: 'metadata-settings', title: 'Metadata' },
  { path: '/settings/tags', component: 'tags-settings', title: 'Tags' },
  { path: '/settings/general', component: 'general-settings', title: 'General' },
  { path: '/settings/ui', component: 'ui-settings', title: 'UI' },
  { path: '/settings/imdb', component: 'imdb-settings', title: 'IMDB' },
  {
    path: '/settings/history',
    component: 'history-settings',
    title: 'Import History',
  },

  // System
  { path: '/system/status', component: 'system-status-page', title: 'Status' },
  { path: '/system/tasks', component: 'system-tasks-page', title: 'Tasks' },
  { path: '/system/backup', component: 'system-backup-page', title: 'Backup' },
  { path: '/system/updates', component: 'system-updates-page', title: 'Updates' },
  { path: '/system/events', component: 'system-events-page', title: 'Events' },
  { path: '/system/logs', component: 'system-logs-page', title: 'Logs' },
];

/**
 * Current route state
 */
export const currentRoute: Signal<Route | null> = signal(null);
export const currentParams: Signal<RouteParams> = signal({});
export const currentQuery: Signal<URLSearchParams> = signal(new URLSearchParams());

/**
 * Router instance
 */
let router: Navigo | null = null;

/**
 * Get the router outlet element
 */
function getOutlet(): HTMLElement | null {
  return document.querySelector('router-outlet');
}

/**
 * Render a route's component
 */
function renderRoute(route: Route, params: RouteParams, queryString: string): void {
  const outlet = getOutlet();
  if (!outlet) {
    console.error('[Router] No router-outlet found');
    return;
  }

  // Update state
  currentRoute.set(route);
  currentParams.set(params);
  currentQuery.set(new URLSearchParams(queryString));

  // Update document title
  document.title = route.title ? `${route.title} | pir9` : 'pir9';

  // Create the component element
  const component = document.createElement(route.component);

  // Pass route params as attributes
  Object.entries(params).forEach(([key, value]) => {
    component.setAttribute(key, value);
  });

  // Replace outlet content
  outlet.textContent = '';
  outlet.appendChild(component);
}

/**
 * Render 404 page
 */
function renderNotFound(): void {
  const outlet = getOutlet();
  if (!outlet) return;

  currentRoute.set(null);
  document.title = 'Not Found | pir9';

  outlet.textContent = '';
  const notFound = document.createElement('not-found-page');
  outlet.appendChild(notFound);
}

/**
 * Initialize the router
 */
export function initializeRouter(): Navigo {
  if (router) {
    return router;
  }

  // Create Navigo instance
  router = new Navigo('/', { hash: false });

  // Register routes
  routes.forEach((route) => {
    router?.on(route.path, (match) => {
      if (match) {
        renderRoute(route, match.data ?? {}, match.queryString ?? '');
      }
    });
  });

  // Handle 404
  router.notFound(() => {
    renderNotFound();
  });

  // Wait for router-outlet to exist before resolving
  // Custom elements need time to upgrade and render
  waitForOutlet().then(() => {
    router?.resolve();
  });

  return router;
}

/**
 * Wait for router-outlet to be available in the DOM
 */
function waitForOutlet(): Promise<void> {
  return new Promise((resolve) => {
    const outlet = getOutlet();
    if (outlet) {
      resolve();
      return;
    }

    // Poll for outlet (custom elements may not be upgraded yet)
    const checkInterval = setInterval(() => {
      if (getOutlet()) {
        clearInterval(checkInterval);
        resolve();
      }
    }, 10);

    // Timeout after 5 seconds
    setTimeout(() => {
      clearInterval(checkInterval);
      console.error('[Router] Timeout waiting for router-outlet');
      resolve();
    }, 5000);
  });
}

/**
 * Navigate to a path
 */
export function navigate(path: string, options?: { replace?: boolean }): void {
  if (!router) {
    console.error('[Router] Router not initialized');
    return;
  }

  if (options?.replace) {
    router.navigate(path, { historyAPIMethod: 'replaceState' });
  } else {
    router.navigate(path);
  }
}

/**
 * Navigate back in history
 */
export function back(): void {
  window.history.back();
}

/**
 * Navigate forward in history
 */
export function forward(): void {
  window.history.forward();
}

/**
 * Get current path
 */
export function getCurrentPath(): string {
  return window.location.pathname;
}

/**
 * Check if a path is active (for navigation highlighting)
 */
export function isActive(path: string, exact = false): boolean {
  const currentPath = getCurrentPath();

  if (exact) {
    return currentPath === path;
  }

  return currentPath.startsWith(path);
}

/**
 * Link click handler - use on anchor elements
 * Prevents default navigation and uses router instead
 */
export function handleLinkClick(event: MouseEvent): void {
  // Only handle left clicks without modifiers
  if (event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) {
    return;
  }

  const target = event.target as HTMLElement;
  const anchor = target.closest('a[href]') as HTMLAnchorElement | null;

  if (!anchor) return;

  const href = anchor.getAttribute('href');

  // Skip external links and special protocols
  if (
    !href ||
    href.startsWith('http') ||
    href.startsWith('//') ||
    href.startsWith('mailto:') ||
    href.startsWith('tel:') ||
    anchor.target === '_blank'
  ) {
    return;
  }

  event.preventDefault();
  navigate(href);
}

/**
 * Install global link click handler
 */
export function installLinkHandler(): void {
  document.addEventListener('click', handleLinkClick);
}

/**
 * Generate a URL with query parameters
 */
export function buildUrl(
  path: string,
  params?: Record<string, string | number | boolean | undefined>,
): string {
  const url = new URL(path, window.location.origin);

  if (params) {
    Object.entries(params).forEach(([key, value]) => {
      if (value !== undefined) {
        url.searchParams.set(key, String(value));
      }
    });
  }

  return url.pathname + url.search;
}
