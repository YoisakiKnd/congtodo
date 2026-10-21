<script lang="ts">
  // 专注面板：计时器 + 统计 + 时长设置。计时状态在 $lib/focus（悬浮小窗共用）
  import { Pause, Play, RotateCcw, SkipForward } from '@lucide/svelte';
  import { toast } from 'svelte-sonner';
  import Choice from '../Choice.svelte';
  import Field from '../Field.svelte';
  import TrendBars from '../TrendBars.svelte';
  import Panel from './Panel.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import * as Tabs from '$lib/components/ui/tabs';
  import {
    finishFocus,
    focus,
    FOCUS_LABEL,
    focusMinutes,
    pauseFocus,
    resetFocus,
    setFocusTodo,
    startFocus,
    switchMode,
    timer,
    type FocusMode,
  } from '$lib/focus.svelte';
  import { clock, db, saveSettings } from '$lib/store.svelte';
  import { addDays, fromTs, hhmm, pad, startOfDay, toTs } from '$lib/time';

  const progress = $derived(1 - focus.left / focus.total);

  const today = $derived(startOfDay(new Date(clock.minute)));
  const todaySessions = $derived(db.focus.items.filter((f) => f.started_at >= toTs(today)));
  const todayMinutes = $derived(todaySessions.reduce((s, f) => s + f.minutes, 0));
  const week = $derived.by(() => {
    const days = Array.from({ length: 7 }, (_, i) => addDays(today, i - 6));
    const mins = days.map((d) =>
      db.focus.items.filter((f) => f.started_at >= toTs(d) && f.started_at < toTs(addDays(d, 1))).reduce((s, f) => s + f.minutes, 0),
    );
    const max = Math.max(60, ...mins);
    return {
      total: mins.reduce((a, b) => a + b, 0),
      bars: days.map((d, i) => ({ label: `${d.getMonth() + 1}/${d.getDate()}`, value: mins[i] / max, detail: `${mins[i]} 分钟` })),
    };
  });

  const todoOptions = $derived([
    { value: null as number | null, label: '不关联待办' },
    ...db.todos.items.filter((t) => !t.done).map((t) => ({ value: t.id as number | null, label: t.title })),
  ]);
  const todoTitle = (id: number | null) => db.todos.items.find((t) => t.id === id)?.title;

  let durations = $state({ focus: focusMinutes('focus'), short: focusMinutes('short'), long: focusMinutes('long') });
  async function saveDurations() {
    await saveSettings({ focus_focus: String(durations.focus), focus_short: String(durations.short), focus_long: String(durations.long) });
    if (!focus.active) resetFocus();
    toast.success('已保存');
  }

  const R = 120;
  const C = 2 * Math.PI * R;
</script>

<Panel name="focus" title="专注">
  <div class="grid gap-4 md:grid-cols-[minmax(0,1fr)_18rem]">
    <div class="grid justify-items-center gap-5 py-2">
      <Tabs.Root value={timer.mode} onValueChange={(v) => !focus.running && switchMode(v as FocusMode)}>
        <Tabs.List>
          {#each ['focus', 'short', 'long'] as const as m (m)}
            <Tabs.Trigger value={m} disabled={focus.running}>{FOCUS_LABEL[m]}</Tabs.Trigger>
          {/each}
        </Tabs.List>
      </Tabs.Root>

      <div class="relative grid size-60 place-items-center">
        <svg viewBox="0 0 260 260" class="absolute inset-0 size-full -rotate-90">
          <circle cx="130" cy="130" r={R} fill="none" stroke="var(--muted)" stroke-width="10" />
          <circle
            cx="130"
            cy="130"
            r={R}
            fill="none"
            stroke={timer.mode === 'focus' ? 'var(--primary)' : '#30a46c'}
            stroke-width="10"
            stroke-linecap="round"
            stroke-dasharray={C}
            stroke-dashoffset={C * (1 - progress)}
            class="transition-[stroke-dashoffset] duration-1000 ease-linear"
          />
        </svg>
        <div class="grid justify-items-center gap-1">
          <span class="num text-5xl font-semibold tracking-tight">{pad(Math.floor(focus.left / 60))}:{pad(focus.left % 60)}</span>
          <span class="text-muted-foreground max-w-44 truncate text-sm">
            {FOCUS_LABEL[timer.mode]}{timer.todoId && timer.mode === 'focus' ? ` · ${todoTitle(timer.todoId) ?? ''}` : ''}
          </span>
        </div>
      </div>

      <div class="flex items-center gap-2">
        <Button variant="outline" size="icon-lg" aria-label="重置" onclick={resetFocus}><RotateCcw /></Button>
        {#if focus.running}
          <Button size="lg" class="w-32" onclick={pauseFocus}><Pause />暂停</Button>
        {:else}
          <Button size="lg" class="w-32" onclick={() => startFocus()}><Play />{timer.startedAt ? '继续' : '开始'}</Button>
        {/if}
        <Button variant="outline" size="icon-lg" aria-label="跳过" onclick={() => finishFocus(true)}><SkipForward /></Button>
      </div>

      {#if timer.mode === 'focus'}
        <div class="w-full max-w-sm">
          <Field label="这次专注做什么？">
            <Choice bind:value={() => timer.todoId, (v) => setFocusTodo(v)} options={todoOptions} />
          </Field>
        </div>
      {/if}
      <p class="text-muted-foreground text-xs">已完成 {timer.rounds} 个番茄 · 每 4 个番茄后长休息</p>
    </div>

    <div class="grid content-start gap-3">
      <Card.Root size="sm">
        <Card.Header>
          <Card.Title class="text-sm">今天</Card.Title>
          <Card.Description><b class="num text-foreground text-2xl">{todayMinutes}</b> 分钟 · {todaySessions.length} 次专注</Card.Description>
        </Card.Header>
        <Card.Content class="grid gap-1.5">
          {#each todaySessions as f (f.id)}
            <div class="flex justify-between gap-2 text-sm">
              <span class="truncate">{todoTitle(f.todo_id) ?? '专注'}</span>
              <span class="num text-muted-foreground shrink-0">{hhmm(fromTs(f.started_at))} · {f.minutes} 分钟</span>
            </div>
          {:else}
            <p class="text-muted-foreground text-sm">还没有专注记录</p>
          {/each}
        </Card.Content>
      </Card.Root>
      <Card.Root size="sm">
        <Card.Header>
          <Card.Title class="text-sm">近 7 天</Card.Title>
          <Card.Description>共 <b class="num text-foreground">{week.total}</b> 分钟</Card.Description>
        </Card.Header>
        <Card.Content><TrendBars values={week.bars} color="var(--primary)" label="近 7 天每天专注分钟数" percent={false} /></Card.Content>
      </Card.Root>
      <Card.Root size="sm">
        <Card.Header><Card.Title class="text-sm">时长（分钟）</Card.Title></Card.Header>
        <Card.Content>
          <form class="grid gap-3" onsubmit={(e) => (e.preventDefault(), saveDurations())}>
            <div class="grid grid-cols-3 gap-2">
              <Field label="专注"><Input type="number" min="1" max="180" bind:value={durations.focus} /></Field>
              <Field label="短休"><Input type="number" min="1" max="60" bind:value={durations.short} /></Field>
              <Field label="长休"><Input type="number" min="1" max="60" bind:value={durations.long} /></Field>
            </div>
            <Button type="submit" variant="outline" size="sm" class="justify-self-start">保存</Button>
          </form>
        </Card.Content>
      </Card.Root>
    </div>
  </div>
</Panel>
