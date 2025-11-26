import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
  root: path.resolve(__dirname, '..'), // Serve from project root
  publicDir: path.resolve(__dirname, '../www/public'),
  server: {
    port: 3000,
    open: false,
  },
  build: {
    target: 'esnext',
    outDir: path.resolve(__dirname, '../dist'),
  },
  resolve: {
    alias: {
      '@wasm': path.resolve(__dirname, '../packages/core/wasm'),
    },
  },
});

