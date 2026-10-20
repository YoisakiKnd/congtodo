<script lang="ts">
  import { createCalendar, type CalendarApp, type CalendarConfig, type CalendarEvent } from '@schedule-x/calendar';
  import { createCurrentTimePlugin } from '@schedule-x/current-time';
  import { mode } from 'mode-watcher';
  import { onMount } from 'svelte';
  import { CALENDARS, toRange, TZ, type Range } from '$lib/schedule';

  // build 读取的响应式数据变化时，自动重新计算并刷新日历事件
  let {
    config,
    build,
    onrange,
  }: {
    config: CalendarConfig;
    build: (r: Range) => CalendarEvent[];
    onrange?: (r: Range) => void;
  } = $props();

  let el: HTMLDivElement;
  let app: CalendarApp | null = null;
  let range = $state<Range | null>(null);

  function setRange(r: Range) {
    range = r;
    onrange?.(r);
  }

  onMount(() => {
    const cal = createCalendar(
      {
        locale: 'zh-CN',
        firstDayOfWeek: 1,
        timezone: TZ as CalendarConfig['timezone'],
        theme: 'shadcn',
        isDark: mode.current === 'dark',
        calendars: CALENDARS,
        ...config,
        callbacks: {
          ...config.callbacks,
          // 首次渲染和每次切换范围都会调用
          fetchEvents: async (r) => {
            setRange(toRange(r));
            return build(toRange(r));
          },
        },
      },
      [createCurrentTimePlugin()],
    );
    cal.render(el);
    app = cal;
    return () => cal.destroy();
  });

  $effect(() => {
    if (range) app?.events.set(build(range));
  });

  $effect(() => {
    app?.setTheme(mode.current === 'dark' ? 'dark' : 'light');
  });
</script>

<div class="sx-host" bind:this={el}></div>
