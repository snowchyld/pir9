import { defineConfig, type PluginOption } from "vite";
import { visualizer } from "rollup-plugin-visualizer";
import tailwindcss from '@tailwindcss/vite';
import { resolve } from 'path';

/**
 * Plugin to replace Vite's __vitePreload wrapper with a simple passthrough.
 * The wrapper hangs in some environments when combined with Web Components
 * that register custom elements at module evaluation time.
 */
function stripVitePreload(): PluginOption {
  return {
    name: 'strip-vite-preload',
    enforce: 'post',
    generateBundle(_options, bundle) {
      for (const chunk of Object.values(bundle)) {
        if (chunk.type === 'chunk' && chunk.isEntry) {
          // Replace the __vitePreload function with a simple passthrough.
          // Vite wraps every dynamic import() in this function which can hang
          // in environments with Web Components and custom element registration.
          // Match: ,i=function(t,o,r){...lots of code...}
          // We find it by its unique signature and replace the body.
          const preloadPattern = /,i=function\(t,o,r\)\{let s=Promise\.resolve\(\);/;
          if (preloadPattern.test(chunk.code)) {
            // Find the start of the function body
            const match = chunk.code.match(/,i=function\(t,o,r\)\{/);
            if (match && match.index != null) {
              const start = match.index;
              // Find matching closing brace by counting braces
              let depth = 0;
              let end = start + match[0].length;
              // We're right after the opening {
              depth = 1;
              while (end < chunk.code.length && depth > 0) {
                if (chunk.code[end] === '{') depth++;
                if (chunk.code[end] === '}') depth--;
                end++;
              }
              // Replace the entire function with a passthrough
              chunk.code =
                chunk.code.substring(0, start) +
                ',i=function(t){return t()}' +
                chunk.code.substring(end);
            }
          }
        }
      }
    },
  };
}

export default defineConfig({
  plugins: [
	  tailwindcss(),
	  visualizer({ open: true } as PluginOption),
	  stripVitePreload(),
  ],


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
    modulePreload: { polyfill: false },
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
