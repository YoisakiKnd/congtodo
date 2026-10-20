<script lang="ts">
  import ColorPick from '../components/ColorPick.svelte';
  import Icon from '../components/Icon.svelte';
  import RemindSelect from '../components/RemindSelect.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { clock, db, PALETTE, saveSettings, settings } from '../lib/store.svelte';
  import { addDays, inWeeks, pad, startOfWeek, termWeek, toMinutes, WEEKDAYS, ymd } from '../lib/time';
  import type { Course } from '../lib/types';

  const PX_PER_MIN = 1.15;

  const now = $derived(new Date(clock.minute));
  let offset = $state(0);
  let showAll = $state(false);
  let termInput = $state(settings.term_start ?? '');

  const weekStart = $derived(addDays(startOfWeek(now), offset * 7));
  const week = $derived(termWeek(settings.term_start, weekStart));
  const hasWeekend = $derived(db.courses.items.some((c) => c.weekday >= 6));
  const days = $derived(Array.from({ length: hasWeekend ? 7 : 5 }, (_, i) => addDays(weekStart, i)));

  const visible = $derived(
    db.courses.items
      .map((c) => ({ c, active: week == null || inWeeks(c.weeks, week) }))
      .filter((x) => x.active || showAll),
  );

  // 纵轴范围：根据课程自动收缩，默认 08:00–18:00
  const range = $derived.by(() => {
    let lo = 8 * 60;
    let hi = 18 * 60;
    for (const c of db.courses.items) {
      lo = Math.min(lo, Math.floor(toMinutes(c.start_time) / 60) * 60);
      hi = Math.max(hi, Math.ceil(toMinutes(c.end_time) / 60) * 60);
    }
    return { lo, hi };
  });
  const hours = $derived(Array.from({ length: (range.hi - range.lo) / 60 + 1 }, (_, i) => range.lo / 60 + i));
  const nowMin = $derived(now.getHours() * 60 + now.getMinutes());
  const todayIdx = $derived(days.findIndex((d) => ymd(d) === ymd(now)));

  let open = $state(false);
  let form = $state<Partial<Course>>({});

  function edit(c?: Course, weekday = 1) {
    form = c
      ? { ...c }
      : { name: '', teacher: '', room: '', weekday, start_time: '08:00', end_time: '09:40', weeks: '1-16', color: PALETTE[db.courses.items.length % PALETTE.length], remind_before: 15 };
    open = true;
  }

  // 点击空白处：按点击位置预填星期和时间
  function clickColumn(e: MouseEvent, weekday: number) {
    if (e.target !== e.currentTarget) return;
    const y = e.offsetY / PX_PER_MIN + range.lo;
    const m = Math.round(y / 30) * 30;
    edit(undefined, weekday);
    form.start_time = `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;
    form.end_time = `${pad(Math.floor((m + 100) / 60))}:${pad((m + 100) % 60)}`;
  }
</script>

<div class="page">
  <header class="page-head">
    <div>
      <h1>课表</h1>
      <p class="sub">{week != null ? `第 ${week} 周` : '未设置开学日期'} · {weekStart.getMonth() + 1}/{weekStart.getDate()} 起</p>
    </div>
    <span class="grow"></span>
    <label class="check"><input type="checkbox" bind:checked={showAll} /> 显示非本周</label>
    <button class="btn icon ghost" aria-label="上一周" onclick={() => offset--}><Icon name="left" /></button>
    <button class="btn sm" onclick={() => (offset = 0)}>本周</button>
    <button class="btn icon ghost" aria-label="下一周" onclick={() => offset++}><Icon name="right" /></button>
    <button class="btn primary" onclick={() => edit()}><Icon name="plus" size={16} /> 课程</button>
  </header>

  {#if !settings.term_start}
    <form class="card banner" onsubmit={(e) => (e.preventDefault(), saveSettings({ term_start: termInput }))}>
      <span>设置开学日期（第 1 周的任意一天），用于计算周次和课程提醒：</span>
      <input class="input" type="date" required bind:value={termInput} />
      <button class="btn primary sm">保存</button>
    </form>
  {/if}

  <div class="card table" style="--cols: {days.length}; --h: {(range.hi - range.lo) * PX_PER_MIN}px">
    <div class="corner"></div>
    {#each days as d, i}
      <div class="head" class:today={i === todayIdx}>
        <span>周{WEEKDAYS[i]}</span>
        <span class="num faint">{d.getMonth() + 1}/{d.getDate()}</span>
      </div>
    {/each}

    <div class="axis">
      {#each hours as h}
        <span class="num" style="top: {(h * 60 - range.lo) * PX_PER_MIN}px">{pad(h)}:00</span>
      {/each}
    </div>
    {#each days as _, i}
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="col" class:today={i === todayIdx} onclick={(e) => clickColumn(e, i + 1)}>
        {#each visible.filter((x) => x.c.weekday === i + 1) as { c, active } (c.id)}
          {@const top = (toMinutes(c.start_time) - range.lo) * PX_PER_MIN}
          {@const height = (toMinutes(c.end_time) - toMinutes(c.start_time)) * PX_PER_MIN}
          <button class="course" class:inactive={!active} style="--c: {c.color ?? 'var(--accent)'}; top: {top}px; height: {height}px" onclick={() => edit(c)}>
            <b>{c.name}</b>
            {#if c.room}<span>@{c.room}</span>{/if}
            <span class="faint num">{c.start_time}–{c.end_time}</span>
          </button>
        {/each}
        {#if i === todayIdx && nowMin >= range.lo && nowMin <= range.hi}
          <div class="now" style="top: {(nowMin - range.lo) * PX_PER_MIN}px"></div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<Sheet bind:open title={form.id ? '编辑课程' : '添加课程'} onsave={() => db.courses.save({ ...form, name: form.name?.trim() })} ondelete={form.id ? () => db.courses.remove(form.id!) : undefined}>
  <label class="field">课程名<input required bind:value={form.name} /></label>
  <div class="row">
    <label class="field">教室<input bind:value={form.room} /></label>
    <label class="field">老师<input bind:value={form.teacher} /></label>
  </div>
  <div class="row">
    <label class="field">
      星期
      <select bind:value={form.weekday}>
        {#each WEEKDAYS as w, i}<option value={i + 1}>周{w}</option>{/each}
      </select>
    </label>
    <label class="field">周次<input required bind:value={form.weeks} placeholder="1-16 或 1-8,10,12-16" /></label>
  </div>
  <div class="row">
    <label class="field">开始<input required type="time" bind:value={form.start_time} /></label>
    <label class="field">结束<input required type="time" bind:value={form.end_time} /></label>
  </div>
  <RemindSelect bind:value={form.remind_before} label="上课提醒" />
  <div class="field">颜色<ColorPick bind:value={form.color} /></div>
</Sheet>

<style>
  .banner {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-bottom: 16px;
    font-size: 14px;
    background: var(--accent-soft);
    border-color: transparent;
  }
  .banner .input {
    width: auto;
  }
  .table {
    display: grid;
    grid-template-columns: 44px repeat(var(--cols), minmax(0, 1fr));
    grid-template-rows: auto var(--h);
    padding: 12px 12px 18px 4px;
    column-gap: 4px;
  }
  .head {
    display: grid;
    justify-items: center;
    font-size: 13px;
    font-weight: 600;
    padding-bottom: 10px;
  }
  .head.today {
    color: var(--accent);
  }
  .axis {
    position: relative;
  }
  .axis span {
    position: absolute;
    right: 6px;
    transform: translateY(-50%);
    font-size: 11px;
    color: var(--text-3);
  }
  .col {
    position: relative;
    border-radius: 10px;
    background: repeating-linear-gradient(
      to bottom,
      var(--border) 0 1px,
      transparent 1px calc(60px * 1.15)
    );
    cursor: copy;
  }
  .col.today {
    background-color: color-mix(in srgb, var(--accent) 5%, transparent);
  }
  .course {
    position: absolute;
    left: 2px;
    right: 2px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 6px 8px;
    border-radius: 9px;
    overflow: hidden;
    text-align: left;
    font-size: 12px;
    line-height: 1.35;
    background: color-mix(in srgb, var(--c) 18%, var(--surface));
    border-left: 3px solid var(--c);
    color: color-mix(in srgb, var(--c) 40%, var(--text));
    cursor: pointer;
    transition: transform 0.12s var(--ease), box-shadow 0.12s;
  }
  .course:hover {
    transform: scale(1.02);
    box-shadow: var(--shadow-lg);
    z-index: 2;
  }
  .course b {
    font-size: 13px;
    color: var(--text);
  }
  .course.inactive {
    opacity: 0.35;
    filter: grayscale(1);
  }
  .now {
    position: absolute;
    left: -4px;
    right: 0;
    height: 2px;
    background: var(--danger);
    pointer-events: none;
    z-index: 3;
  }
  .now::before {
    content: '';
    position: absolute;
    left: 0;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
  }
  @media (max-width: 760px) {
    .table {
      grid-template-columns: 34px repeat(var(--cols), minmax(0, 1fr));
      padding: 8px 4px 12px 0;
      column-gap: 2px;
    }
    .course {
      padding: 4px;
      font-size: 10px;
    }
    .course b {
      font-size: 11px;
    }
    .axis span {
      font-size: 9px;
      right: 3px;
    }
  }
</style>
