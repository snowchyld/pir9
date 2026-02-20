import { defineConfig } from 'vite';
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';

export default defineConfig({
  plugins: [tailwindcss()],

  resolve: {
    alias: {
      '@': resolve(__dirname, './src'),
    },
  },

  server: {
    port: 3000,
    proxy: {
      '/api': {
        target: 'http://10.0.0.13:8989',
        changeOrigin: true,
      },
      '/ws': {
        // nosemgrep: javascript.lang.security.detect-insecure-websocket.detect-insecure-websocket
        target: 'ws://10.0.0.13:8989',
        ws: true,
      },
    },
  },

  build: {
    target: 'es2022',
    outDir: 'dist',
    rollupOptions: {
      output: {
        manualChunks: {
          query: ['@tanstack/query-core'],
          router: ['navigo'],
        },
      },
    },
  },
});
