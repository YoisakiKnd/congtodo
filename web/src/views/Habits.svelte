<script lang="ts">
  import { Bell, Check, Minus, NotebookPen, Plus } from '@lucide/svelte';
  import ChannelPick from '../components/ChannelPick.svelte';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import TrendBars from '../components/TrendBars.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { Textarea } from '$lib/components/ui/textarea';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { clock, db, habitCount, habitDone, habitLog, logHabit, PALETTE, toggleHabit } from '$lib/store.svelte';
  import { addDays, isoWeekday, startOfDay, startOfWeek, WEEKDAYS, ymd } from '$lib/time';
  import type { ChannelIds, Habit } from '$lib/types';
  import { cn } from '$lib/utils';

  const WEEKS = 18;
  const TREND_WEEKS = 12;

  const today = $derived(startOfDay(new Date(clock.minute)));
  const todayStr = $derived(ymd(today));
  const habits = $derived(db.habits.items.toSorted((a, b) => a.archived - b.archived || a.id - b.id));

  /** 该习惯在某天是否需要打卡 */
  const planned = (h: Habit, d: Date) => !h.days || h.days.split(',').map(Number).includes(isoWeekday(d));

  // 热力图：WEEKS 列 × 7 行，按周一对齐
  const heatDays = $derived.by(() => {
    const start = addDays(startOfWeek(today), -(WEEKS - 1) * 7);
    return Array.from({ length: WEEKS * 7 }, (_, i) => {
      const d = addDays(start, i);
      return { d, key: ymd(d), future: d > today, label: `${d.getMonth() + 1}月${d.getDate()}日` };
    });
  });

  function stats(h: Habit) {
    // 连续天数：只看计划内的日子，今天还没完成不打断
    let d = today;
    let streak = 0;
    if (!habitDone(h, ymd(d))) d = addDays(d, -1);
    for (let i = 0; i < 400; i++, d = addDays(d, -1)) {
      if (!planned(h, d)) continue;
      if (!habitDone(h, ymd(d))) break;
      streak++;
    }
    let week = 0;
    for (let x = startOfWeek(today); x <= today; x = addDays(x, 1)) if (habitDone(h, ymd(x))) week++;
    // 近 12 周每周完成率 = 完成天数 / 计划天数
    const trend = Array.from({ length: TREND_WEEKS }, (_, i) => {
      const start = addDays(startOfWeek(today), -(TREND_WEEKS - 1 - i) * 7);
      let plan = 0;
      let hit = 0;
      for (let k = 0; k < 7; k++) {
        const day = addDays(start, k);
        if (day > today || !planned(h, day)) continue;
        plan++;
        if (habitDone(h, ymd(day))) hit++;
      }
      return { label: `${start.getMonth() + 1}/${start.getDate()} 起`, value: plan ? hit / plan : 0, detail: `${hit}/${plan} 天`, plan };
    });
    const counted = trend.filter((t) => t.plan);
    const avg = counted.length ? counted.reduce((s, t) => s + t.value, 0) / counted.length : 0;
    return { streak, week, trend, avg };
  }

  const daysLabel = (h: Habit) =>
    !h.days ? '每天' : h.days === '1,2,3,4,5' ? '工作日' : h.days.split(',').map((n) => `周${WEEKDAYS[+n - 1]}`).join(' ');

  // ---------- 编辑习惯 ----------
  let sheet = $state(false);
  let form = $state({
    id: 0,
    name: '',
    icon: '',
    color: PALETTE[0] as string | null,
    target: 7,
    remind_time: '21:00',
    archived: false,
    days: [] as string[],
    goal: 1,
    unit: '',
    channels: null as ChannelIds,
  });

  function edit(h?: Habit) {
    form = h
      ? {
          id: h.id,
          name: h.name,
          icon: h.icon ?? '',
          color: h.color,
          target: h.target,
          remind_time: h.remind_time ?? '',
          archived: !!h.archived,
          days: h.days ? h.days.split(',') : [],
          goal: h.goal || 1,
          unit: h.unit ?? '',
          channels: h.channels,
        }
      : {
          id: 0,
          name: '',
          icon: '',
          color: PALETTE[(db.habits.items.length * 3) % PALETTE.length],
          target: 7,
          remind_time: '21:00',
          archived: false,
          days: [],
          goal: 1,
          unit: '',
          channels: null,
        };
    sheet = true;
  }

  function save() {
    const days = form.days.length && form.days.length < 7 ? form.days.toSorted().join(',') : null;
    return db.habits.save({
      id: form.id || undefined,
      name: form.name.trim(),
      icon: form.icon || null,
      color: form.color,
      // 指定了星期时，每周目标不超过计划天数
      target: days ? Math.min(form.target, form.days.length) : form.target,
      remind_time: form.remind_time || null,
      archived: +form.archived,
      days,
      goal: Math.max(1, Math.round(form.goal) || 1),
      unit: form.unit.trim() || null,
      channels: form.channels,
    });
  }

  // ---------- 某天的记录（次数 + 备注） ----------
  let daySheet = $state(false);
  let dayForm = $state({ habit: null as Habit | null, day: '', label: '', count: 0, note: '' });

  function editDay(h: Habit, day: string, label: string) {
    const log = habitLog(h.id, day);
    dayForm = { habit: h, day, label, count: log?.count ?? 0, note: log?.note ?? '' };
    daySheet = true;
  }

  const ICONS = ['🏃', '📖', '💧', '🧘', '💤', '🥗', '✍️', '🎹', '🧠', '🌅', '💪', '🚭'];
  const TARGETS = [1, 2, 3, 4, 5, 6, 7].map((n) => ({ value: n, label: n === 7 ? '每天' : `每周 ${n} 次` }));
</script>

<Page title="习惯养成">
  {#snippet actions()}
    <Button onclick={() => edit()}><Plus />新习惯</Button>
  {/snippet}

  <Tooltip.Provider delayDuration={200}>
    <div class="grid gap-4 lg:grid-cols-2">
      {#each habits as h (h.id)}
        {@const s = stats(h)}
        {@const count = habitCount(h.id, todayStr)}
        {@const done = habitDone(h, todayStr)}
        {@const goal = h.goal || 1}
        <Card.Root class={cn(h.archived && 'opacity-50')} style="--c: {h.color ?? 'var(--primary)'}">
          <Card.Content class="grid gap-4">
            <div class="flex items-center gap-3">
              <button
                class="grid size-11 shrink-0 place-items-center rounded-xl text-2xl"
                style="background: color-mix(in oklch, var(--c) 16%, var(--card))"
                onclick={() => edit(h)}
                aria-label="编辑">{h.icon || '✨'}</button
              >
              <div class="min-w-0 flex-1">
                <p class="font-semibold">{h.name}</p>
                <p class="text-muted-foreground flex flex-wrap items-center gap-x-2 text-xs">
                  <span>本周 <b class="num text-foreground">{s.week}/{h.target}</b></span>
                  {#if s.streak}<span>🔥 连续 <b class="num text-foreground">{s.streak}</b> 次</span>{/if}
                  <span>{daysLabel(h)}</span>
                  {#if h.remind_time}<span class="flex items-center gap-1"><Bell class="size-3" />{h.remind_time}</span>{/if}
                </p>
              </div>
              {#if goal > 1}
                <!-- 计数型：今天的次数 / 目标 -->
                <div class="flex items-center gap-1">
                  <Button variant="ghost" size="icon-sm" aria-label="减一" disabled={!count} onclick={() => logHabit(h.id, todayStr, count - 1)}>
                    <Minus />
                  </Button>
                  <button
                    class={cn('num min-w-14 rounded-full border-2 px-2 py-1 text-center text-sm font-semibold transition-colors', done && 'text-white')}
                    style="border-color: var(--c); {done ? 'background: var(--c)' : ''}"
                    onclick={() => logHabit(h.id, todayStr, count + 1)}
                    aria-label="加一"
                  >
                    {count}/{goal}<span class="ml-0.5 text-xs font-normal">{h.unit ?? ''}</span>
                  </button>
                </div>
              {:else}
                <button
                  class={cn(
                    'grid size-11 shrink-0 place-items-center rounded-full border-2 transition-all active:scale-90',
                    done ? 'text-white' : 'text-transparent hover:text-muted-foreground/40',
                  )}
                  style="border-color: var(--c); {done ? 'background: var(--c)' : ''}"
                  onclick={() => toggleHabit(h, todayStr)}
                  aria-label="今日打卡"
                >
                  <Check class="size-5" strokeWidth={3} />
                </button>
              {/if}
            </div>

            <div class="bg-muted h-1.5 overflow-hidden rounded-full">
              <div
                class="h-full rounded-full transition-[width] duration-500"
                style="width: {Math.min(100, (s.week / h.target) * 100)}%; background: var(--c)"
              ></div>
            </div>

            <!-- 热力图：颜色深浅 = 当天完成度；点击查看 / 补记次数和备注 -->
            <div class="grid grid-flow-col grid-rows-7 gap-[3px]" style="grid-auto-columns: minmax(0, 1fr)">
              {#each heatDays as d (d.key)}
                {@const n = habitCount(h.id, d.key)}
                {@const log = habitLog(h.id, d.key)}
                <Tooltip.Root>
                  <Tooltip.Trigger
                    disabled={d.future}
                    class={cn(
                      'bg-muted relative aspect-square rounded-[3px] transition-colors disabled:invisible',
                      d.key === todayStr && 'ring-muted-foreground ring-1',
                      !planned(h, d.d) && !n && 'opacity-40',
                    )}
                    style={n ? `background: color-mix(in oklch, var(--c) ${Math.round(35 + 65 * Math.min(1, n / goal))}%, var(--muted))` : ''}
                    onclick={() => (goal > 1 || log?.note ? editDay(h, d.key, d.label) : toggleHabit(h, d.key))}
                    oncontextmenu={(e: MouseEvent) => (e.preventDefault(), editDay(h, d.key, d.label))}
                  >
                    {#if log?.note}<span class="absolute top-0.5 right-0.5 size-1 rounded-full bg-white/80"></span>{/if}
                  </Tooltip.Trigger>
                  <Tooltip.Content>
                    {d.label}{n ? ` · ${goal > 1 ? `${n}/${goal}${h.unit ?? ''}` : '已完成'}` : ''}{log?.note ? ` · ${log.note}` : ''}
                  </Tooltip.Content>
                </Tooltip.Root>
              {/each}
            </div>

            <div class="grid gap-1.5">
              <div class="text-muted-foreground flex items-baseline justify-between text-xs">
                <span>近 {TREND_WEEKS} 周完成率</span>
                <span>
                  本周 <b class="num text-foreground">{Math.round(s.trend[TREND_WEEKS - 1].value * 100)}%</b> · 平均
                  <b class="num text-foreground">{Math.round(s.avg * 100)}%</b>
                </span>
              </div>
              <TrendBars values={s.trend} color={h.color ?? 'var(--primary)'} label="{h.name}近 {TREND_WEEKS} 周每周完成率" />
            </div>
            <button
              class="text-muted-foreground hover:text-foreground -mt-1 flex items-center gap-1 justify-self-start text-xs"
              onclick={() => editDay(h, todayStr, '今天')}
            >
              <NotebookPen class="size-3.5" />{habitLog(h.id, todayStr)?.note ? '查看今天的备注' : '写今天的备注'}
            </button>
          </Card.Content>
        </Card.Root>
      {:else}
        <p class="text-muted-foreground col-span-full rounded-xl border border-dashed py-12 text-center text-sm">
          从一个小习惯开始：每天喝 8 杯水、读 10 页书……
        </p>
      {/each}
    </div>
  </Tooltip.Provider>
  <p class="text-muted-foreground mt-3 text-xs">点击格子打卡；计数型习惯或有备注的日子点击后可编辑次数与备注（右键 / 长按也可以）。</p>
</Page>

<Sheet bind:open={sheet} title={form.id ? '编辑习惯' : '新习惯'} onsave={save} ondelete={form.id ? () => db.habits.remove(form.id) : undefined}>
  <Field label="名称"><Input required bind:value={form.name} placeholder="例如：喝水" /></Field>
  <div class="grid gap-2">
    <span class="text-sm font-medium">图标</span>
    <div class="flex flex-wrap items-center gap-1.5">
      {#each ICONS as i (i)}
        <button
          type="button"
          class={cn('bg-muted size-9 rounded-lg text-lg transition-all', form.icon === i && 'ring-primary ring-2')}
          onclick={() => (form.icon = i)}>{i}</button
        >
      {/each}
      <Input class="w-20" bind:value={form.icon} placeholder="自定义" />
    </div>
  </div>
  <div class="grid gap-2">
    <span class="text-sm font-medium">打卡日</span>
    <ToggleGroup.Root type="multiple" variant="outline" size="sm" bind:value={form.days} class="w-full">
      {#each WEEKDAYS as w, i (i)}
        <ToggleGroup.Item value={String(i + 1)} class="flex-1">{w}</ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
    <span class="text-muted-foreground text-xs">不选 = 每天；只在选中的日子提醒和统计</span>
  </div>
  <div class="grid grid-cols-3 gap-3">
    <Field label="每天目标次数"><Input type="number" min="1" max="99" bind:value={form.goal} /></Field>
    <Field label="单位"><Input bind:value={form.unit} placeholder="杯 / 页 / 次" /></Field>
    <Field label="每周目标"><Choice bind:value={form.target} options={TARGETS} /></Field>
  </div>
  <Field label="每日提醒（未完成时）"><Input type="time" bind:value={form.remind_time} /></Field>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <ChannelPick bind:value={form.channels} />
  {#if form.id}
    <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.archived} />归档（停止提醒）</label>
  {/if}
</Sheet>

<Sheet
  bind:open={daySheet}
  title="{dayForm.habit?.name} · {dayForm.label}"
  onsave={() => logHabit(dayForm.habit!.id, dayForm.day, dayForm.count, dayForm.note.trim() || null)}
>
  <div class="grid gap-2">
    <span class="text-sm font-medium">完成次数（目标 {dayForm.habit?.goal ?? 1}{dayForm.habit?.unit ?? ''}）</span>
    <div class="flex items-center gap-2">
      <Button variant="outline" size="icon" disabled={!dayForm.count} onclick={() => dayForm.count--}><Minus /></Button>
      <Input type="number" min="0" class="w-20 text-center" bind:value={dayForm.count} />
      <Button variant="outline" size="icon" onclick={() => dayForm.count++}><Plus /></Button>
    </div>
  </div>
  <Field label="备注"><Textarea bind:value={dayForm.note} placeholder="今天的感受、读了什么……" /></Field>
</Sheet>
