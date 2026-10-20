<script lang="ts">
  import Icon, { type IconName } from './components/Icon.svelte';
  import { loadAll, route, toast } from './lib/store.svelte';
  import Today from './views/Today.svelte';
  import Countdowns from './views/Countdowns.svelte';
  import Calendar from './views/Calendar.svelte';
  import Timetable from './views/Timetable.svelte';
  import Todos from './views/Todos.svelte';
  import Habits from './views/Habits.svelte';
  import Settings from './views/Settings.svelte';

  // 新增模块：在这里加一行 + 写一个 views/Xxx.svelte
  const routes: { path: string; label: string; icon: IconName; view: any }[] = [
    { path: '/', label: '今天', icon: 'home', view: Today },
    { path: '/todos', label: '待办', icon: 'todo', view: Todos },
    { path: '/calendar', label: '日历', icon: 'calendar', view: Calendar },
    { path: '/timetable', label: '课表', icon: 'grid', view: Timetable },
    { path: '/habits', label: '习惯', icon: 'flame', view: Habits },
    { path: '/countdowns', label: '倒数', icon: 'hourglass', view: Countdowns },
    { path: '/settings', label: '设置', icon: 'settings', view: Settings },
  ];
  const current = $derived(routes.find((r) => r.path === route.path) ?? routes[0]);

  loadAll();
  // 切回页面时刷新（多设备同步的最简实现）
  document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && loadAll());
</script>

<div class="shell">
  <nav>
    <a class="brand" href="#/">
      <svg viewBox="0 0 64 64" width="26" height="26"
        ><rect width="64" height="64" rx="16" fill="var(--accent)" /><path
          d="M20 33l8 8 16-18"
          fill="none"
          stroke="var(--on-accent)"
          stroke-width="6"
          stroke-linecap="round"
          stroke-linejoin="round"
        /></svg
      >
      <span>Congtodo</span>
    </a>
    {#each routes as r (r.path)}
      <a href="#{r.path}" class:active={r === current} aria-current={r === current ? 'page' : undefined}>
        <Icon name={r.icon} size={20} />
        <span>{r.label}</span>
      </a>
    {/each}
  </nav>

  <main>
    {#key current.path}
      <current.view />
    {/key}
  </main>
</div>

{#if toast.msg}
  <div class="toast" role="status">{toast.msg}</div>
{/if}

<style>
  .shell {
    min-height: 100dvh;
  }
  nav {
    position: fixed;
    inset: 0 auto 0 0;
    width: var(--nav-w);
    padding: 20px 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    border-right: 1px solid var(--border);
    background: var(--bg);
    z-index: 10;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 10px 20px;
    font-weight: 700;
    font-size: 17px;
    letter-spacing: -0.01em;
  }
  nav a:not(.brand) {
    display: flex;
    align-items: center;
    gap: 12px;
    height: 40px;
    padding: 0 12px;
    border-radius: 11px;
    color: var(--text-2);
    font-weight: 500;
    transition:
      background 0.15s,
      color 0.15s;
  }
  nav a:not(.brand):hover {
    background: var(--surface-2);
    color: var(--text);
  }
  nav a.active {
    background: var(--surface);
    color: var(--accent);
    box-shadow: var(--shadow);
    font-weight: 600;
  }
  nav a:last-child {
    margin-top: auto;
  }
  main {
    margin-left: var(--nav-w);
  }

  .toast {
    position: fixed;
    left: 50%;
    bottom: 32px;
    transform: translateX(-50%);
    background: var(--text);
    color: var(--bg);
    padding: 10px 18px;
    border-radius: 12px;
    font-size: 14px;
    box-shadow: var(--shadow-lg);
    z-index: 100;
    animation: up 0.25s var(--ease);
  }
  @keyframes up {
    from {
      opacity: 0;
      transform: translate(-50%, 10px);
    }
  }

  /* 移动端：底部标签栏 */
  @media (max-width: 760px) {
    nav {
      inset: auto 0 0 0;
      width: auto;
      height: calc(64px + env(safe-area-inset-bottom));
      padding: 6px 4px env(safe-area-inset-bottom);
      flex-direction: row;
      justify-content: space-around;
      border-right: 0;
      border-top: 1px solid var(--border);
      background: color-mix(in srgb, var(--bg) 85%, transparent);
      backdrop-filter: saturate(1.6) blur(16px);
    }
    .brand {
      display: none !important;
    }
    nav a:not(.brand) {
      flex: 1;
      flex-direction: column;
      justify-content: center;
      gap: 2px;
      height: 52px;
      padding: 0;
      font-size: 11px;
    }
    nav a.active {
      background: none;
      box-shadow: none;
    }
    nav a:last-child {
      margin-top: 0;
    }
    main {
      margin-left: 0;
    }
    .toast {
      bottom: calc(80px + env(safe-area-inset-bottom));
    }
  }
</style>
