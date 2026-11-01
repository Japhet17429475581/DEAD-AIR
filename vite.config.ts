import { defineConfig } from 'vitest/config';

export default defineConfig({
  server: { port: 1420, strictPort: true },
  preview: { port: 1420, strictPort: true },
  test: { include: ['src/**/*.test.ts'] },
  clearScreen: false,
});
