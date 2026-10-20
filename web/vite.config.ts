import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [tailwindcss(), svelte()],
  resolve: {
    alias: { $lib: fileURLToPath(new URL('./src/lib', import.meta.url)) },
  },
  server: {
    // 开发时把 API 转发给 Rust 后端
    proxy: { '/api': 'http://127.0.0.1:8080' },
  },
  build: {
    target: 'es2022',
  },
});
