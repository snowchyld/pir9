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
  /** Lazy loader — dynamic import() that registers the custom element */
  load: () => Promise<unknown>;
}

// ── Lazy import factories ─────────────────────────────────────────
// Vite splits each dynamic import() into its own chunk automatically.
// The module's side-effect (@customElement decorator) registers the CE.

const dashboard = () => import('./features/dashboard/dashboard-page');
const seriesIndex = () => import('./features/series/series-index-page');
const seriesDetail = () => import('./features/series/series-detail-page');
const animeIndex = () => import('./features/anime/anime-index-page');
const moviesIndex = () => import('./features/movies/movies-index-page');
const movieDetail = () => import('./features/movies/movie-detail-page');
const addSeries = () => import('./features/add-series/add-series-page');
const importSeries = () => import('./features/add-series/import-series-page');
const addMovie = () => import('./features/add-movie/add-movie-page');
const importMovie = () => import('./features/add-movie/import-movie-page');
const musicIndex = () => import('./features/music/music-index-page');
const artistDetail = () => import('./features/music/artist-detail-page');
const albumDetail = () => import('./features/music/album-detail-page');
const addMusic = () => import('./features/add-music/add-music-page');
const podcastsIndex = () => import('./features/podcasts/podcasts-index-page');
const podcastDetail = () => import('./features/podcasts/podcast-detail-page');
const addPodcast = () => import('./features/add-podcast/add-podcast-page');
const audiobooksIndex = () => import('./features/audiobooks/audiobooks-index-page');
const audiobookDetail = () => import('./features/audiobooks/audiobook-detail-page');
const addAudiobook = () => import('./features/add-audiobook/add-audiobook-page');
const calendar = () => import('./features/calendar/calendar-page');
const queue = () => import('./features/activity/queue-page');
const importPreview = () => import('./features/activity/import-preview-page');
const history = () => import('./features/activity/history-page');
const blocklist = () => import('./features/activity/blocklist-page');
const wanted = () => import('./features/wanted/wanted-page');
const settingsPage = () => import('./features/settings/settings-page');
const mediaManagement = () => import('./features/settings/media-management-settings');
const rootFolders = () => import('./features/settings/root-folders-settings');
const profiles = () => import('./features/settings/profiles-settings');
const quality = () => import('./features/settings/quality-settings');
const customFormats = () => import('./features/settings/custom-formats-settings');
const indexers = () => import('./features/settings/indexers-settings');
const downloadClients = () => import('./features/settings/download-clients-settings');
const importLists = () => import('./features/settings/import-lists-settings');
const connect = () => import('./features/settings/connect-settings');
const metadata = () => import('./features/settings/metadata-settings');
const tags = () => import('./features/settings/tags-settings');
const general = () => import('./features/settings/general-settings');
const ui = () => import('./features/settings/ui-settings');
const imdb = () => import('./features/settings/imdb-settings');
const historySettings = () => import('./features/settings/history-settings');
const systemStatus = () => import('./features/system/system-status-page');
const systemTasks = () => import('./features/system/system-tasks-page');
const systemBackup = () => import('./features/system/system-backup-page');
const systemUpdates = () => import('./features/system/system-updates-page');
const systemEvents = () => import('./features/system/system-events-page');
const systemLogs = () => import('./features/system/system-logs-page');
const notFound = () => import('./features/not-found-page');

/**
 * Application routes
 */
export const routes: Route[] = [
  // Dashboard
  { path: '/', component: 'dashboard-page', title: 'Dashboard', load: dashboard },

  // Series
  { path: '/series', component: 'series-index-page', title: 'Series', load: seriesIndex },
  {
    path: '/series/:titleSlug',
    component: 'series-detail-page',
    title: 'Series',
    load: seriesDetail,
  },

  // Anime
  { path: '/anime', component: 'anime-index-page', title: 'Anime', load: animeIndex },

  // Movies
  { path: '/movies', component: 'movies-index-page', title: 'Movies', load: moviesIndex },
  { path: '/movies/:titleSlug', component: 'movie-detail-page', title: 'Movie', load: movieDetail },

  // Add Series
  { path: '/add/new', component: 'add-series-page', title: 'Add Series', load: addSeries },
  {
    path: '/add/import',
    component: 'import-series-page',
    title: 'Import Series',
    load: importSeries,
  },

  // Add Movies
  { path: '/add/movies', component: 'add-movie-page', title: 'Add Movie', load: addMovie },
  {
    path: '/add/movies/import',
    component: 'import-movie-page',
    title: 'Import Movies',
    load: importMovie,
  },

  // Music
  { path: '/music', component: 'music-index-page', title: 'Music', load: musicIndex },
  {
    path: '/music/:titleSlug',
    component: 'artist-detail-page',
    title: 'Artist',
    load: artistDetail,
  },
  {
    path: '/music/:titleSlug/album/:albumId',
    component: 'album-detail-page',
    title: 'Album',
    load: albumDetail,
  },

  // Add Music
  { path: '/add-music', component: 'add-music-page', title: 'Add Artist', load: addMusic },

  // Podcasts
  { path: '/podcasts', component: 'podcasts-index-page', title: 'Podcasts', load: podcastsIndex },
  {
    path: '/podcasts/:titleSlug',
    component: 'podcast-detail-page',
    title: 'Podcast',
    load: podcastDetail,
  },

  // Add Podcast
  { path: '/add-podcast', component: 'add-podcast-page', title: 'Add Podcast', load: addPodcast },

  // Audiobooks
  {
    path: '/audiobooks',
    component: 'audiobooks-index-page',
    title: 'Audiobooks',
    load: audiobooksIndex,
  },
  {
    path: '/audiobooks/:titleSlug',
    component: 'audiobook-detail-page',
    title: 'Audiobook',
    load: audiobookDetail,
  },

  // Add Audiobook
  {
    path: '/add-audiobook',
    component: 'add-audiobook-page',
    title: 'Add Audiobook',
    load: addAudiobook,
  },

  // Calendar
  { path: '/calendar', component: 'calendar-page', title: 'Calendar', load: calendar },

  // Activity
  { path: '/activity/queue', component: 'queue-page', title: 'Queue', load: queue },
  {
    path: '/activity/queue/:id/import',
    component: 'import-preview-page',
    title: 'Import Preview',
    load: importPreview,
  },
  { path: '/activity/history', component: 'history-page', title: 'History', load: history },
  { path: '/activity/blocklist', component: 'blocklist-page', title: 'Blocklist', load: blocklist },

  // Wanted
  { path: '/wanted', component: 'wanted-page', title: 'Wanted', load: wanted },
  { path: '/wanted/missing', component: 'wanted-page', title: 'Wanted', load: wanted },
  { path: '/wanted/cutoffunmet', component: 'wanted-page', title: 'Wanted', load: wanted },

  // Settings
  { path: '/settings', component: 'settings-page', title: 'Settings', load: settingsPage },
  {
    path: '/settings/mediamanagement',
    component: 'media-management-settings',
    title: 'Media Management',
    load: mediaManagement,
  },
  {
    path: '/settings/rootfolders',
    component: 'root-folders-settings',
    title: 'Root Folders',
    load: rootFolders,
  },
  { path: '/settings/profiles', component: 'profiles-settings', title: 'Profiles', load: profiles },
  { path: '/settings/quality', component: 'quality-settings', title: 'Quality', load: quality },
  {
    path: '/settings/customformats',
    component: 'custom-formats-settings',
    title: 'Custom Formats',
    load: customFormats,
  },
  { path: '/settings/indexers', component: 'indexers-settings', title: 'Indexers', load: indexers },
  {
    path: '/settings/downloadclients',
    component: 'download-clients-settings',
    title: 'Download Clients',
    load: downloadClients,
  },
  {
    path: '/settings/importlists',
    component: 'import-lists-settings',
    title: 'Import Lists',
    load: importLists,
  },
  { path: '/settings/connect', component: 'connect-settings', title: 'Connect', load: connect },
  { path: '/settings/metadata', component: 'metadata-settings', title: 'Metadata', load: metadata },
  { path: '/settings/tags', component: 'tags-settings', title: 'Tags', load: tags },
  { path: '/settings/general', component: 'general-settings', title: 'General', load: general },
  { path: '/settings/ui', component: 'ui-settings', title: 'UI', load: ui },
  { path: '/settings/data', component: 'imdb-settings', title: 'Data Sources', load: imdb },
  { path: '/settings/imdb', component: 'imdb-settings', title: 'Data Sources', load: imdb },
  {
    path: '/settings/history',
    component: 'history-settings',
    title: 'Import History',
    load: historySettings,
  },

  // System
  { path: '/system/status', component: 'system-status-page', title: 'Status', load: systemStatus },
  { path: '/system/tasks', component: 'system-tasks-page', title: 'Tasks', load: systemTasks },
  { path: '/system/backup', component: 'system-backup-page', title: 'Backup', load: systemBackup },
  {
    path: '/system/updates',
    component: 'system-updates-page',
    title: 'Updates',
    load: systemUpdates,
  },
  { path: '/system/events', component: 'system-events-page', title: 'Events', load: systemEvents },
  { path: '/system/logs', component: 'system-logs-page', title: 'Logs', load: systemLogs },
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

/** Track the current navigation to discard stale loads */
let navigationId = 0;

/**
 * Render a route's component (lazy-loads the module first)
 */
async function renderRoute(route: Route, params: RouteParams, queryString: string): Promise<void> {
  const outlet = getOutlet();
  if (!outlet) {
    console.error('[Router] No router-outlet found');
    return;
  }

  // Update state immediately (title, signals)
  const thisNav = ++navigationId;
  currentRoute.set(route);
  currentParams.set(params);
  currentQuery.set(new URLSearchParams(queryString));
  document.title = route.title ? `${route.title} | pir9` : 'pir9';

  // Lazy-load the component module (registers the custom element)
  await route.load();

  // If the user navigated away while we were loading, discard
  if (thisNav !== navigationId) return;

  // Create the component element
  const component = document.createElement(route.component);

  // Pass route params as attributes
  for (const [key, value] of Object.entries(params)) {
    component.setAttribute(key, value);
  }

  // Replace outlet content
  outlet.textContent = '';
  outlet.appendChild(component);
}

/**
 * Render 404 page
 */
async function renderNotFound(): Promise<void> {
  const outlet = getOutlet();
  if (!outlet) return;

  currentRoute.set(null);
  document.title = 'Not Found | pir9';

  await notFound();

  outlet.textContent = '';
  const el = document.createElement('not-found-page');
  outlet.appendChild(el);
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
