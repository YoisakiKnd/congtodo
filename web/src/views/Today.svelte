<script lang="ts">
  import { ArrowRight, CalendarDays, CheckSquare, Flame, LayoutGrid } from '@lucide/svelte';
  import Page from '../components/Page.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { clock, db, habitLogs, settings, toggleHabit } from '$lib/store.svelte';
  import {
    addDays,
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
    ymd,
  } from '$lib/time';
  import { cn } from '$lib/utils';

  const now = $derived(new Date(clock.minute));
  const today = $derived(startOfDay(now));
  const todayStr = $derived(ymd(today));
  const week = $derived(termWeek(settings.term_start, today));
  const nowMin = $derived(now.getHours() * 60 + now.getMinutes());

  const greeting = $derived.by(() => {
    const h = now.getHours();
    return h < 5 ? '夜深了' : h < 11 ? '早上好' : h < 13 ? '中午好' : h < 18 ? '下午好' : '晚上好';
  });

  const courses = $derived(
    db.courses.items
      .filter((c) => c.weekday === isoWeekday(today) && (week == null || inWeeks(c.weeks, week)))
      .toSorted((a, b) => a.start_time.localeCompare(b.start_time)),
  );
  const events = $derived(db.events.items.filter((e) => occursOn(e, today)));
  const tomorrow = $derived(toTs(addDays(today, 1)));
  const todos = $derived(db.todos.items.filter((t) => !t.done && t.due_at != null && t.due_at < tomorrow));
  const habits = $derived(db.habits.items.filter((h) => !h.archived));
  const countdowns = $derived(
    db.countdowns.items
      .filter((c) => c.target_at * 1000 > clock.minute)
      .toSorted((a, b) => b.pinned - a.pinned || a.target_at - b.target_at)
      .slice(0, 3),
  );
</script>

{#snippet head(Icon: typeof Flame, title: string, href: string, link: string)}
  <Card.Header>
    <Card.Title class="flex items-center gap-2 text-sm"><Icon class="text-muted-foreground size-4" />{title}</Card.Title>
    <Card.Action>
      <a {href} class="text-muted-foreground hover:text-primary flex items-center gap-1 text-xs">{link}<ArrowRight class="size-3" /></a>
    </Card.Action>
  </Card.Header>
{/snippet}

{#snippet empty(text: string)}
  <p class="text-muted-foreground py-6 text-center text-sm">{text}</p>
{/snippet}

<Page title={greeting} sub="{fmtDate(now)}{week ? ` · 第 ${week} 周` : ''}">
  <div class="grid gap-4 md:grid-cols-2">
    {#if countdowns.length}
      <div class="grid grid-cols-3 gap-3 md:col-span-2">
        {#each countdowns as c (c.id)}
          <a
            href="#/countdowns"
            class="rounded-xl p-4 ring-1 ring-foreground/10 transition-transform hover:-translate-y-0.5"
            style="--c: {c.color ?? 'var(--primary)'}; background: color-mix(in oklch, var(--c) 12%, var(--card))"
          >
            <p class="text-muted-foreground truncate text-xs font-medium">{c.title}</p>
            <p class="num text-3xl font-bold tracking-tight md:text-4xl" style="color: var(--c)">
              {dayDiff(now, fromTs(c.target_at))}<span class="ml-1 text-sm font-semibold">天</span>
            </p>
            <p class="text-muted-foreground truncate text-xs">{fmtRelative(c.target_at, now, false)}</p>
          </a>
        {/each}
      </div>
    {/if}

    <Card.Root>
      {@render head(LayoutGrid, '今日课程', '#/timetable', '课表')}
      <Card.Content class="grid gap-2">
        {#each courses as c (c.id)}
          {@const state = nowMin >= toMinutes(c.end_time) ? 'past' : nowMin >= toMinutes(c.start_time) ? 'now' : ''}
          <div
            class={cn('flex items-center gap-3 rounded-lg border-l-3 px-3 py-2', state === 'past' && 'opacity-45')}
            style="border-color: {c.color}; background: color-mix(in oklch, {c.color} 10%, var(--card))"
          >
            <span class="num text-muted-foreground w-10 text-xs leading-tight">{c.start_time}<br />{c.end_time}</span>
            <div class="min-w-0 flex-1">
              <p class="font-medium">{c.name}</p>
              {#if c.room || c.teacher}
                <p class="text-muted-foreground text-xs">{[c.room, c.teacher].filter(Boolean).join(' · ')}</p>
              {/if}
            </div>
            {#if state === 'now'}<Badge>进行中</Badge>{/if}
          </div>
        {:else}
          {@render empty('今天没有课 🎉')}
        {/each}
      </Card.Content>
    </Card.Root>

    <Card.Root>
      {@render head(CalendarDays, '今日日程', '#/calendar', '日历')}
      <Card.Content class="grid gap-2">
        {#each events as e (e.id)}
          <div
            class="flex items-center gap-3 rounded-lg border-l-3 px-3 py-2"
            style="border-color: {e.color}; background: color-mix(in oklch, {e.color} 10%, var(--card))"
          >
            <span class="num text-muted-foreground w-10 text-xs">{e.all_day ? '全天' : hhmm(fromTs(e.start_at))}</span>
            <span class="font-medium">{e.title}</span>
          </div>
        {:else}
          {@render empty('没有日程安排')}
        {/each}
      </Card.Content>
    </Card.Root>

    <Card.Root>
      {@render head(CheckSquare, '今天要做', '#/todos', '全部')}
      <Card.Content class="grid">
        {#each todos as t (t.id)}
          <label class="flex items-center gap-3 border-b py-2.5 last:border-0">
            <Checkbox onCheckedChange={() => db.todos.patch(t.id, { done: 1, done_at: toTs(new Date()) })} />
            <span class="flex-1">{t.title}</span>
            <span class={cn('num text-xs', t.due_at! * 1000 < clock.minute ? 'text-destructive font-medium' : 'text-muted-foreground')}>
              {fmtRelative(t.due_at!, now)}
            </span>
          </label>
        {:else}
          {@render empty('今天的事都做完了')}
        {/each}
      </Card.Content>
    </Card.Root>

    <Card.Root>
      {@render head(Flame, '习惯打卡', '#/habits', '详情')}
      <Card.Content class="flex-row flex-wrap gap-2">
        {#each habits as h (h.id)}
          {@const on = habitLogs.has(`${h.id}:${todayStr}`)}
          <button
            class={cn(
              'flex items-center gap-1.5 rounded-full px-3.5 py-1.5 text-sm transition-all active:scale-95',
              on ? 'text-white' : 'bg-muted hover:bg-accent',
            )}
            style={on ? `background: ${h.color}` : ''}
            onclick={() => toggleHabit(h.id, todayStr)}
          >
            <span>{h.icon || '✨'}</span>{h.name}
          </button>
        {:else}
          <div class="w-full">{@render empty('还没有习惯，去添加一个吧')}</div>
        {/each}
      </Card.Content>
    </Card.Root>
  </div>
</Page>
