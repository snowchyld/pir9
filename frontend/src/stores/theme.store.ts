/**
 * Theme store for managing light/dark mode
 */

import { computed, effect, persistedSignal } from '../core/reactive';

export type Theme = 'light' | 'dark' | 'system';

/**
 * User's theme preference
 */
export const themePreference = persistedSignal<Theme>('theme', 'dark');

/**
 * System's color scheme preference
 */
const systemPrefersDark = (() => {
  if (typeof window === 'undefined') return true;
  return window.matchMedia('(prefers-color-scheme: dark)').matches;
})();

/**
 * Resolved theme (light or dark)
 */
export const resolvedTheme = computed(() => {
  const pref = themePreference.value;

  if (pref === 'system') {
    return systemPrefersDark ? 'dark' : 'light';
  }

  return pref;
});

/**
 * Apply theme to document
 */
effect(() => {
  const theme = resolvedTheme.value;
  document.documentElement.setAttribute('data-theme', theme);
});

/**
 * Set theme preference
 */
export function setTheme(theme: Theme): void {
  themePreference.set(theme);
}

/**
 * Toggle between light and dark
 */
export function toggleTheme(): void {
  const current = resolvedTheme.value;
  setTheme(current === 'dark' ? 'light' : 'dark');
}

/**
 * Listen for system theme changes
 */
if (typeof window !== 'undefined') {
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (_e) => {
    if (themePreference.value === 'system') {
      // Trigger re-evaluation of resolved theme
      themePreference.set('system');
    }
  });
}
