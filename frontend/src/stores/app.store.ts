/**
 * Application-level state store
 */

import { persistedSignal, signal } from '../core/reactive';

/**
 * Sidebar collapsed state
 */
export const sidebarCollapsed = persistedSignal('sidebar-collapsed', false);

/**
 * Toggle sidebar
 */
export function toggleSidebar(): void {
  sidebarCollapsed.update((v) => !v);
}

/**
 * Mobile menu open state
 */
export const mobileMenuOpen = signal(false);

/**
 * Toggle mobile menu
 */
export function toggleMobileMenu(): void {
  mobileMenuOpen.update((v) => !v);
}

/**
 * Close mobile menu
 */
export function closeMobileMenu(): void {
  mobileMenuOpen.set(false);
}

/**
 * Series view mode
 */
export type ViewMode = 'posters' | 'posters-small' | 'overview' | 'table';

export const seriesViewMode = persistedSignal<ViewMode>('series-view-mode', 'posters');

/**
 * Set series view mode
 */
export function setSeriesViewMode(mode: ViewMode): void {
  seriesViewMode.set(mode);
}

/**
 * Series sort key
 */
export type SeriesSortKey =
  | 'sortTitle'
  | 'status'
  | 'network'
  | 'qualityProfileId'
  | 'nextAiring'
  | 'previousAiring'
  | 'added'
  | 'year'
  | 'path'
  | 'sizeOnDisk'
  | 'seasonCount'
  | 'episodeProgress'
  | 'ratings';

export const seriesSortKey = persistedSignal<SeriesSortKey>('series-sort-key', 'sortTitle');
export const seriesSortDirection = persistedSignal<'ascending' | 'descending'>(
  'series-sort-direction',
  'ascending',
);

/**
 * Set series sort
 */
export function setSeriesSort(key: SeriesSortKey, direction?: 'ascending' | 'descending'): void {
  if (key === seriesSortKey.value && !direction) {
    // Toggle direction
    seriesSortDirection.update((d) => (d === 'ascending' ? 'descending' : 'ascending'));
  } else {
    seriesSortKey.set(key);
    if (direction) {
      seriesSortDirection.set(direction);
    }
  }
}

/**
 * Series filter
 */
export const seriesFilter = persistedSignal<string>('series-filter', 'all');

/**
 * Set series filter
 */
export function setSeriesFilter(filter: string): void {
  seriesFilter.set(filter);
}

/**
 * Series network filter — persisted so it survives navigation
 */
export const seriesNetworkFilter = persistedSignal<string>('series-network-filter', 'all');

export function setSeriesNetworkFilter(network: string): void {
  seriesNetworkFilter.set(network);
}

/**
 * Movie view mode
 */
export const movieViewMode = persistedSignal<ViewMode>('movie-view-mode', 'posters');

export function setMovieViewMode(mode: ViewMode): void {
  movieViewMode.set(mode);
}

/**
 * Movie sort key
 */
export type MovieSortKey =
  | 'sortTitle'
  | 'status'
  | 'studio'
  | 'added'
  | 'year'
  | 'path'
  | 'sizeOnDisk'
  | 'ratings';

export const movieSortKey = persistedSignal<MovieSortKey>('movie-sort-key', 'sortTitle');
export const movieSortDirection = persistedSignal<'ascending' | 'descending'>(
  'movie-sort-direction',
  'ascending',
);

export function setMovieSort(key: MovieSortKey, direction?: 'ascending' | 'descending'): void {
  if (key === movieSortKey.value && !direction) {
    movieSortDirection.update((d) => (d === 'ascending' ? 'descending' : 'ascending'));
  } else {
    movieSortKey.set(key);
    if (direction) {
      movieSortDirection.set(direction);
    }
  }
}

/**
 * Movie filter
 */
export const movieFilter = persistedSignal<string>('movie-filter', 'all');

export function setMovieFilter(filter: string): void {
  movieFilter.set(filter);
}

/**
 * Global search query
 */
export const searchQuery = signal('');

/**
 * Set search query
 */
export function setSearchQuery(query: string): void {
  searchQuery.set(query);
}

/**
 * Clear search
 */
export function clearSearch(): void {
  searchQuery.set('');
}

/**
 * Active modal state
 */
export interface ModalState {
  type: string;
  props?: Record<string, unknown>;
}

export const activeModal = signal<ModalState | null>(null);

/**
 * Open a modal
 */
export function openModal(type: string, props?: Record<string, unknown>): void {
  activeModal.set({ type, props });
}

/**
 * Close the active modal
 */
export function closeModal(): void {
  activeModal.set(null);
}

/**
 * Toast notifications
 */
export interface Toast {
  id: string;
  type: 'info' | 'success' | 'warning' | 'error';
  title?: string;
  message: string;
  duration?: number;
}

export const toasts = signal<Toast[]>([]);

let toastId = 0;

/**
 * Show a toast notification
 */
export function showToast(toast: Omit<Toast, 'id'>, duration = 5000): string {
  const id = `toast-${++toastId}`;

  toasts.update((t) => [...t, { ...toast, id, duration }]);

  if (duration > 0) {
    setTimeout(() => {
      dismissToast(id);
    }, duration);
  }

  return id;
}

/**
 * Dismiss a toast
 */
export function dismissToast(id: string): void {
  toasts.update((t) => t.filter((toast) => toast.id !== id));
}

/**
 * Show success toast
 */
export function showSuccess(message: string, title?: string): string {
  return showToast({ type: 'success', message, title });
}

/**
 * Show error toast
 */
export function showError(message: string, title?: string): string {
  return showToast({ type: 'error', message, title }, 8000);
}

/**
 * Show warning toast
 */
export function showWarning(message: string, title?: string): string {
  return showToast({ type: 'warning', message, title });
}

/**
 * Show info toast
 */
export function showInfo(message: string, title?: string): string {
  return showToast({ type: 'info', message, title });
}
