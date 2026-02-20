/**
 * Minimal reactive signals implementation (~1KB minified)
 * Provides signal(), computed(), and effect() primitives
 */

type Subscriber = () => void;
type CleanupFn = () => void;

let currentEffect: Subscriber | null = null;
let batchDepth = 0;
const pendingEffects = new Set<Subscriber>();

/**
 * Base interface for reactive values (both signals and computed)
 */
export interface Watchable<T> {
  readonly value: T;
  subscribe(fn: Subscriber): CleanupFn;
}

/**
 * A reactive signal that notifies subscribers when its value changes
 */
export interface Signal<T> extends Watchable<T> {
  set(value: T): void;
  update(fn: (current: T) => T): void;
}

/**
 * A read-only computed signal derived from other signals
 */
export interface Computed<T> extends Watchable<T> {}

/**
 * Create a reactive signal with an initial value
 */
export function signal<T>(initialValue: T): Signal<T> {
  let value = initialValue;
  const subscribers = new Set<Subscriber>();

  const notify = () => {
    if (batchDepth > 0) {
      subscribers.forEach((sub) => pendingEffects.add(sub));
    } else {
      subscribers.forEach((sub) => sub());
    }
  };

  return {
    get value(): T {
      if (currentEffect) {
        subscribers.add(currentEffect);
      }
      return value;
    },

    set(newValue: T): void {
      if (!Object.is(value, newValue)) {
        value = newValue;
        notify();
      }
    },

    update(fn: (current: T) => T): void {
      this.set(fn(value));
    },

    subscribe(fn: Subscriber): CleanupFn {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
  };
}

/**
 * Create a computed signal derived from other signals
 */
export function computed<T>(fn: () => T): Computed<T> {
  let cachedValue: T;
  let dirty = true;
  const subscribers = new Set<Subscriber>();

  const recompute = () => {
    dirty = true;
    if (batchDepth > 0) {
      subscribers.forEach((sub) => pendingEffects.add(sub));
    } else {
      subscribers.forEach((sub) => sub());
    }
  };

  // Track dependencies by running the function
  const track = () => {
    const prevEffect = currentEffect;
    currentEffect = recompute;
    try {
      cachedValue = fn();
      dirty = false;
    } finally {
      currentEffect = prevEffect;
    }
  };

  return {
    get value(): T {
      if (dirty) {
        track();
      }
      if (currentEffect) {
        subscribers.add(currentEffect);
      }
      return cachedValue;
    },

    subscribe(fn: Subscriber): CleanupFn {
      subscribers.add(fn);
      return () => subscribers.delete(fn);
    },
  };
}

/**
 * Create a side effect that runs when its dependencies change
 */
export function effect(fn: () => void | CleanupFn): CleanupFn {
  let cleanup: void | CleanupFn;

  const execute = () => {
    // Run cleanup from previous execution
    if (typeof cleanup === 'function') {
      cleanup();
    }

    const prevEffect = currentEffect;
    currentEffect = execute;
    try {
      cleanup = fn();
    } finally {
      currentEffect = prevEffect;
    }
  };

  execute();

  return () => {
    if (typeof cleanup === 'function') {
      cleanup();
    }
  };
}

/**
 * Batch multiple signal updates to avoid unnecessary re-renders
 */
export function batch(fn: () => void): void {
  batchDepth++;
  try {
    fn();
  } finally {
    batchDepth--;
    if (batchDepth === 0) {
      const effects = [...pendingEffects];
      pendingEffects.clear();
      effects.forEach((effect) => effect());
    }
  }
}

/**
 * Create a signal from localStorage with automatic persistence
 */
export function persistedSignal<T>(
  key: string,
  initialValue: T
): Signal<T> {
  const stored = localStorage.getItem(key);
  const initial = stored ? (JSON.parse(stored) as T) : initialValue;
  const sig = signal(initial);

  // Wrap set to persist
  const originalSet = sig.set.bind(sig);
  sig.set = (value: T) => {
    localStorage.setItem(key, JSON.stringify(value));
    originalSet(value);
  };

  return sig;
}
