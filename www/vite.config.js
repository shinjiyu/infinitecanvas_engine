import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
  root: path.resolve(__dirname), // Serve from www directory
  publicDir: path.resolve(__dirname, 'public'),
  server: {
    port: 3000,
    open: '/test-custom-image.html', // 自动打开自定义图片页面
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

