// Installs Testing Library DOM matchers and browser APIs that jsdom lacks for component tests.

import "@testing-library/jest-dom/vitest";

// Recharts' ResponsiveContainer observes its size; jsdom has no layout, so a no-op observer suffices.
class NoopResizeObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
}

globalThis.ResizeObserver ??= NoopResizeObserver;
