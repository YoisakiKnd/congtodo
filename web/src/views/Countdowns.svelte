<script lang="ts">
  import { Pin, Plus } from '@lucide/svelte';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { clock, db, PALETTE } from '$lib/store.svelte';
  import {
    addDays,
    dayDiff,
    fmtRelative,
    fromInput,
    fromTs,
    pad,
    REMIND_OPTIONS,
    split,
    startOfDay,
    toInput,
    toTs,
  } from '$lib/time';
  import type { Countdown } from '$lib/types';
  import { cn } from '$lib/utils';

  const now = $derived(new Date(clock.minute));
  const sorted = $derived(
    db.countdowns.items.toSorted((a, b) => {
      const pa = a.target_at * 1000 < clock.minute ? 1 : 0;
      const pb = b.target_at * 1000 < clock.minute ? 1 : 0;
      return pa - pb || b.pinned - a.pinned || (pa ? b.target_at - a.target_at : a.target_at - b.target_at);
    }),
  );

  let sheet = $state(false);
  let form = $state({ id: 0, title: '', target: '', color: PALETTE[0] as string | null, pinned: false, remind_before: 1440 as number | null });

  function edit(c?: Countdown) {
    form = c
      ? { id: c.id, title: c.title, target: toInput(c.target_at), color: c.color, pinned: !!c.pinned, remind_before: c.remind_before }
      : { id: 0, title: '', target: toInput(toTs(addDays(startOfDay(new Date()), 7))), color: PALETTE[0], pinned: false, remind_before: 1440 };
    sheet = true;
  }

  function save() {
    return db.countdowns.save({
      id: form.id || undefined,
      title: form.title.trim(),
      target_at: fromInput(form.target)!,
      color: form.color,
      pinned: +form.pinned,
      remind_before: form.remind_before,
    });
  }
</script>

<Page title="倒数日">
  {#snippet actions()}
    <Button onclick={() => edit()}><Plus />新建</Button>
  {/snippet}

  <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
    {#each sorted as c (c.id)}
      {@const p = split(c.target_at * 1000 - clock.now)}
      <button
        class={cn(
          'grid gap-1 rounded-xl p-5 text-left shadow-xs ring-1 ring-foreground/10 transition-all hover:-translate-y-0.5 hover:shadow-md',
          p.past && 'opacity-60 grayscale',
        )}
        style="--c: {c.color ?? 'var(--primary)'}; background: linear-gradient(145deg, color-mix(in oklch, var(--c) 16%, var(--card)), var(--card) 70%)"
        onclick={() => edit(c)}
      >
        <span class="text-muted-foreground flex items-center justify-between text-sm font-medium">
          {c.title}
          {#if c.pinned}<Pin class="size-3.5" />{/if}
        </span>
        <span class="num text-5xl font-bold tracking-tight" style="color: var(--c)">
          {#if p.past}<span class="mr-1 text-base font-semibold">已过</span>{/if}{Math.abs(dayDiff(now, fromTs(c.target_at)))}<span
            class="ml-1 text-base font-semibold">天</span
          >
        </span>
        {#if !p.past}
          <span class="num text-muted-foreground font-mono text-xs">
            {p.days}天 {pad(p.hours)}:{pad(p.minutes)}:{pad(p.seconds)}
          </span>
        {/if}
        <span class="text-muted-foreground/70 text-xs">{fmtRelative(c.target_at, now)}</span>
      </button>
    {:else}
      <p class="text-muted-foreground col-span-full rounded-xl border border-dashed py-12 text-center text-sm">
        添加一个值得期待的日子吧：考试、生日、假期……
      </p>
    {/each}
  </div>
</Page>

<Sheet
  bind:open={sheet}
  title={form.id ? '编辑倒数日' : '新建倒数日'}
  onsave={save}
  ondelete={form.id ? () => db.countdowns.remove(form.id) : undefined}
>
  <Field label="名称"><Input required bind:value={form.title} placeholder="例如：期末考试" /></Field>
  <Field label="目标时间"><Input required type="datetime-local" bind:value={form.target} /></Field>
  <Field label="提醒"><Choice bind:value={form.remind_before} options={REMIND_OPTIONS} /></Field>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.pinned} />置顶</label>
</Sheet>
