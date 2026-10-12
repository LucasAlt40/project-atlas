import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Tauri expects a fixed dev-server port (see src-tauri/tauri.conf.json -> build.devUrl).
const DEV_PORT = 1420;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  clearScreen: false,
  build: {
    rollupOptions: {
      // Two pages from one codebase: the desktop app, and the phone's page that the app's own
      // server hands to a browser on the local network (ADR 0034).
      input: {
        main: fileURLToPath(new URL('./index.html', import.meta.url)),
        mobile: fileURLToPath(new URL('./mobile.html', import.meta.url)),
      },
    },
  },
  server: {
    port: DEV_PORT,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./src/test/setup.ts'],
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
