import '@testing-library/jest-dom/vitest';

// jsdom has no matchMedia. Answer media queries as a browser with "reduce
// motion" switched on, so animations (count-ups, transitions) settle
// immediately and assertions see final values — the same choice the
// Playwright suite makes. A test can replace window.matchMedia to try others.
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  window.matchMedia = (query: string) => ({
    matches: query.includes('prefers-reduced-motion: reduce'),
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  });
}
