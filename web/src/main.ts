import 'temporal-polyfill/global';
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

mount(App, { target: document.getElementById('app')! });

// PWA：离线缓存 + 浏览器推送（开发模式下不注册，避免缓存干扰热更新）
if ('serviceWorker' in navigator && import.meta.env.PROD) {
  navigator.serviceWorker.register('/sw.js').catch(() => {});
}
