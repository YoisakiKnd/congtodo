<script lang="ts">
  import { Plus } from '@lucide/svelte';
  import {
    createViewDay,
    createViewMonthAgenda,
    createViewMonthGrid,
    createViewWeek,
    type CalendarConfig,
  } from '@schedule-x/calendar';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import ScheduleX from '../components/ScheduleX.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { Textarea } from '$lib/components/ui/textarea';
  import { api } from '$lib/api';
  import { createCellRenderer } from '$lib/calendar-cells';
  import {
    courseInstances,
    eventInstances,
    feedInstances,
    todoInstances,
    toDate,
    type EventRef,
    type Range,
  } from '$lib/schedule';
  import ChannelPick from '../components/ChannelPick.svelte';
  import { db, ensureHolidays, holidays, PALETTE, settings } from '$lib/store.svelte';
  import { fromInput, fromTs, hhmm, parseYmd, REMIND_OPTIONS, toInput, toTs, ymd } from '$lib/time';
  import type { CalEvent, ChannelIds, FeedEvent, Repeat } from '$lib/types';
  import { toast } from 'svelte-sonner';

  let showCourses = $state(false);
  let showTodos = $state(true);
  let showFeeds = $state(true);
  let feedEvents = $state<FeedEvent[]>([]);

  // 农历 / 节假日格子：节假日数据加载后重绘
  const cells = createCellRenderer();
  $effect(() => {
    void holidays.size;
    cells.repaint();
  });

  async function onrange(r: Range) {
    for (let y = r.start.getFullYear(); y <= r.end.getFullYear(); y++) ensureHolidays(y);
    if (!db.feeds.items.some((f) => f.enabled)) return (feedEvents = []);
    feedEvents = await api
      .get<FeedEvent[]>(`/feeds/events?from=${toTs(r.start)}&to=${toTs(r.end)}`)
      .catch(() => []);
  }

  const build = (r: Range) => [
    ...(showFeeds ? feedInstances(feedEvents) : []),
    ...eventInstances(db.events.items, r),
    ...(showTodos ? todoInstances(db.todos.items, r) : []),
    ...(showCourses ? courseInstances(db.courses.items, r, settings.term_start, false) : []),
  ];

  const config: CalendarConfig = {
    views: [createViewMonthGrid(), createViewWeek(), createViewDay(), createViewMonthAgenda()],
    defaultView: 'month-grid',
    weekOptions: { gridHeight: 1100 },
    monthGridOptions: { nEventsPerDay: 4 },
    callbacks: {
      onEventClick(ev) {
        const ref = ev.ref as EventRef;
        if (ref.kind === 'event') {
          const e = db.events.items.find((x) => x.id === ref.id);
          if (e) edit(e);
        } else if (ref.kind === 'feed') {
          toast.info(ev.title ?? '', { description: `${ev.description ?? ''}（订阅日历只读）` });
        } else location.hash = ref.kind === 'todo' ? '#/todos' : '#/timetable';
      },
      onDoubleClickDate: (d) => create(toDate(d), true),
      onDoubleClickDateTime: (d) => create(toDate(d), false),
    },
  };

  // ---------- 日程表单 ----------

  const REPEATS: { value: Repeat; label: string }[] = [
    { value: 'none', label: '不重复' },
    { value: 'daily', label: '每天' },
    { value: 'weekly', label: '每周' },
    { value: 'monthly', label: '每月' },
    { value: 'yearly', label: '每年' },
  ];

  let sheet = $state(false);
  let form = $state({
    id: 0,
    title: '',
    all_day: false,
    start: '',
    end: '',
    repeat: 'none' as Repeat,
    remind_before: 15 as number | null,
    color: PALETTE[0] as string | null,
    note: '',
    channels: null as ChannelIds,
  });

  function create(at: Date, dateOnly: boolean) {
    const start = dateOnly ? new Date(at.getFullYear(), at.getMonth(), at.getDate(), 9) : at;
    const end = new Date(start.getTime() + 3600_000);
    form = {
      id: 0,
      title: '',
      all_day: false,
      start: `${ymd(start)}T${hhmm(start)}`,
      end: `${ymd(end)}T${hhmm(end)}`,
      repeat: 'none',
      remind_before: 15,
      color: PALETTE[0],
      note: '',
      channels: null,
    };
    sheet = true;
  }

  function edit(e: CalEvent) {
    const allDay = !!e.all_day;
    const conv = (ts: number | null) => (allDay ? (ts ? ymd(fromTs(ts)) : '') : toInput(ts));
    form = {
      id: e.id,
      title: e.title,
      all_day: allDay,
      start: conv(e.start_at),
      end: conv(e.end_at),
      repeat: e.repeat,
      remind_before: e.remind_before,
      color: e.color,
      note: e.note ?? '',
      channels: e.channels,
    };
    sheet = true;
  }

  function setAllDay(v: boolean) {
    form.all_day = v;
    const conv = (s: string) => (!s ? '' : v ? s.slice(0, 10) : s.length === 10 ? `${s}T09:00` : s);
    form.start = conv(form.start);
    form.end = conv(form.end);
  }

  function save() {
    const ts = (s: string) => (!s ? null : form.all_day ? toTs(parseYmd(s)) : fromInput(s));
    return db.events.save({
      id: form.id || undefined,
      title: form.title.trim(),
      all_day: +form.all_day,
      start_at: ts(form.start)!,
      end_at: ts(form.end),
      repeat: form.repeat,
      remind_before: form.remind_before,
      color: form.color,
      note: form.note || null,
      channels: form.channels,
    });
  }
</script>

<Page title="日历" class="max-w-7xl">
  {#snippet actions()}
    <label class="flex items-center gap-2 text-sm"><Switch bind:checked={showTodos} />待办</label>
    <label class="flex items-center gap-2 text-sm"><Switch bind:checked={showCourses} />课程</label>
    {#if db.feeds.items.length}
      <label class="flex items-center gap-2 text-sm"><Switch bind:checked={showFeeds} />订阅</label>
    {/if}
    <Button onclick={() => create(new Date(), true)}><Plus />日程</Button>
  {/snippet}

  <Card.Root class="p-0">
    <ScheduleX {config} {build} {onrange} custom={cells.custom} />
  </Card.Root>
  <p class="text-muted-foreground mt-3 text-xs">双击日期或时间格可快速新建日程</p>
</Page>

<Sheet bind:open={sheet} title={form.id ? '编辑日程' : '新建日程'} onsave={save} ondelete={form.id ? () => db.events.remove(form.id) : undefined}>
  <Field label="标题"><Input required bind:value={form.title} /></Field>
  <label class="flex items-center gap-3 text-sm">
    <Switch checked={form.all_day} onCheckedChange={setAllDay} />全天
  </label>
  <div class="grid grid-cols-2 gap-3">
    {#if form.all_day}
      <Field label="开始"><Input required type="date" bind:value={form.start} /></Field>
      <Field label="结束"><Input type="date" bind:value={form.end} /></Field>
    {:else}
      <Field label="开始"><Input required type="datetime-local" bind:value={form.start} /></Field>
      <Field label="结束"><Input type="datetime-local" bind:value={form.end} /></Field>
    {/if}
  </div>
  <div class="grid grid-cols-2 gap-3">
    <Field label="重复"><Choice bind:value={form.repeat} options={REPEATS} /></Field>
    <Field label="提醒"><Choice bind:value={form.remind_before} options={REMIND_OPTIONS} /></Field>
  </div>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <ChannelPick bind:value={form.channels} />
  <Field label="备注"><Textarea bind:value={form.note} /></Field>
</Sheet>
