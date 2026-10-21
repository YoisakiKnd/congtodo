<script lang="ts">
  import { CalendarDays, CheckSquare, Flame, Hourglass, House, LayoutGrid, LogOut, Settings as SettingsIcon, Users } from '@lucide/svelte';
  import { ModeWatcher } from 'mode-watcher';
  import { Toaster } from '$lib/components/ui/sonner';
  import { auth, checkAuth, loadAll, route, signOut } from '$lib/store.svelte';
  import { cn } from '$lib/utils';
  import Today from './views/Today.svelte';
  import Countdowns from './views/Countdowns.svelte';
  import Calendar from './views/Calendar.svelte';
  import Timetable from './views/Timetable.svelte';
  import Todos from './views/Todos.svelte';
  import Habits from './views/Habits.svelte';
  import Settings from './views/Settings.svelte';
  import Login from './views/Login.svelte';
  import Admin from './views/Admin.svelte';

  // 新增模块：在这里加一行 + 写一个 views/Xxx.svelte
  const routes = [
    { path: '/', label: '今天', icon: House, view: Today },
    { path: '/todos', label: '待办', icon: CheckSquare, view: Todos },
    { path: '/calendar', label: '日历', icon: CalendarDays, view: Calendar },
    { path: '/timetable', label: '课表', icon: LayoutGrid, view: Timetable },
    { path: '/habits', label: '习惯', icon: Flame, view: Habits },
    { path: '/countdowns', label: '倒数', icon: Hourglass, view: Countdowns },
    { path: '/settings', label: '设置', icon: SettingsIcon, view: Settings },
    // admin：仅管理员可见；移动端底栏放不下，从设置页进入
    { path: '/admin', label: '用户', icon: Users, view: Admin, admin: true },
  ];
  const visible = $derived(routes.filter((r) => !r.admin || auth.user?.is_admin));
  const current = $derived(visible.find((r) => r.path === route.path) ?? visible[0]);

  checkAuth();
  // 切回页面时刷新（多设备同步的最简实现）
  document.addEventListener('visibilitychange', () => document.visibilityState === 'visible' && auth.user && loadAll());
</script>

<ModeWatcher />
<Toaster richColors position="top-center" />

{#if !auth.checked}
  <!-- 确认登录状态前不渲染，避免闪现登录页 -->
{:else if !auth.user}
  <Login />
{:else}

<!-- 桌面端侧栏 -->
<aside class="bg-sidebar fixed inset-y-0 left-0 z-20 hidden w-56 flex-col gap-1 border-r p-3 md:flex">
  <a href="#/" class="mb-4 flex items-center gap-2.5 px-2 py-1.5 text-base font-semibold">
    <span class="bg-primary text-primary-foreground grid size-7 place-items-center rounded-lg">
      <CheckSquare class="size-4" />
    </span>
    Congtodo
  </a>
  {#each visible as r (r.path)}
    <a
      href="#{r.path}"
      aria-current={r === current ? 'page' : undefined}
      class={cn(
        'text-muted-foreground hover:bg-sidebar-accent hover:text-foreground flex h-9 items-center gap-3 rounded-md px-3 text-sm font-medium transition-colors',
        r === current && 'bg-sidebar-accent text-foreground',
        r.path === '/settings' && 'mt-auto',
      )}
    >
      <r.icon class={cn('size-4', r === current && 'text-primary')} />
      {r.label}
    </a>
  {/each}
  <div class="mt-2 flex items-center gap-2 border-t px-2 pt-3">
    <span class="bg-primary/15 text-primary grid size-7 shrink-0 place-items-center rounded-full text-xs font-semibold uppercase">
      {auth.user.username.slice(0, 1)}
    </span>
    <span class="min-w-0 flex-1 truncate text-sm">{auth.user.username}</span>
    <button class="text-muted-foreground hover:text-foreground p-1" title="退出登录" onclick={signOut}>
      <LogOut class="size-4" />
    </button>
  </div>
</aside>

<main class="pb-24 md:pb-0 md:pl-56">
  {#key current.path}
    <current.view />
  {/key}
</main>

<!-- 移动端底栏 -->
<nav
  class="bg-background/85 fixed inset-x-0 bottom-0 z-20 flex border-t pb-[env(safe-area-inset-bottom)] backdrop-blur-lg md:hidden"
>
  {#each routes.filter((r) => !r.admin) as r (r.path)}
    <a
      href="#{r.path}"
      aria-current={r === current ? 'page' : undefined}
      class={cn(
        'text-muted-foreground flex h-14 flex-1 flex-col items-center justify-center gap-0.5 text-[11px]',
        r === current && 'text-primary font-medium',
      )}
    >
      <r.icon class="size-5" />
      {r.label}
    </a>
  {/each}
</nav>
{/if}
