import { defineConfig } from 'vite';
import path from 'path';

export default defineConfig({
  root: path.resolve(__dirname), // Serve from www directory
  publicDir: path.resolve(__dirname, 'public'),
  server: {
    port: 5179, // 使用不常用端口避免冲突
    open: false, // 不自动打开浏览器
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

