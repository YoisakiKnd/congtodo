<script lang="ts">
  import ColorPick from '../components/ColorPick.svelte';
  import Icon from '../components/Icon.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { clock, db, habitLogs, PALETTE, toggleHabit } from '../lib/store.svelte';
  import { addDays, startOfDay, startOfWeek, ymd } from '../lib/time';
  import type { Habit } from '../lib/types';

  const WEEKS = 18;

  const today = $derived(startOfDay(new Date(clock.minute)));
  const todayStr = $derived(ymd(today));
  const habits = $derived(db.habits.items.toSorted((a, b) => a.archived - b.archived || a.id - b.id));

  // 热力图：WEEKS 列 × 7 行，按周一对齐
  const heatDays = $derived.by(() => {
    const start = addDays(startOfWeek(today), -(WEEKS - 1) * 7);
    return Array.from({ length: WEEKS * 7 }, (_, i) => {
      const d = addDays(start, i);
      return { key: ymd(d), future: d > today, label: `${d.getMonth() + 1}/${d.getDate()}` };
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

  let open = $state(false);
  let form = $state<Partial<Habit>>({});

  function edit(h?: Habit) {
    form = h ? { ...h } : { name: '', icon: '', color: PALETTE[(db.habits.items.length * 3) % PALETTE.length], target: 7, remind_time: '21:00', archived: 0 };
    open = true;
  }

  const ICONS = ['🏃', '📖', '💧', '🧘', '💤', '🥗', '✍️', '🎹', '🧠', '🌅', '💪', '🚭'];
</script>

<div class="page">
  <header class="page-head">
    <h1>习惯养成</h1>
    <span class="grow"></span>
    <button class="btn primary" onclick={() => edit()}><Icon name="plus" size={16} /> 新习惯</button>
  </header>

  <div class="list">
    {#each habits as h (h.id)}
      {@const s = stats(h)}
      {@const done = has(h.id, todayStr)}
      <section class="card habit" class:archived={h.archived} style="--c: {h.color ?? 'var(--accent)'}">
        <div class="top">
          <button class="icon" onclick={() => edit(h)} aria-label="编辑">{h.icon || '✨'}</button>
          <div class="grow">
            <h3>{h.name}</h3>
            <p class="meta">
              本周 <b class="num">{s.week}/{h.target}</b>
              {#if s.streak}<span>· 🔥 连续 <b class="num">{s.streak}</b> 天</span>{/if}
              {#if h.remind_time}<span class="faint">· <Icon name="bell" size={11} /> {h.remind_time}</span>{/if}
            </p>
          </div>
          <button class="tick big" class:on={done} style="--c: {h.color ?? 'var(--accent)'}" onclick={() => toggleHabit(h.id, todayStr)} aria-label="今日打卡">
            <Icon name="check" size={20} stroke={3} />
          </button>
        </div>
        <div class="progress"><span style="width: {Math.min(100, (s.week / h.target) * 100)}%"></span></div>
        <div class="heat" style="--weeks: {WEEKS}">
          {#each heatDays as d (d.key)}
            <button
              class="cell"
              class:on={has(h.id, d.key)}
              class:today={d.key === todayStr}
              disabled={d.future}
              title={d.label}
              aria-label={d.label}
              onclick={() => toggleHabit(h.id, d.key)}
            ></button>
          {/each}
        </div>
      </section>
    {:else}
      <p class="empty card">从一个小习惯开始：每天喝 8 杯水、读 10 页书……</p>
    {/each}
  </div>
</div>

<Sheet bind:open title={form.id ? '编辑习惯' : '新习惯'} onsave={() => db.habits.save({ ...form, name: form.name?.trim(), remind_time: form.remind_time || null })} ondelete={form.id ? () => db.habits.remove(form.id!) : undefined}>
  <label class="field">名称<input required bind:value={form.name} placeholder="例如：早起" /></label>
  <div class="field">
    图标
    <div class="icons">
      {#each ICONS as i}
        <button type="button" class:on={form.icon === i} onclick={() => (form.icon = i)}>{i}</button>
      {/each}
      <input class="input" style="width: 64px" bind:value={form.icon} placeholder="自定义" />
    </div>
  </div>
  <div class="row">
    <label class="field">
      每周目标
      <select bind:value={form.target}>
        {#each [1, 2, 3, 4, 5, 6, 7] as n}<option value={n}>{n === 7 ? '每天' : `${n} 次`}</option>{/each}
      </select>
    </label>
    <label class="field">每日提醒<input type="time" bind:value={form.remind_time} /></label>
  </div>
  <div class="field">颜色<ColorPick bind:value={form.color} /></div>
  {#if form.id}
    <label class="check"><input type="checkbox" checked={!!form.archived} onchange={(e) => (form.archived = +e.currentTarget.checked)} /> 归档（停止提醒）</label>
  {/if}
</Sheet>

<style>
  .list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr));
    gap: 16px;
  }
  .list .empty {
    grid-column: 1 / -1;
  }
  .habit.archived {
    opacity: 0.5;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .icon {
    width: 44px;
    height: 44px;
    flex: none;
    border-radius: 14px;
    font-size: 22px;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--c) 16%, var(--surface));
  }
  h3 {
    font-size: 16px;
  }
  .meta {
    font-size: 12px;
    color: var(--text-2);
    display: flex;
    gap: 4px;
    align-items: center;
    flex-wrap: wrap;
  }
  .tick.big {
    width: 44px;
    height: 44px;
    border-width: 2.5px;
  }
  .progress {
    height: 4px;
    border-radius: 2px;
    background: var(--surface-2);
    margin: 14px 0 12px;
    overflow: hidden;
  }
  .progress span {
    display: block;
    height: 100%;
    background: var(--c);
    border-radius: 2px;
    transition: width 0.4s var(--ease);
  }
  .heat {
    display: grid;
    grid-template-rows: repeat(7, 1fr);
    grid-auto-flow: column;
    grid-auto-columns: minmax(0, 1fr);
    gap: 3px;
  }
  .cell {
    aspect-ratio: 1;
    border-radius: 3px;
    background: var(--surface-2);
    transition: background 0.15s;
  }
  .cell.on {
    background: var(--c);
  }
  .cell.today {
    box-shadow: 0 0 0 1.5px var(--text-2);
  }
  .cell:disabled {
    visibility: hidden;
  }
  .icons {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }
  .icons button {
    width: 36px;
    height: 36px;
    border-radius: 10px;
    font-size: 18px;
    background: var(--surface-2);
  }
  .icons button.on {
    box-shadow: 0 0 0 2px var(--accent);
  }
</style>
