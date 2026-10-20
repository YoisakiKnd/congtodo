<script lang="ts">
  import ColorPick from '../components/ColorPick.svelte';
  import Icon from '../components/Icon.svelte';
  import RemindSelect from '../components/RemindSelect.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { clock, db, PALETTE } from '../lib/store.svelte';
  import { addDays, dayDiff, fmtRelative, fromInput, fromTs, pad, split, startOfDay, toInput, toTs } from '../lib/time';
  import type { Countdown } from '../lib/types';

  const now = $derived(new Date(clock.minute));
  const sorted = $derived(
    db.countdowns.items.toSorted((a, b) => {
      const pa = a.target_at * 1000 < clock.minute ? 1 : 0;
      const pb = b.target_at * 1000 < clock.minute ? 1 : 0;
      return pa - pb || b.pinned - a.pinned || (pa ? b.target_at - a.target_at : a.target_at - b.target_at);
    }),
  );

  let open = $state(false);
  let form = $state<Partial<Countdown> & { target: string }>({ target: '' });

  function edit(c?: Countdown) {
    form = c
      ? { ...c, target: toInput(c.target_at) }
      : { title: '', color: PALETTE[0], pinned: 0, remind_before: 1440, target: toInput(toTs(addDays(startOfDay(new Date()), 7))) };
    open = true;
  }

  function save() {
    const { target, ...rest } = form;
    return db.countdowns.save({ ...rest, title: rest.title?.trim(), target_at: fromInput(target)! });
  }
</script>

<div class="page">
  <header class="page-head">
    <h1>倒数日</h1>
    <span class="grow"></span>
    <button class="btn primary" onclick={() => edit()}><Icon name="plus" size={16} /> 新建</button>
  </header>

  <div class="grid">
    {#each sorted as c (c.id)}
      {@const ms = c.target_at * 1000 - clock.now}
      {@const p = split(ms)}
      {@const days = dayDiff(now, fromTs(c.target_at))}
      <button class="cd" class:past={p.past} style="--c: {c.color ?? 'var(--accent)'}" onclick={() => edit(c)}>
        <div class="top">
          <span class="title">{c.title}</span>
          {#if c.pinned}<Icon name="pin" size={14} />{/if}
        </div>
        <div class="days num">
          {#if p.past}<small>已过</small>{/if}{Math.abs(days)}<small>天</small>
        </div>
        {#if !p.past}
          <div class="clock num">{p.days}天 {pad(p.hours)}:{pad(p.minutes)}:{pad(p.seconds)}</div>
        {/if}
        <div class="date">{fmtRelative(c.target_at, now)}</div>
      </button>
    {:else}
      <p class="empty card">添加一个值得期待的日子吧：考试、生日、假期……</p>
    {/each}
  </div>
</div>

<Sheet bind:open title={form.id ? '编辑倒数日' : '新建倒数日'} onsave={save} ondelete={form.id ? () => db.countdowns.remove(form.id!) : undefined}>
  <label class="field">名称<input required bind:value={form.title} placeholder="例如：期末考试" /></label>
  <label class="field">目标时间<input required type="datetime-local" bind:value={form.target} /></label>
  <RemindSelect bind:value={form.remind_before} />
  <div class="field">颜色<ColorPick bind:value={form.color} /></div>
  <label class="check"><input type="checkbox" checked={!!form.pinned} onchange={(e) => (form.pinned = +e.currentTarget.checked)} /> 置顶</label>
</Sheet>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(240px, 1fr));
    gap: 16px;
  }
  .grid .empty {
    grid-column: 1 / -1;
  }
  .cd {
    position: relative;
    overflow: hidden;
    display: grid;
    gap: 4px;
    text-align: left;
    padding: 20px;
    border-radius: var(--radius);
    background: linear-gradient(145deg, color-mix(in srgb, var(--c) 16%, var(--surface)), var(--surface) 70%);
    border: 1px solid color-mix(in srgb, var(--c) 22%, var(--border));
    box-shadow: var(--shadow);
    transition: transform 0.18s var(--ease), box-shadow 0.18s;
  }
  .cd:hover {
    transform: translateY(-3px);
    box-shadow: var(--shadow-lg);
  }
  .cd.past {
    filter: grayscale(0.8);
    opacity: 0.7;
  }
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--text-2);
    font-weight: 600;
    font-size: 14px;
  }
  .days {
    font-size: 52px;
    font-weight: 780;
    line-height: 1.1;
    letter-spacing: -0.03em;
    color: var(--c);
  }
  .days small {
    font-size: 16px;
    font-weight: 600;
    margin: 0 3px;
    letter-spacing: 0;
  }
  .clock {
    font-family: var(--mono);
    font-size: 13px;
    color: var(--text-2);
  }
  .date {
    font-size: 12px;
    color: var(--text-3);
  }
</style>
