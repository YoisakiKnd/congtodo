<script lang="ts">
  import {
    CalendarDays,
    CheckSquare,
    Ellipsis,
    Flame,
    Hourglass,
    House,
    LayoutGrid,
    LogOut,
    Settings as SettingsIcon,
    Timer,
    Users,
  } from '@lucide/svelte';
  import { ModeWatcher } from 'mode-watcher';
  import * as Drawer from '$lib/components/ui/drawer';
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
  import Focus from './views/Focus.svelte';

  // 新增模块：在这里加一行 + 写一个 views/Xxx.svelte
  // tab：移动端底栏直接显示的入口，其余收进「更多」；admin：仅管理员可见
  const routes = [
    { path: '/', label: '今天', icon: House, view: Today, tab: true },
    { path: '/todos', label: '待办', icon: CheckSquare, view: Todos, tab: true },
    { path: '/calendar', label: '日历', icon: CalendarDays, view: Calendar, tab: true },
    { path: '/timetable', label: '课表', icon: LayoutGrid, view: Timetable, tab: true },
    { path: '/habits', label: '习惯', icon: Flame, view: Habits },
    { path: '/countdowns', label: '倒数', icon: Hourglass, view: Countdowns },
    { path: '/focus', label: '专注', icon: Timer, view: Focus },
    { path: '/settings', label: '设置', icon: SettingsIcon, view: Settings },
    { path: '/admin', label: '用户', icon: Users, view: Admin, admin: true },
  ];
  const visible = $derived(routes.filter((r) => !r.admin || auth.user?.is_admin));
  const current = $derived(visible.find((r) => r.path === route.path) ?? visible[0]);
  const more = $derived(visible.filter((r) => !r.tab));
  let moreOpen = $state(false);

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
  {#each visible.filter((r) => r.tab) as r (r.path)}
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
  <button
    class={cn(
      'text-muted-foreground flex h-14 flex-1 flex-col items-center justify-center gap-0.5 text-[11px]',
      more.includes(current) && 'text-primary font-medium',
    )}
    onclick={() => (moreOpen = true)}
  >
    <Ellipsis class="size-5" />
    {more.includes(current) ? current.label : '更多'}
  </button>
</nav>

<Drawer.Root bind:open={moreOpen}>
  <Drawer.Content>
    <Drawer.Header><Drawer.Title>更多</Drawer.Title></Drawer.Header>
    <div class="grid grid-cols-4 gap-2 px-4 pb-[calc(1.5rem+env(safe-area-inset-bottom))]">
      {#each more as r (r.path)}
        <a
          href="#{r.path}"
          onclick={() => (moreOpen = false)}
          class={cn('hover:bg-muted flex flex-col items-center gap-1.5 rounded-xl py-3 text-xs', r === current && 'bg-muted text-primary font-medium')}
        >
          <r.icon class="size-6" />
          {r.label}
        </a>
      {/each}
    </div>
  </Drawer.Content>
</Drawer.Root>
{/if}
