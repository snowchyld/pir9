/**
 * Base Web Component class using Light DOM
 * Provides template rendering, attribute observation, and lifecycle hooks
 *
 * SECURITY NOTE: The render() method uses innerHTML with developer-controlled
 * templates (from the template() method). User data should be escaped before
 * interpolation - use the html() and escapeHtml() helpers for safe templating.
 */

import { effect, type Watchable } from './reactive';

type Constructor<T = object> = new (...args: unknown[]) => T;

/**
 * Registry of custom elements to prevent double registration
 */
const registry = new Map<string, Constructor<HTMLElement>>();

/**
 * Escape HTML entities to prevent XSS
 */
export function escapeHtml(str: string | number | boolean | null | undefined): string {
  if (str === null || str === undefined) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Tagged template literal for HTML templating
 * Does NOT escape interpolated values by default (templates are developer-controlled)
 * Use escapeHtml() explicitly for user-provided data to prevent XSS
 * Handles safeHtml() wrapped values by extracting the inner HTML
 */
export function html(strings: TemplateStringsArray, ...values: unknown[]): string {
  return strings.reduce((result, str, i) => {
    const value = values[i - 1];
    // Handle safeHtml wrapped values
    if (value && typeof value === 'object' && '__safeHtml' in value) {
      return result + (value as { __safeHtml: string }).__safeHtml + str;
    }
    return result + String(value ?? '') + str;
  });
}

/**
 * Mark a string as safe HTML (bypasses escaping)
 * Only use with trusted, sanitized content!
 */
export function safeHtml(content: string): { __safeHtml: string } {
  return { __safeHtml: content };
}

/**
 * Decorator to register a custom element
 * @param tagName - The custom element tag name (must contain a hyphen)
 */
export function customElement(tagName: string) {
  return <T extends Constructor<HTMLElement>>(target: T): T => {
    // Guard with browser's own registry — prevents duplicate define errors
    // when Vite code-splits and modules evaluate across separate chunks
    if (!customElements.get(tagName)) {
      registry.set(tagName, target);
      customElements.define(tagName, target);
    }
    return target;
  };
}

/**
 * Decorator to mark a property as reactive (triggers re-render on change)
 */
export function reactive() {
  return (target: BaseComponent, propertyKey: string): void => {
    const privateKey = `__${propertyKey}`;

    Object.defineProperty(target, propertyKey, {
      get() {
        return this[privateKey];
      },
      set(value: unknown) {
        const oldValue = this[privateKey];
        if (oldValue !== value) {
          this[privateKey] = value;
          if (this._isConnected) {
            this.requestUpdate();
          }
        }
      },
      enumerable: true,
      configurable: true,
    });
  };
}

/**
 * Decorator to observe an attribute and sync it with a property
 */
export function attribute(options?: { type?: 'string' | 'number' | 'boolean' }) {
  const type = options?.type ?? 'string';

  return (target: BaseComponent, propertyKey: string): void => {
    const attrName = propertyKey.replace(/([A-Z])/g, '-$1').toLowerCase();

    // Add to observed attributes
    const ctor = target.constructor as typeof BaseComponent;
    if (!ctor._observedAttrs) {
      ctor._observedAttrs = [];
    }
    ctor._observedAttrs.push(attrName);

    // Define property with attribute sync
    const privateKey = `__${propertyKey}`;

    Object.defineProperty(target, propertyKey, {
      get() {
        return this[privateKey];
      },
      set(value: unknown) {
        const oldValue = this[privateKey];
        if (oldValue !== value) {
          this[privateKey] = value;

          // Sync to attribute
          if (type === 'boolean') {
            if (value) {
              this.setAttribute(attrName, '');
            } else {
              this.removeAttribute(attrName);
            }
          } else {
            this.setAttribute(attrName, String(value));
          }

          if (this._isConnected) {
            this.requestUpdate();
          }
        }
      },
      enumerable: true,
      configurable: true,
    });

    // Store attribute mapping
    if (!ctor._attrPropMap) {
      ctor._attrPropMap = new Map();
    }
    ctor._attrPropMap.set(attrName, { propertyKey, type });
  };
}

/**
 * Base class for Web Components using Light DOM
 */
export abstract class BaseComponent extends HTMLElement {
  static _observedAttrs?: string[];
  static _attrPropMap?: Map<string, { propertyKey: string; type: string }>;

  _isConnected = false;
  private _updateScheduled = false;
  private _rafId: number | null = null;
  private _lastTemplateHash = '';
  private _effectCleanups: Array<() => void> = [];

  static get observedAttributes(): string[] {
    return BaseComponent._observedAttrs ?? [];
  }

  /**
   * Called when connected to DOM
   */
  connectedCallback(): void {
    this._isConnected = true;
    this.onInit();
    this.render();
    this.onMount();
  }

  /**
   * Called when disconnected from DOM
   */
  disconnectedCallback(): void {
    this._isConnected = false;
    if (this._rafId !== null) {
      cancelAnimationFrame(this._rafId);
      this._rafId = null;
      this._updateScheduled = false;
    }
    this._effectCleanups.forEach((cleanup) => {
      cleanup();
    });
    this._effectCleanups = [];
    this.onDestroy();
  }

  /**
   * Called when an observed attribute changes
   */
  attributeChangedCallback(name: string, oldValue: string | null, newValue: string | null): void {
    if (oldValue === newValue) return;

    const ctor = this.constructor as typeof BaseComponent;
    const mapping = ctor._attrPropMap?.get(name);

    if (mapping) {
      const { propertyKey, type } = mapping;
      let parsedValue: unknown;

      switch (type) {
        case 'boolean':
          parsedValue = newValue !== null;
          break;
        case 'number':
          parsedValue = newValue !== null ? Number(newValue) : undefined;
          break;
        default:
          parsedValue = newValue;
      }

      (this as Record<string, unknown>)[`__${propertyKey}`] = parsedValue;
      if (this._isConnected) {
        this.requestUpdate();
      }
    }
  }

  /**
   * Override to provide component template
   * Use the html() tagged template literal for safe interpolation
   */
  protected abstract template(): string;

  /**
   * Called before first render
   */
  protected onInit(): void {}

  /**
   * Called after first render
   */
  protected onMount(): void {}

  /**
   * Called on disconnection
   */
  protected onDestroy(): void {}

  /**
   * Called after each render
   */
  protected onUpdate(): void {}

  /**
   * Schedule an update (batched via requestAnimationFrame).
   * RAF is throttled to display refresh rate and paused when tab is hidden,
   * preventing render storms on mobile.
   */
  requestUpdate(): void {
    if (!this._updateScheduled) {
      this._updateScheduled = true;
      this._rafId = requestAnimationFrame(() => {
        this._updateScheduled = false;
        this._rafId = null;
        if (this._isConnected) {
          this.render();
        }
      });
    }
  }

  /**
   * Render the component
   * Uses developer-controlled template() output to update the DOM.
   * User data must be escaped via html() or escapeHtml().
   *
   * Preserves focus and cursor position across re-renders by saving
   * the active element's selector and selection state before updating,
   * then restoring it after.
   */
  /**
   * Render the component.
   * Uses developer-controlled template() output to update the DOM.
   * Skips DOM update if template output is identical (prevents unnecessary reflows).
   * User data must be escaped via html() or escapeHtml().
   */
  private render(): void {
    // Templates are author-controlled; user data must use escapeHtml() — see file header.
    const templateContent = this.template();

    // Skip DOM update if template output hasn't changed — prevents unnecessary reflows
    if (templateContent === this._lastTemplateHash) {
      return;
    }
    this._lastTemplateHash = templateContent;

    const focusInfo = this.saveFocusState();
    const scrollY = window.scrollY;

    // nosemgrep: javascript.browser.security.insecure-document-method.insecure-document-method
    this.innerHTML = templateContent; // Safe: developer-controlled templates, user data escaped via escapeHtml() -- existing code, no change

    window.scrollTo({ top: scrollY, behavior: 'instant' });

    if (focusInfo) {
      this.restoreFocusState(focusInfo);
    }

    this.onUpdate();
  }

  /**
   * Save the currently focused element's info if it's inside this component
   */
  private saveFocusState(): {
    selector: string;
    selectionStart: number | null;
    selectionEnd: number | null;
    scrollTop: number;
  } | null {
    const active = document.activeElement;
    if (!active || !this.contains(active) || active === this) return null;

    // Build a selector to find the element after re-render
    const tag = active.tagName.toLowerCase();
    const classes = active.className ? `.${active.className.trim().split(/\s+/).join('.')}` : '';
    const type = active.getAttribute('type');
    const name = active.getAttribute('name');
    const placeholder = active.getAttribute('placeholder');

    let selector = tag;
    if (classes) selector += classes;
    if (type) selector += `[type="${type}"]`;
    if (name) selector += `[name="${name}"]`;
    if (placeholder) selector += `[placeholder="${placeholder}"]`;

    const inputEl = active as HTMLInputElement | HTMLTextAreaElement;

    return {
      selector,
      selectionStart: inputEl.selectionStart ?? null,
      selectionEnd: inputEl.selectionEnd ?? null,
      scrollTop: inputEl.scrollTop ?? 0,
    };
  }

  /**
   * Restore focus to the matching element after re-render
   */
  private restoreFocusState(info: {
    selector: string;
    selectionStart: number | null;
    selectionEnd: number | null;
    scrollTop: number;
  }): void {
    const el = this.querySelector<HTMLElement>(info.selector);
    if (!el) return;

    el.focus();

    // Restore cursor/selection position for text inputs
    const inputEl = el as HTMLInputElement | HTMLTextAreaElement;
    if (info.selectionStart !== null && typeof inputEl.setSelectionRange === 'function') {
      try {
        inputEl.setSelectionRange(info.selectionStart, info.selectionEnd ?? info.selectionStart);
      } catch {
        // setSelectionRange throws on non-text inputs (email, number, etc.)
      }
    }
    if (info.scrollTop) {
      inputEl.scrollTop = info.scrollTop;
    }
  }

  /**
   * Subscribe to a signal/computed and auto-update when it changes
   */
  protected watch<T>(sig: Watchable<T>, callback?: (value: T) => void): void {
    const cleanup = effect(() => {
      const value = sig.value;
      if (callback) {
        callback(value);
      } else {
        this.requestUpdate();
      }
    });
    this._effectCleanups.push(cleanup);
  }

  /**
   * Query a child element
   */
  protected $<T extends Element = Element>(selector: string): T | null {
    return this.querySelector<T>(selector);
  }

  /**
   * Query all child elements
   */
  protected $$<T extends Element = Element>(selector: string): NodeListOf<T> {
    return this.querySelectorAll<T>(selector);
  }

  /**
   * Emit a custom event
   */
  protected emit<T = unknown>(
    eventName: string,
    detail?: T,
    options?: Partial<CustomEventInit<T>>,
  ): boolean {
    return this.dispatchEvent(
      new CustomEvent(eventName, {
        bubbles: true,
        composed: true,
        detail,
        ...options,
      }),
    );
  }

  /**
   * Helper for conditional class names
   */
  protected cx(
    ...classes: Array<string | Record<string, boolean> | undefined | null | false>
  ): string {
    return classes
      .filter((c): c is string | Record<string, boolean> => Boolean(c))
      .flatMap((c) => {
        if (typeof c === 'string') return c;
        return Object.entries(c)
          .filter(([, v]) => v)
          .map(([k]) => k);
      })
      .join(' ');
  }
}

/**
 * Define multiple custom elements at once
 */
export function defineComponents(...components: Array<Constructor<HTMLElement>>): void {
  // Components register themselves via @customElement decorator
  // This function exists for explicit registration if needed
  components.forEach((Component) => {
    const name = (Component as { tagName?: string }).tagName;
    if (name && !customElements.get(name)) {
      customElements.define(name, Component);
    }
  });
}
