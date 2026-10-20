<script lang="ts">
  import ColorPick from '../components/ColorPick.svelte';
  import Icon from '../components/Icon.svelte';
  import RemindSelect from '../components/RemindSelect.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { clock, db, PALETTE, settings } from '../lib/store.svelte';
  import {
    addDays,
    fmtDate,
    fromInput,
    fromTs,
    hhmm,
    inWeeks,
    isoWeekday,
    occursOn,
    parseYmd,
    startOfDay,
    startOfWeek,
    termWeek,
    toInput,
    toTs,
    WEEKDAYS,
    ymd,
  } from '../lib/time';
  import type { CalEvent, Repeat } from '../lib/types';

  const today = $derived(startOfDay(new Date(clock.minute)));
  let cursor = $state(new Date(new Date().getFullYear(), new Date().getMonth(), 1));
  let selected = $state(startOfDay(new Date()));

  const cells = $derived.by(() => {
    const first = startOfWeek(cursor);
    return Array.from({ length: 42 }, (_, i) => {
      const d = addDays(first, i);
      const key = ymd(d);
      return {
        d,
        key,
        events: db.events.items.filter((e) => occursOn(e, d)),
        todos: db.todos.items.filter((t) => !t.done && t.due_at && ymd(fromTs(t.due_at)) === key).length,
      };
    });
  });

  const dayEvents = $derived(
    db.events.items
      .filter((e) => occursOn(e, selected))
      .toSorted((a, b) => b.all_day - a.all_day || hhmm(fromTs(a.start_at)).localeCompare(hhmm(fromTs(b.start_at)))),
  );
  const dayTodos = $derived(db.todos.items.filter((t) => t.due_at && ymd(fromTs(t.due_at)) === ymd(selected)));
  const dayCourses = $derived.by(() => {
    const w = termWeek(settings.term_start, selected);
    if (w == null) return [];
    return db.courses.items
      .filter((c) => c.weekday === isoWeekday(selected) && inWeeks(c.weeks, w))
      .toSorted((a, b) => a.start_time.localeCompare(b.start_time));
  });

  function shift(n: number) {
    cursor = new Date(cursor.getFullYear(), cursor.getMonth() + n, 1);
  }
  function goToday() {
    cursor = new Date(today.getFullYear(), today.getMonth(), 1);
    selected = today;
  }

  const REPEATS: { value: Repeat; label: string }[] = [
    { value: 'none', label: '不重复' },
    { value: 'daily', label: '每天' },
    { value: 'weekly', label: '每周' },
    { value: 'monthly', label: '每月' },
    { value: 'yearly', label: '每年' },
  ];

  let open = $state(false);
  let form = $state({
    id: 0,
    title: '',
    all_day: false,
    start: '',
    end: '',
    repeat: 'none' as Repeat,
    remind_before: 15 as number | null,
    color: PALETTE[0] as string | null,
    note: '',
  });

  function edit(e?: CalEvent) {
    if (e) {
      const allDay = !!e.all_day;
      const conv = (ts: number | null) => (allDay ? (ts ? ymd(fromTs(ts)) : '') : toInput(ts));
      form = { ...e, all_day: allDay, start: conv(e.start_at), end: conv(e.end_at), note: e.note ?? '' };
    } else {
      form = {
        id: 0,
        title: '',
        all_day: false,
        start: `${ymd(selected)}T09:00`,
        end: `${ymd(selected)}T10:00`,
        repeat: 'none',
        remind_before: 15,
        color: PALETTE[0],
        note: '',
      };
    }
    open = true;
  }

  function setAllDay(v: boolean) {
    form.all_day = v;
    const conv = (s: string) => (!s ? '' : v ? s.slice(0, 10) : s.length === 10 ? `${s}T09:00` : s);
    form.start = conv(form.start);
    form.end = conv(form.end);
  }

  function save() {
    const ts = (s: string) => (!s ? null : form.all_day ? toTs(parseYmd(s)) : fromInput(s));
    return db.events.save({
      id: form.id || undefined,
      title: form.title.trim(),
      all_day: +form.all_day,
      start_at: ts(form.start)!,
      end_at: ts(form.end),
      repeat: form.repeat,
      remind_before: form.remind_before,
      color: form.color,
      note: form.note || null,
    });
  }
</script>

<div class="page">
  <header class="page-head">
    <h1 class="num">{cursor.getFullYear()}年{cursor.getMonth() + 1}月</h1>
    <span class="grow"></span>
    <button class="btn icon ghost" aria-label="上个月" onclick={() => shift(-1)}><Icon name="left" /></button>
    <button class="btn sm" onclick={goToday}>今天</button>
    <button class="btn icon ghost" aria-label="下个月" onclick={() => shift(1)}><Icon name="right" /></button>
    <button class="btn primary" onclick={() => edit()}><Icon name="plus" size={16} /> 日程</button>
  </header>

  <div class="layout">
    <div class="card month">
      {#each WEEKDAYS as w}<div class="wd">{w}</div>{/each}
      {#each cells as c (c.key)}
        <button
          class="cell"
          class:other={c.d.getMonth() !== cursor.getMonth()}
          class:today={c.key === ymd(today)}
          class:sel={c.key === ymd(selected)}
          onclick={() => (selected = c.d)}
          ondblclick={() => edit()}
        >
          <span class="n num">{c.d.getDate()}</span>
          <span class="evs">
            {#each c.events.slice(0, 3) as e (e.id)}
              <span class="ev" style="--c: {e.color ?? 'var(--accent)'}">{e.title}</span>
            {/each}
            {#if c.events.length > 3}<span class="more">+{c.events.length - 3}</span>{/if}
          </span>
          {#if c.todos}<span class="todo-dot" title="{c.todos} 项待办到期"></span>{/if}
        </button>
      {/each}
    </div>

    <aside class="card agenda">
      <h2 class="card-title">{fmtDate(selected)}</h2>
      {#each dayEvents as e (e.id)}
        <button class="row tint" style="--c: {e.color ?? 'var(--accent)'}" onclick={() => edit(e)}>
          <span class="t num">{e.all_day ? '全天' : hhmm(fromTs(e.start_at))}</span>
          <span class="grow">{e.title}</span>
          {#if e.remind_before != null}<Icon name="bell" size={13} />{/if}
        </button>
      {/each}
      {#each dayCourses as c (c.id)}
        <div class="row course" style="--c: {c.color ?? 'var(--accent)'}">
          <span class="t num">{c.start_time}</span>
          <span class="grow">{c.name}</span>
          <span class="faint">{c.room ?? ''}</span>
        </div>
      {/each}
      {#each dayTodos as t (t.id)}
        <div class="row" class:done={t.done}>
          <span class="t"><Icon name="todo" size={14} /></span>
          <span class="grow">{t.title}</span>
        </div>
      {/each}
      {#if !dayEvents.length && !dayCourses.length && !dayTodos.length}
        <p class="empty">这一天空空如也<br /><span class="hint">双击日期可快速新建</span></p>
      {/if}
    </aside>
  </div>
</div>

<Sheet bind:open title={form.id ? '编辑日程' : '新建日程'} onsave={save} ondelete={form.id ? () => db.events.remove(form.id) : undefined}>
  <label class="field">标题<input required bind:value={form.title} /></label>
  <label class="check"><input type="checkbox" checked={form.all_day} onchange={(e) => setAllDay(e.currentTarget.checked)} /> 全天</label>
  <div class="row">
    {#if form.all_day}
      <label class="field">开始<input required type="date" bind:value={form.start} /></label>
      <label class="field">结束<input type="date" bind:value={form.end} /></label>
    {:else}
      <label class="field">开始<input required type="datetime-local" bind:value={form.start} /></label>
      <label class="field">结束<input type="datetime-local" bind:value={form.end} /></label>
    {/if}
  </div>
  <div class="row">
    <label class="field">
      重复
      <select bind:value={form.repeat}>
        {#each REPEATS as r}<option value={r.value}>{r.label}</option>{/each}
      </select>
    </label>
    <RemindSelect bind:value={form.remind_before} />
  </div>
  <div class="field">颜色<ColorPick bind:value={form.color} /></div>
  <label class="field">备注<textarea bind:value={form.note}></textarea></label>
</Sheet>

<style>
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 16px;
    align-items: start;
  }
  .month {
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 4px;
    padding: 12px;
  }
  .wd {
    text-align: center;
    font-size: 12px;
    color: var(--text-3);
    padding: 4px 0 8px;
    font-weight: 600;
  }
  .cell {
    position: relative;
    min-height: 92px;
    padding: 6px;
    border-radius: 10px;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 3px;
    text-align: left;
    transition: background 0.12s;
  }
  .cell:hover {
    background: var(--surface-2);
  }
  .cell.other {
    opacity: 0.38;
  }
  .cell.sel {
    background: var(--accent-soft);
  }
  .n {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    font-size: 13px;
    font-weight: 600;
  }
  .today .n {
    background: var(--accent);
    color: var(--on-accent);
  }
  .evs {
    display: grid;
    gap: 2px;
    min-width: 0;
  }
  .ev {
    font-size: 11px;
    line-height: 1.6;
    padding: 0 5px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--c) 18%, transparent);
    color: color-mix(in srgb, var(--c) 75%, var(--text));
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .more {
    font-size: 11px;
    color: var(--text-3);
    padding-left: 4px;
  }
  .todo-dot {
    position: absolute;
    top: 10px;
    right: 8px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
  }

  .agenda {
    position: sticky;
    top: 20px;
  }
  .agenda .row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: 10px;
    margin-bottom: 6px;
    font-size: 14px;
    text-align: left;
  }
  .course {
    background: var(--surface-2);
  }
  .agenda .done {
    color: var(--text-3);
    text-decoration: line-through;
  }
  .t {
    font-size: 12px;
    color: var(--text-2);
    min-width: 36px;
    display: flex;
  }

  @media (max-width: 900px) {
    .layout {
      grid-template-columns: minmax(0, 1fr);
    }
    .agenda {
      position: static;
    }
  }
  @media (max-width: 760px) {
    .month {
      padding: 6px;
      gap: 2px;
    }
    .cell {
      min-height: 54px;
      padding: 3px;
      align-items: center;
    }
    .ev {
      width: 6px;
      height: 6px;
      padding: 0;
      font-size: 0;
      border-radius: 50%;
      background: var(--c);
    }
    .evs {
      display: flex;
      gap: 2px;
    }
    .more {
      display: none;
    }
  }
</style>
