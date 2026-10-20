import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],
  server: {
    // 开发时把 API 转发给 Rust 后端
    proxy: { '/api': 'http://127.0.0.1:8080' },
  },
  build: {
    target: 'es2022',
    cssMinify: true,
    modulePreload: { polyfill: false },
  },
});
