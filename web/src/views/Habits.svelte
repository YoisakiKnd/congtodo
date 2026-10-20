<script lang="ts">
  import { Bell, Check, Plus } from '@lucide/svelte';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import * as Tooltip from '$lib/components/ui/tooltip';
  import { clock, db, habitLogs, PALETTE, toggleHabit } from '$lib/store.svelte';
  import { addDays, startOfDay, startOfWeek, ymd } from '$lib/time';
  import type { Habit } from '$lib/types';
  import { cn } from '$lib/utils';

  const WEEKS = 18;

  const today = $derived(startOfDay(new Date(clock.minute)));
  const todayStr = $derived(ymd(today));
  const habits = $derived(db.habits.items.toSorted((a, b) => a.archived - b.archived || a.id - b.id));

  // 热力图：WEEKS 列 × 7 行，按周一对齐
  const heatDays = $derived.by(() => {
    const start = addDays(startOfWeek(today), -(WEEKS - 1) * 7);
    return Array.from({ length: WEEKS * 7 }, (_, i) => {
      const d = addDays(start, i);
      return { key: ymd(d), future: d > today, label: `${d.getMonth() + 1}月${d.getDate()}日` };
    });
  });

  const has = (id: number, day: string) => habitLogs.has(`${id}:${day}`);

  function stats(h: Habit) {
    let d = has(h.id, todayStr) ? today : addDays(today, -1);
    let streak = 0;
    while (has(h.id, ymd(d))) {
      streak++;
      d = addDays(d, -1);
    }
    let week = 0;
    for (let x = startOfWeek(today); x <= today; x = addDays(x, 1)) if (has(h.id, ymd(x))) week++;
    return { streak, week };
  }

  let sheet = $state(false);
  let form = $state({ id: 0, name: '', icon: '', color: PALETTE[0] as string | null, target: 7, remind_time: '21:00', archived: false });

  function edit(h?: Habit) {
    form = h
      ? { id: h.id, name: h.name, icon: h.icon ?? '', color: h.color, target: h.target, remind_time: h.remind_time ?? '', archived: !!h.archived }
      : { id: 0, name: '', icon: '', color: PALETTE[(db.habits.items.length * 3) % PALETTE.length], target: 7, remind_time: '21:00', archived: false };
    sheet = true;
  }

  function save() {
    return db.habits.save({
      id: form.id || undefined,
      name: form.name.trim(),
      icon: form.icon || null,
      color: form.color,
      target: form.target,
      remind_time: form.remind_time || null,
      archived: +form.archived,
    });
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
        {@const done = has(h.id, todayStr)}
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
                  {#if s.streak}<span>🔥 连续 <b class="num text-foreground">{s.streak}</b> 天</span>{/if}
                  {#if h.remind_time}<span class="flex items-center gap-1"><Bell class="size-3" />{h.remind_time}</span>{/if}
                </p>
              </div>
              <button
                class={cn(
                  'grid size-11 shrink-0 place-items-center rounded-full border-2 transition-all active:scale-90',
                  done ? 'text-white' : 'text-transparent hover:text-muted-foreground/40',
                )}
                style="border-color: var(--c); {done ? 'background: var(--c)' : ''}"
                onclick={() => toggleHabit(h.id, todayStr)}
                aria-label="今日打卡"
              >
                <Check class="size-5" strokeWidth={3} />
              </button>
            </div>

            <div class="bg-muted h-1.5 overflow-hidden rounded-full">
              <div
                class="h-full rounded-full transition-[width] duration-500"
                style="width: {Math.min(100, (s.week / h.target) * 100)}%; background: var(--c)"
              ></div>
            </div>

            <div class="grid grid-flow-col grid-rows-7 gap-[3px]" style="grid-auto-columns: minmax(0, 1fr)">
              {#each heatDays as d (d.key)}
                <Tooltip.Root>
                  <Tooltip.Trigger
                    disabled={d.future}
                    class={cn(
                      'bg-muted aspect-square rounded-[3px] transition-colors disabled:invisible',
                      d.key === todayStr && 'ring-muted-foreground ring-1',
                    )}
                    style={has(h.id, d.key) ? 'background: var(--c)' : ''}
                    onclick={() => toggleHabit(h.id, d.key)}
                  />
                  <Tooltip.Content>{d.label}</Tooltip.Content>
                </Tooltip.Root>
              {/each}
            </div>
          </Card.Content>
        </Card.Root>
      {:else}
        <p class="text-muted-foreground col-span-full rounded-xl border border-dashed py-12 text-center text-sm">
          从一个小习惯开始：每天喝 8 杯水、读 10 页书……
        </p>
      {/each}
    </div>
  </Tooltip.Provider>
</Page>

<Sheet bind:open={sheet} title={form.id ? '编辑习惯' : '新习惯'} onsave={save} ondelete={form.id ? () => db.habits.remove(form.id) : undefined}>
  <Field label="名称"><Input required bind:value={form.name} placeholder="例如：早起" /></Field>
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
  <div class="grid grid-cols-2 gap-3">
    <Field label="目标"><Choice bind:value={form.target} options={TARGETS} /></Field>
    <Field label="每日提醒"><Input type="time" bind:value={form.remind_time} /></Field>
  </div>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  {#if form.id}
    <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.archived} />归档（停止提醒）</label>
  {/if}
</Sheet>
