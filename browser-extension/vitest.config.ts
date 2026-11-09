/**
 * Vitest configuration for the browser extension: DOM-dependent specs run in jsdom.
 */
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "jsdom",
    globals: true,
    include: ["tests/**/*.test.ts"],
  },
});
