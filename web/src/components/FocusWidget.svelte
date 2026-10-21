<script lang="ts">
  // 专注悬浮小窗：计时进行中时显示在右下角，到点自动结束；点击打开专注面板
  import { Pause, Play, Square } from '@lucide/svelte';
  import { finishFocus, focus, FOCUS_LABEL, pauseFocus, startFocus, stopFocus, timer } from '$lib/focus.svelte';
  import { db } from '$lib/store.svelte';
  import { pad } from '$lib/time';
  import { openPanel } from '$lib/ui.svelte';
  import { cn } from '$lib/utils';

  // 到点自动结束（这个组件始终挂载，所以不打开面板也会触发）
  $effect(() => {
    if (focus.running && focus.left === 0) finishFocus();
  });

  // 标签页标题显示倒计时
  const originalTitle = document.title;
  $effect(() => {
    document.title = focus.running
      ? `${pad(Math.floor(focus.left / 60))}:${pad(focus.left % 60)} ${FOCUS_LABEL[timer.mode]} · Congtodo`
      : originalTitle;
  });

  const todo = $derived(timer.todoId ? db.todos.items.find((t) => t.id === timer.todoId) : undefined);
  const progress = $derived(1 - focus.left / focus.total);
</script>

{#if focus.active}
  <div
    class="bg-popover text-popover-foreground fixed right-4 bottom-20 z-30 flex items-center gap-1 overflow-hidden rounded-full py-1 pr-1 pl-1 shadow-lg ring-1 ring-foreground/10 md:bottom-6"
  >
    <button class="flex min-w-0 items-center gap-2.5 rounded-full py-1 pr-2 pl-1" onclick={() => openPanel('focus')}>
      <span
        class="grid size-8 shrink-0 place-items-center rounded-full"
        style="background: conic-gradient({timer.mode === 'focus' ? 'var(--primary)' : '#30a46c'} {progress * 360}deg, var(--muted) 0)"
      >
        <span class="bg-popover size-6 rounded-full"></span>
      </span>
      <span class="grid text-left leading-tight">
        <span class="num text-sm font-semibold">{pad(Math.floor(focus.left / 60))}:{pad(focus.left % 60)}</span>
        <span class="text-muted-foreground max-w-36 truncate text-xs">{todo?.title ?? FOCUS_LABEL[timer.mode]}</span>
      </span>
    </button>
    <button
      class={cn('hover:bg-muted grid size-8 place-items-center rounded-full')}
      aria-label={focus.running ? '暂停' : '继续'}
      onclick={() => (focus.running ? pauseFocus() : startFocus())}
    >
      {#if focus.running}<Pause class="size-4" />{:else}<Play class="size-4" />{/if}
    </button>
    <button class="hover:bg-muted grid size-8 place-items-center rounded-full" aria-label="结束" onclick={() => (timer.mode === 'focus' ? finishFocus(true) : stopFocus())}>
      <Square class="size-3.5" />
    </button>
  </div>
{/if}
