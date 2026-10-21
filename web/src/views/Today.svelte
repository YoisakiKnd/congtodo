<script lang="ts">
  import {
    ArrowRight,
    BookOpen,
    CalendarDays,
    CalendarPlus,
    CheckSquare,
    Flame,
    Hourglass,
    MapPin,
    Plus,
    Timer,
  } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import Page from '../components/Page.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { Input } from '$lib/components/ui/input';
  import { api } from '$lib/api';
  import { startFocus, timer } from '$lib/focus.svelte';
  import { lunar } from '$lib/lunar';
  import { clock, db, habitCount, habitDone, holidays, settings, toggleHabit } from '$lib/store.svelte';
  import {
    addDays,
    countdownTarget,
    dayDiff,
    fmtDate,
    fmtRelative,
    fromTs,
    hhmm,
    inWeeks,
    isoWeekday,
    occursOn,
    startOfDay,
    termWeek,
    toMinutes,
    toTs,
    WEEKDAYS,
    ymd,
  } from '$lib/time';
  import type { CalEvent, Course, FeedEvent, Todo } from '$lib/types';
  import { openCourse, openEvent, openPanel } from '$lib/ui.svelte';
  import { buttonVariants } from '$lib/components/ui/button';
  import { cn } from '$lib/utils';

  const now = $derived(new Date(clock.minute));
  const today = $derived(startOfDay(now));
  const todayStr = $derived(ymd(today));
  const tomorrow = $derived(addDays(today, 1));
  const week = $derived(termWeek(settings.term_start, today));
  const holiday = $derived(holidays.get(todayStr));
  const nowMin = $derived(now.getHours() * 60 + now.getMinutes());

  const greeting = $derived.by(() => {
    const h = now.getHours();
    return h < 5 ? '夜深了' : h < 11 ? '早上好' : h < 13 ? '中午好' : h < 18 ? '下午好' : '晚上好';
  });
  const subtitle = $derived.by(() => {
    const l = lunar(today);
    return [
      fmtDate(now),
      `农历${l.month}${l.dayName}`,
      holiday ? (holiday.off ? `${holiday.name}假期` : `${holiday.name}调休上班`) : '',
      week ? `第 ${week} 周` : '',
    ]
      .filter(Boolean)
      .join(' · ');
  });

  // ---------- 订阅日历：今天和接下来一周 ----------
  let feedEvents = $state<FeedEvent[]>([]);
  onMount(() => {
    if (!db.feeds.items.some((f) => f.enabled)) return;
    api
      .get<FeedEvent[]>(`/feeds/events?from=${toTs(today)}&to=${toTs(addDays(today, 8))}`)
      .then((r) => (feedEvents = r))
      .catch(() => {});
  });

  // ---------- 时间线：课程 + 日程 + 订阅 ----------
  type Item = {
    key: string;
    title: string;
    sub?: string;
    color: string;
    start: number | null; // 分钟；null = 全天
    end: number | null;
    kind: 'course' | 'event' | 'feed';
    open?: () => void;
  };

  const skipHoliday = $derived(settings.course_skip_holiday !== '0');

  function coursesOn(d: Date): Course[] {
    const w = termWeek(settings.term_start, d);
    if (skipHoliday && holidays.get(ymd(d))?.off) return [];
    return db.courses.items.filter((c) => c.weekday === isoWeekday(d) && (w == null || inWeeks(c.weeks, w)));
  }
  const minutesOf = (ts: number) => {
    const d = fromTs(ts);
    return d.getHours() * 60 + d.getMinutes();
  };
  const sameDay = (ts: number, d: Date) => ymd(fromTs(ts)) === ymd(d);

  function itemsOn(d: Date): Item[] {
    const items: Item[] = [];
    for (const c of coursesOn(d)) {
      items.push({
        key: `c${c.id}`,
        title: c.name,
        sub: [c.room, c.teacher].filter(Boolean).join(' · '),
        color: c.color ?? 'var(--primary)',
        start: toMinutes(c.start_time),
        end: toMinutes(c.end_time),
        kind: 'course',
        open: () => openCourse({ course: c }),
      });
    }
    for (const e of db.events.items.filter((e) => occursOn(e, d))) {
      const timed = !e.all_day && (e.repeat !== 'none' || sameDay(e.start_at, d));
      items.push({
        key: `e${e.id}`,
        title: e.title,
        sub: e.note ?? undefined,
        color: e.color ?? 'var(--primary)',
        start: timed ? minutesOf(e.start_at) : null,
        end: timed && e.end_at ? minutesOf(e.end_at) : null,
        kind: 'event',
        open: () => openEvent({ event: e as CalEvent }),
      });
    }
    for (const f of feedEvents.filter((f) => (f.all_day ? f.start_at <= toTs(d) && (f.end_at ?? f.start_at) >= toTs(d) : sameDay(f.start_at, d)))) {
      items.push({
        key: `f${f.feed_id}-${f.uid}-${f.start_at}`,
        title: f.title,
        sub: [f.location, f.feed].filter(Boolean).join(' · '),
        color: f.color ?? 'var(--muted-foreground)',
        start: f.all_day ? null : minutesOf(f.start_at),
        end: f.all_day || !f.end_at ? null : minutesOf(f.end_at),
        kind: 'feed',
      });
    }
    return items.toSorted((a, b) => (a.start ?? -1) - (b.start ?? -1));
  }

  const agenda = $derived(itemsOn(today));
  const allDay = $derived(agenda.filter((i) => i.start == null));
  const timed = $derived(agenda.filter((i) => i.start != null));
  /** 「现在」分隔线插在第一个还没开始的事项前面 */
  const nowIndex = $derived(timed.findIndex((i) => i.start! > nowMin));
  const stateOf = (i: Item) =>
    nowMin >= (i.end ?? i.start! + 60) ? 'past' : nowMin >= i.start! ? 'now' : 'next';

  // ---------- 今天的待办：逾期 + 今天到期 ----------
  const endOfToday = $derived(toTs(tomorrow));
  const todos = $derived(
    db.todos.items
      .filter((t) => !t.done && !t.parent_id && t.due_at != null && t.due_at < endOfToday)
      .toSorted((a, b) => a.due_at! - b.due_at! || b.priority - a.priority),
  );
  const doneToday = $derived(db.todos.items.filter((t) => t.done && t.done_at && t.done_at >= toTs(today) && !t.parent_id).length);
  let quick = $state('');

  async function addTodo(e: SubmitEvent) {
    e.preventDefault();
    const title = quick.trim();
    if (!title) return;
    quick = '';
    await db.todos.add({ title, due_at: endOfToday - 60 });
  }

  async function complete(t: Todo) {
    const row = await db.todos.patch(t.id, { done: 1, done_at: toTs(new Date()) });
    if (row._next) {
      toast.success(`已完成，下次：${fmtRelative(row.due_at!, new Date())}`);
      await db.todos.load();
    }
  }

  // ---------- 接下来 7 天 ----------
  type Upcoming = { day: Date; items: { key: string; time: string; title: string; color: string }[]; courses: number };
  const upcoming = $derived.by(() => {
    const out: Upcoming[] = [];
    for (let i = 1; i <= 7; i++) {
      const d = addDays(today, i);
      const items = itemsOn(d)
        .filter((x) => x.kind !== 'course')
        .map((x) => ({
          key: x.key,
          time: x.start == null ? '全天' : `${String(Math.floor(x.start / 60)).padStart(2, '0')}:${String(x.start % 60).padStart(2, '0')}`,
          title: x.title,
          color: x.color,
        }));
      for (const t of db.todos.items.filter((t) => !t.done && !t.parent_id && t.due_at && sameDay(t.due_at, d))) {
        items.push({ key: `t${t.id}`, time: hhmm(fromTs(t.due_at!)), title: `☐ ${t.title}`, color: '#f5a524' });
      }
      const courses = coursesOn(d).length;
      if (items.length || courses) out.push({ day: d, items: items.slice(0, 4), courses });
    }
    return out.slice(0, 5);
  });

  // ---------- 侧栏：倒数日 / 习惯 / 专注 ----------
  const countdowns = $derived(
    db.countdowns.items
      .map((c) => ({ ...c, next: countdownTarget(c, now) }))
      .filter((c) => dayDiff(now, fromTs(c.next.ts)) >= 0)
      .toSorted((a, b) => b.pinned - a.pinned || a.next.ts - b.next.ts)
      .slice(0, 3),
  );
  const habits = $derived(
    db.habits.items.filter((h) => !h.archived && (!h.days || h.days.split(',').map(Number).includes(isoWeekday(today)))),
  );
  const focusToday = $derived(db.focus.items.filter((f) => f.started_at >= toTs(today)).reduce((s, f) => s + f.minutes, 0));

  const fmtMin = (m: number) => `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`;
</script>

{#snippet head(Icon: typeof Flame, title: string, action?: { label: string; onclick: () => void })}
  <Card.Header>
    <Card.Title class="flex items-center gap-2 text-sm"><Icon class="text-muted-foreground size-4" />{title}</Card.Title>
    {#if action}
      <Card.Action>
        <button class="text-muted-foreground hover:text-primary flex items-center gap-1 text-xs" onclick={action.onclick}>
          {action.label}<ArrowRight class="size-3" />
        </button>
      </Card.Action>
    {/if}
  </Card.Header>
{/snippet}

<Page title={greeting} sub={subtitle}>
  {#snippet actions()}
    <DropdownMenu.Root>
      <DropdownMenu.Trigger class={buttonVariants()}><Plus />新建</DropdownMenu.Trigger>
      <DropdownMenu.Content align="end" class="w-40">
        <DropdownMenu.Item onSelect={() => document.getElementById('today-quick')?.focus()}><CheckSquare />待办</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={() => openEvent()}><CalendarPlus />日程</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={() => openCourse({ weekday: isoWeekday(today) })}><BookOpen />课程</DropdownMenu.Item>
        <DropdownMenu.Separator />
        <DropdownMenu.Item onSelect={() => openPanel('countdowns')}><Hourglass />倒数日</DropdownMenu.Item>
        <DropdownMenu.Item onSelect={() => openPanel('habits')}><Flame />习惯</DropdownMenu.Item>
      </DropdownMenu.Content>
    </DropdownMenu.Root>
  {/snippet}

  <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_20rem]">
    <div class="grid content-start gap-4">
      <!-- 时间线 -->
      <Card.Root>
        {@render head(CalendarDays, '今天的安排', { label: '日历', onclick: () => (location.hash = '#/calendar') })}
        <Card.Content class="grid gap-1">
          {#each allDay as i (i.key)}
            <button
              class="flex items-center gap-3 rounded-lg px-3 py-2 text-left text-sm"
              style="background: color-mix(in oklch, {i.color} 12%, var(--card))"
              onclick={i.open}
              disabled={!i.open}
            >
              <span class="text-muted-foreground w-24 shrink-0 text-xs">全天</span>
              <span class="font-medium">{i.title}</span>
            </button>
          {/each}
          {#each timed as i, idx (i.key)}
            {#if idx === nowIndex}
              <div class="text-destructive flex items-center gap-2 py-1 text-xs font-medium">
                <span class="num w-24 shrink-0 pl-3">{fmtMin(nowMin)}</span>
                <span class="bg-destructive/60 h-px flex-1"></span>
              </div>
            {/if}
            {@const st = stateOf(i)}
            <button
              class={cn(
                'hover:bg-muted/60 flex items-center gap-3 rounded-lg px-3 py-2.5 text-left transition-colors disabled:cursor-default',
                st === 'past' && 'opacity-50',
                st === 'now' && 'bg-primary/5 ring-primary/30 ring-1',
              )}
              onclick={i.open}
              disabled={!i.open}
            >
              <span class="num text-muted-foreground w-24 shrink-0 text-xs">
                {fmtMin(i.start!)}{i.end != null ? ` – ${fmtMin(i.end)}` : ''}
              </span>
              <span class="h-8 w-1 shrink-0 rounded-full" style="background: {i.color}"></span>
              <span class="min-w-0 flex-1">
                <span class="flex items-center gap-1.5 font-medium">
                  {#if i.kind === 'course'}<BookOpen class="text-muted-foreground size-3.5" />{/if}{i.title}
                  {#if st === 'now'}<span class="text-primary text-xs font-normal">进行中</span>{/if}
                </span>
                {#if i.sub}<span class="text-muted-foreground flex items-center gap-1 truncate text-xs">{#if i.kind === 'course'}<MapPin class="size-3" />{/if}{i.sub}</span>{/if}
              </span>
            </button>
          {/each}
          {#if !agenda.length}
            <p class="text-muted-foreground py-6 text-center text-sm">
              {holiday?.off ? `${holiday.name}假期，没有安排 🎉` : '今天没有课程和日程'}
            </p>
          {/if}
        </Card.Content>
      </Card.Root>

      <!-- 待办 -->
      <Card.Root>
        {@render head(CheckSquare, `今天的待办${doneToday ? ` · 已完成 ${doneToday}` : ''}`, {
          label: '全部',
          onclick: () => (location.hash = '#/todos'),
        })}
        <Card.Content class="grid gap-1">
          <form class="relative mb-1" onsubmit={addTodo}>
            <Plus class="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
            <Input id="today-quick" class="pl-9" placeholder="添加今天要做的事，回车保存" bind:value={quick} />
          </form>
          {#each todos as t (t.id)}
            {@const overdue = t.due_at! < toTs(today)}
            <div class="group hover:bg-muted/60 flex items-center gap-3 rounded-lg px-3 py-2">
              <Checkbox onCheckedChange={() => complete(t)} aria-label="完成" />
              <span class="min-w-0 flex-1 truncate">{t.title}</span>
              <button
                class={cn(
                  'text-muted-foreground hover:text-primary rounded p-1 opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100 max-md:opacity-100',
                  timer.todoId === t.id && 'text-primary opacity-100',
                )}
                title="开始专注"
                aria-label="开始专注"
                onclick={() => startFocus(t.id)}
              >
                <Timer class="size-4" />
              </button>
              <span class={cn('num shrink-0 text-xs', overdue ? 'text-destructive font-medium' : 'text-muted-foreground')}>
                {overdue ? `逾期 · ${fmtRelative(t.due_at!, now, false)}` : hhmm(fromTs(t.due_at!))}
              </span>
            </div>
          {:else}
            <p class="text-muted-foreground py-4 text-center text-sm">今天的事都做完了 ✨</p>
          {/each}
        </Card.Content>
      </Card.Root>
    </div>

    <!-- 侧栏 -->
    <div class="grid content-start gap-4">
      {#if countdowns.length}
        <Card.Root size="sm">
          {@render head(Hourglass, '倒数日', { label: '全部', onclick: () => openPanel('countdowns') })}
          <Card.Content class="grid gap-2">
            {#each countdowns as c (c.id)}
              {@const days = dayDiff(now, fromTs(c.next.ts))}
              <button
                class="flex items-center gap-3 rounded-lg px-3 py-2 text-left"
                style="background: color-mix(in oklch, {c.color ?? 'var(--primary)'} 10%, var(--card))"
                onclick={() => openPanel('countdowns')}
              >
                <span class="min-w-0 flex-1">
                  <span class="block truncate text-sm font-medium">{c.title}</span>
                  <span class="text-muted-foreground text-xs">{fmtRelative(c.next.ts, now, false)}{c.next.years ? ` · ${c.next.years} 周年` : ''}</span>
                </span>
                <span class="num text-2xl font-bold" style="color: {c.color ?? 'var(--primary)'}">
                  {days === 0 ? '今天' : days}<span class="ml-0.5 text-xs font-medium">{days ? '天' : ''}</span>
                </span>
              </button>
            {/each}
          </Card.Content>
        </Card.Root>
      {/if}

      <Card.Root size="sm">
        {@render head(Flame, '习惯打卡', { label: '管理', onclick: () => openPanel('habits') })}
        <Card.Content class="flex-row flex-wrap gap-2">
          {#each habits as h (h.id)}
            {@const on = habitDone(h, todayStr)}
            <button
              class={cn(
                'flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm transition-all active:scale-95',
                on ? 'text-white' : 'bg-muted hover:bg-accent',
              )}
              style={on ? `background: ${h.color}` : ''}
              onclick={() => toggleHabit(h, todayStr)}
            >
              <span>{h.icon || '✨'}</span>{h.name}
              {#if (h.goal || 1) > 1}<span class="num text-xs opacity-80">{habitCount(h.id, todayStr)}/{h.goal}</span>{/if}
            </button>
          {:else}
            <button class="text-muted-foreground hover:text-foreground text-sm" onclick={() => openPanel('habits')}>
              + 添加一个想坚持的习惯
            </button>
          {/each}
        </Card.Content>
      </Card.Root>

      <Card.Root size="sm">
        {@render head(Timer, '专注', { label: '统计', onclick: () => openPanel('focus') })}
        <Card.Content class="flex-row items-center justify-between gap-2">
          <span class="text-muted-foreground text-sm">今天 <b class="num text-foreground text-lg">{focusToday}</b> 分钟</span>
          <Button size="sm" variant="outline" onclick={() => (startFocus(null), openPanel('focus'))}><Timer />开始专注</Button>
        </Card.Content>
      </Card.Root>

      {#if upcoming.length}
        <Card.Root size="sm">
          {@render head(CalendarDays, '接下来')}
          <Card.Content class="grid gap-3">
            {#each upcoming as u (ymd(u.day))}
              <div class="grid gap-1">
                <span class="text-muted-foreground text-xs font-medium">
                  {dayDiff(today, u.day) === 1 ? '明天' : `${u.day.getMonth() + 1}月${u.day.getDate()}日`} · 周{WEEKDAYS[isoWeekday(u.day) - 1]}
                  {#if u.courses}<span class="ml-1">· {u.courses} 节课</span>{/if}
                </span>
                {#each u.items as x (x.key)}
                  <div class="flex items-center gap-2 text-sm">
                    <span class="size-1.5 shrink-0 rounded-full" style="background: {x.color}"></span>
                    <span class="num text-muted-foreground w-10 shrink-0 text-xs">{x.time}</span>
                    <span class="truncate">{x.title}</span>
                  </div>
                {/each}
              </div>
            {/each}
          </Card.Content>
        </Card.Root>
      {/if}
    </div>
  </div>
</Page>
