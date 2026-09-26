// Adds DOM matchers such as `toBeDisabled()` / `toBeInTheDocument()`.
import '@testing-library/jest-dom/vitest';

// jsdom has no matchMedia; `svelte/motion` queries prefers-reduced-motion on import.
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string) =>
    ({
      matches: false,
      media: query,
      onchange: null,
      addEventListener: () => {},
      removeEventListener: () => {},
      addListener: () => {},
      removeListener: () => {},
      dispatchEvent: () => false
    }) as MediaQueryList;
}
