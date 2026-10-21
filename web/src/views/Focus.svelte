<script lang="ts">
  import { Pause, Play, RotateCcw, SkipForward } from '@lucide/svelte';
  import { onDestroy } from 'svelte';
  import { toast } from 'svelte-sonner';
  import Choice from '../components/Choice.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import TrendBars from '../components/TrendBars.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import * as Tabs from '$lib/components/ui/tabs';
  import { clock, db, saveSettings, settings } from '$lib/store.svelte';
  import { addDays, fmtRelative, fromTs, hhmm, pad, startOfDay, toTs } from '$lib/time';
  import { cn } from '$lib/utils';

  type Mode = 'focus' | 'short' | 'long';
  const LABEL: Record<Mode, string> = { focus: '专注', short: '短休息', long: '长休息' };
  const minutesOf = (m: Mode) =>
    Number(settings[`focus_${m}`]) || { focus: 25, short: 5, long: 15 }[m];

  // 计时状态存本地：刷新页面不丢；endsAt 是绝对时间，后台标签页也准
  type Timer = { mode: Mode; endsAt: number | null; remaining: number; startedAt: number | null; todoId: number | null; rounds: number };
  const KEY = 'congtodo.focus';
  function load(): Timer {
    try {
      const t = JSON.parse(localStorage.getItem(KEY) ?? '');
      if (t && t.mode) return t;
    } catch {}
    return { mode: 'focus', endsAt: null, remaining: minutesOf('focus') * 60, startedAt: null, todoId: null, rounds: 0 };
  }
  let timer = $state<Timer>(load());
  $effect(() => {
    try {
      localStorage.setItem(KEY, JSON.stringify(timer));
    } catch {}
  });

  const running = $derived(timer.endsAt != null);
  const left = $derived(running ? Math.max(0, Math.round((timer.endsAt! - clock.now) / 1000)) : timer.remaining);
  const total = $derived(minutesOf(timer.mode) * 60);
  const progress = $derived(1 - left / total);

  function start() {
    if (running) return;
    timer.startedAt ??= Math.floor(Date.now() / 1000);
    timer.endsAt = Date.now() + timer.remaining * 1000;
    if ('Notification' in window && Notification.permission === 'default') Notification.requestPermission();
  }
  function pause() {
    if (!running) return;
    timer.remaining = left;
    timer.endsAt = null;
  }
  function switchMode(m: Mode) {
    timer = { ...timer, mode: m, endsAt: null, remaining: minutesOf(m) * 60, startedAt: null };
  }
  const reset = () => switchMode(timer.mode);

  function beep() {
    try {
      const ctx = new AudioContext();
      [0, 0.25, 0.5].forEach((t) => {
        const o = ctx.createOscillator();
        const g = ctx.createGain();
        o.frequency.value = 880;
        g.gain.setValueAtTime(0.2, ctx.currentTime + t);
        g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + t + 0.2);
        o.connect(g).connect(ctx.destination);
        o.start(ctx.currentTime + t);
        o.stop(ctx.currentTime + t + 0.2);
      });
    } catch {}
  }

  let finishing = false;
  async function finish(skipped = false) {
    if (finishing) return;
    finishing = true;
    try {
      await complete(skipped);
    } finally {
      finishing = false;
    }
  }

  async function complete(skipped: boolean) {
    const mode = timer.mode;
    if (mode === 'focus') {
      const minutes = Math.round((total - left) / 60);
      if (minutes >= 1) {
        await db.focus.add({ todo_id: timer.todoId, started_at: timer.startedAt ?? toTs(new Date()) - minutes * 60, minutes });
      }
      timer.rounds += skipped ? 0 : 1;
    }
    if (!skipped) {
      beep();
      const msg = mode === 'focus' ? '专注完成，休息一下吧 ☕' : '休息结束，继续专注 💪';
      toast.success(msg);
      if ('Notification' in window && Notification.permission === 'granted') new Notification('Congtodo', { body: msg });
    }
    // 每 4 个番茄来一次长休息
    switchMode(mode === 'focus' ? (timer.rounds % 4 === 0 && timer.rounds ? 'long' : 'short') : 'focus');
  }

  // 到点自动结束
  $effect(() => {
    if (running && left === 0) finish();
  });

  // 页面标题显示倒计时
  const originalTitle = document.title;
  $effect(() => {
    document.title = running ? `${pad(Math.floor(left / 60))}:${pad(left % 60)} ${LABEL[timer.mode]} · Congtodo` : originalTitle;
  });
  onDestroy(() => (document.title = originalTitle));

  // ---------- 统计 ----------
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

  let durations = $state({ focus: minutesOf('focus'), short: minutesOf('short'), long: minutesOf('long') });
  async function saveDurations() {
    await saveSettings({ focus_focus: String(durations.focus), focus_short: String(durations.short), focus_long: String(durations.long) });
    if (!running) reset();
    toast.success('已保存');
  }

  const R = 120;
  const C = 2 * Math.PI * R;
</script>

<Page title="专注" sub="番茄工作法：专注一段时间，休息一下，再继续">
  <div class="grid gap-4 lg:grid-cols-[minmax(0,1fr)_20rem]">
    <Card.Root>
      <Card.Content class="grid justify-items-center gap-6 py-4">
        <Tabs.Root value={timer.mode} onValueChange={(v) => !running && switchMode(v as Mode)}>
          <Tabs.List>
            {#each ['focus', 'short', 'long'] as const as m (m)}
              <Tabs.Trigger value={m} disabled={running}>{LABEL[m]}</Tabs.Trigger>
            {/each}
          </Tabs.List>
        </Tabs.Root>

        <div class="relative grid size-64 place-items-center sm:size-72">
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
            <span class="num text-6xl font-semibold tracking-tight">{pad(Math.floor(left / 60))}:{pad(left % 60)}</span>
            <span class="text-muted-foreground text-sm">{LABEL[timer.mode]}{timer.todoId && timer.mode === 'focus' ? ` · ${todoTitle(timer.todoId) ?? ''}` : ''}</span>
          </div>
        </div>

        <div class="flex items-center gap-2">
          <Button variant="outline" size="icon-lg" aria-label="重置" onclick={reset}><RotateCcw /></Button>
          {#if running}
            <Button size="lg" class="w-32" onclick={pause}><Pause />暂停</Button>
          {:else}
            <Button size="lg" class="w-32" onclick={start}><Play />{timer.startedAt ? '继续' : '开始'}</Button>
          {/if}
          <Button variant="outline" size="icon-lg" aria-label="跳过" onclick={() => finish(true)}><SkipForward /></Button>
        </div>

        {#if timer.mode === 'focus'}
          <div class="w-full max-w-sm">
            <Field label="这次专注做什么？"><Choice bind:value={timer.todoId} options={todoOptions} /></Field>
          </div>
        {/if}
        <p class="text-muted-foreground text-xs">已完成 {timer.rounds} 个番茄 · 每 4 个番茄后长休息</p>
      </Card.Content>
    </Card.Root>

    <div class="grid content-start gap-4">
      <Card.Root>
        <Card.Header>
          <Card.Title class="text-sm">今天</Card.Title>
          <Card.Description>
            <b class="num text-foreground text-2xl">{todayMinutes}</b> 分钟 · {todaySessions.length} 次专注
          </Card.Description>
        </Card.Header>
        <Card.Content class="grid gap-1.5">
          {#each todaySessions as f (f.id)}
            <div class={cn('flex justify-between text-sm')}>
              <span class="truncate">{todoTitle(f.todo_id) ?? '专注'}</span>
              <span class="num text-muted-foreground shrink-0">{hhmm(fromTs(f.started_at))} · {f.minutes} 分钟</span>
            </div>
          {:else}
            <p class="text-muted-foreground text-sm">还没有专注记录</p>
          {/each}
        </Card.Content>
      </Card.Root>

      <Card.Root>
        <Card.Header>
          <Card.Title class="text-sm">近 7 天</Card.Title>
          <Card.Description>共 <b class="num text-foreground">{week.total}</b> 分钟 · 今天 <b class="num text-foreground">{todayMinutes}</b> 分钟</Card.Description>
        </Card.Header>
        <Card.Content>
          <TrendBars values={week.bars} color="var(--primary)" label="近 7 天每天专注分钟数" percent={false} />
        </Card.Content>
      </Card.Root>

      <Card.Root>
        <Card.Header><Card.Title class="text-sm">时长（分钟）</Card.Title></Card.Header>
        <Card.Content>
          <form class="grid gap-3" onsubmit={(e) => (e.preventDefault(), saveDurations())}>
            <div class="grid grid-cols-3 gap-2">
              <Field label="专注"><Input type="number" min="1" max="180" bind:value={durations.focus} /></Field>
              <Field label="短休息"><Input type="number" min="1" max="60" bind:value={durations.short} /></Field>
              <Field label="长休息"><Input type="number" min="1" max="60" bind:value={durations.long} /></Field>
            </div>
            <Button type="submit" variant="outline" size="sm" class="justify-self-start">保存</Button>
          </form>
        </Card.Content>
      </Card.Root>
    </div>
  </div>
  {#if db.focus.items.length}
    <p class="text-muted-foreground mt-3 text-xs">最近一次：{fmtRelative(db.focus.items[0].started_at, new Date())}</p>
  {/if}
</Page>
