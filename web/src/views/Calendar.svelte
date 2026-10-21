<script lang="ts">
  // 统一日历：日程、课程、待办、倒数日、订阅日历作为图层叠加；课表的导入 / 作息 / 添加都在「课表」菜单里
  import { BookOpen, CalendarPlus, ChevronDown, Clock, FileUp, LayoutGrid, Plus } from '@lucide/svelte';
  import {
    createViewDay,
    createViewMonthAgenda,
    createViewMonthGrid,
    createViewWeek,
    type CalendarConfig,
  } from '@schedule-x/calendar';
  import { createCalendarControlsPlugin } from '@schedule-x/calendar-controls';
  import { createScrollControllerPlugin } from '@schedule-x/scroll-controller';
  import { toast } from 'svelte-sonner';
  import Page from '../components/Page.svelte';
  import ScheduleX from '../components/ScheduleX.svelte';
  import { buttonVariants } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { api } from '$lib/api';
  import { createCellRenderer } from '$lib/calendar-cells';
  import {
    countdownInstances,
    courseInstances,
    eventInstances,
    feedInstances,
    todoInstances,
    toDate,
    type EventRef,
    type Range,
  } from '$lib/schedule';
  import { db, ensureHolidays, holidays, settings } from '$lib/store.svelte';
  import { isoWeekday, termWeek, toTs, ymd } from '$lib/time';
  import type { FeedEvent } from '$lib/types';
  import { openCourse, openEvent, openImport, openPanel, openPeriods } from '$lib/ui.svelte';
  import { cn } from '$lib/utils';

  // ---------- 图层（记在本地） ----------
  type Layer = 'events' | 'courses' | 'todos' | 'countdowns' | 'feeds';
  const LAYERS: { key: Layer; label: string; color: string }[] = [
    { key: 'events', label: '日程', color: '#5b5bd6' },
    { key: 'courses', label: '课程', color: '#12a594' },
    { key: 'todos', label: '待办', color: '#f5a524' },
    { key: 'countdowns', label: '倒数日', color: '#e5484d' },
    { key: 'feeds', label: '订阅', color: '#0091ff' },
  ];
  const LAYER_KEY = 'congtodo.calendar.layers';
  function loadLayers(): Record<Layer, boolean> {
    const all = { events: true, courses: true, todos: true, countdowns: true, feeds: true };
    try {
      return { ...all, ...JSON.parse(localStorage.getItem(LAYER_KEY) ?? '{}') };
    } catch {
      return all;
    }
  }
  let layers = $state(loadLayers());
  $effect(() => {
    try {
      localStorage.setItem(LAYER_KEY, JSON.stringify(layers));
    } catch {}
  });
  const visibleLayers = $derived(LAYERS.filter((l) => l.key !== 'feeds' || db.feeds.items.length));

  // ---------- 数据 ----------
  let range = $state<Range | null>(null);
  let feedEvents = $state<FeedEvent[]>([]);
  // 月视图（范围超过 8 天）：周次按今天算；课程太密，只在周 / 日视图显示
  const monthView = $derived(!!range && range.end.getTime() - range.start.getTime() > 8 * 86400_000);
  const week = $derived(range ? termWeek(settings.term_start, monthView ? new Date() : range.start) : null);
  const skipHoliday = $derived(settings.course_skip_holiday !== '0');

  async function onrange(r: Range) {
    range = r;
    for (let y = r.start.getFullYear(); y <= r.end.getFullYear(); y++) ensureHolidays(y);
    if (!db.feeds.items.some((f) => f.enabled)) return (feedEvents = []);
    feedEvents = await api.get<FeedEvent[]>(`/feeds/events?from=${toTs(r.start)}&to=${toTs(r.end)}`).catch(() => []);
  }

  const build = (r: Range) => [
    ...(layers.feeds ? feedInstances(feedEvents) : []),
    ...(layers.events ? eventInstances(db.events.items, r) : []),
    ...(layers.courses && !monthView
      ? courseInstances(db.courses.items, r, settings.term_start, false, (d) => skipHoliday && !!holidays.get(ymd(d))?.off)
      : []),
    ...(layers.todos ? todoInstances(db.todos.items, r) : []),
    ...(layers.countdowns ? countdownInstances(db.countdowns.items, r) : []),
  ];

  // 农历 / 节假日格子：节假日数据加载后重绘
  const cells = createCellRenderer();
  $effect(() => {
    void holidays.size;
    cells.repaint();
  });

  // ---------- Schedule-X ----------
  const controls = createCalendarControlsPlugin();
  const plugins = [controls, createScrollControllerPlugin({ initialScroll: '07:30' })];

  const config: CalendarConfig = {
    views: [createViewMonthGrid(), createViewWeek(), createViewDay(), createViewMonthAgenda()],
    defaultView: 'month-grid',
    weekOptions: { gridHeight: 1300, eventOverlap: false },
    monthGridOptions: { nEventsPerDay: 4 },
    callbacks: {
      onEventClick(ev) {
        const ref = ev.ref as EventRef;
        if (ref.kind === 'event') {
          const e = db.events.items.find((x) => x.id === ref.id);
          if (e) openEvent({ event: e });
        } else if (ref.kind === 'course') {
          const c = db.courses.items.find((x) => x.id === ref.id);
          if (c) openCourse({ course: c });
        } else if (ref.kind === 'countdown') {
          openPanel('countdowns');
        } else if (ref.kind === 'todo') {
          location.hash = '#/todos';
        } else {
          toast.info(ev.title ?? '', { description: `${ev.description ?? ''}（订阅日历只读）` });
        }
      },
      onDoubleClickDate: (d) => openEvent({ start: toDate(d), allDay: true }),
      onDoubleClickDateTime: (d) => openEvent({ start: toDate(d) }),
    },
  };

  /** 切到本周的周视图，只看课程 */
  function showTimetable() {
    layers = { events: false, courses: true, todos: false, countdowns: false, feeds: false };
    controls.setDate(Temporal.PlainDate.from(ymd(new Date())));
    // 滚动控制插件会在切到周视图时自动滚到 07:30
    controls.setView('week');
  }

</script>

<Page title="日历" sub={week != null && week > 0 ? `第 ${week} 周` : undefined} class="max-w-7xl">
  {#snippet actions()}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger class={buttonVariants({ variant: 'outline' })}><BookOpen />课表<ChevronDown class="opacity-60" /></DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-44">
        <DropdownMenu.Item onSelect={showTimetable}><LayoutGrid />查看本周课表</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={() => openCourse({ weekday: Math.min(isoWeekday(new Date()), 7) })}><Plus />添加课程</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={openImport}><FileUp />导入课表</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={openPeriods}><Clock />学期与作息</DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
    <DropdownMenu.Root>
      <DropdownMenu.Trigger class={buttonVariants()}><Plus />新建</DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-36">
        <DropdownMenu.Item onSelect={() => openEvent()}><CalendarPlus />日程</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={() => openCourse({ weekday: isoWeekday(new Date()) })}><BookOpen />课程</DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  {/snippet}

  <div class="mb-3 flex flex-wrap items-center gap-1.5">
    {#each visibleLayers as l (l.key)}
      <button
        class={cn(
          'flex h-8 items-center gap-1.5 rounded-full border px-3 text-sm transition-colors',
          layers[l.key] ? 'bg-card text-foreground shadow-xs' : 'text-muted-foreground border-dashed',
        )}
        aria-pressed={layers[l.key]}
        onclick={() => (layers[l.key] = !layers[l.key])}
      >
        <span class={cn('size-2 rounded-full', !layers[l.key] && 'opacity-30')} style="background: {l.color}"></span>
        {l.label}
      </button>
    {/each}
    {#if !db.courses.items.length && layers.courses}
      <button class="text-primary ml-1 text-sm hover:underline" onclick={openImport}>导入课表，上课前自动提醒 →</button>
    {/if}
  </div>

  <Card.Root class="p-0">
    <ScheduleX {config} {build} {onrange} {plugins} custom={cells.custom} />
  </Card.Root>
  <p class="text-muted-foreground mt-3 text-xs">
    双击日期或时间格新建日程 · 课程在周 / 日视图显示，点击可编辑{skipHoliday ? ' · 法定假日的课程淡化显示且不提醒' : ''}
  </p>
</Page>
