import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    exclude: ['scripts/**', 'node_modules/**', 'dist/**'],
    restoreMocks: true,
  },
});
