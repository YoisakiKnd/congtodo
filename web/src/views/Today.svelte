<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import { clock, db, habitLogs, settings, toggleHabit } from '../lib/store.svelte';
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
  } from '../lib/time';

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

<div class="page">
  <header class="page-head hero">
    <div>
      <p class="sub">{fmtDate(now)}{week ? ` · 第 ${week} 周` : ''}</p>
      <h1>{greeting}</h1>
    </div>
  </header>

  <div class="grid">
    {#if countdowns.length}
      <section class="card span2 cds">
        {#each countdowns as c (c.id)}
          {@const days = dayDiff(now, fromTs(c.target_at))}
          <a href="#/countdowns" class="cd" style="--c: {c.color ?? 'var(--accent)'}">
            <span class="cd-title">{c.title}</span>
            <span class="cd-days num">{days}<small>天</small></span>
            <span class="faint">{fmtRelative(c.target_at, now, false)}</span>
          </a>
        {/each}
      </section>
    {/if}

    <section class="card">
      <h2 class="card-title"><Icon name="grid" size={15} /> 今日课程 <a href="#/timetable">课表 →</a></h2>
      {#each courses as c (c.id)}
        {@const state = nowMin >= toMinutes(c.end_time) ? 'past' : nowMin >= toMinutes(c.start_time) ? 'now' : ''}
        <div class="line tint {state}" style="--c: {c.color ?? 'var(--accent)'}">
          <span class="time num">{c.start_time}<br /><span class="faint">{c.end_time}</span></span>
          <div class="grow">
            <b>{c.name}</b>
            {#if c.room || c.teacher}<div class="faint small">{[c.room, c.teacher].filter(Boolean).join(' · ')}</div>{/if}
          </div>
          {#if state === 'now'}<span class="badge">进行中</span>{/if}
        </div>
      {:else}
        <p class="empty">今天没有课 🎉</p>
      {/each}
    </section>

    <section class="card">
      <h2 class="card-title"><Icon name="calendar" size={15} /> 今日日程 <a href="#/calendar">日历 →</a></h2>
      {#each events as e (e.id)}
        <div class="line tint" style="--c: {e.color ?? 'var(--accent)'}">
          <span class="time num">{e.all_day ? '全天' : hhmm(fromTs(e.start_at))}</span>
          <b class="grow">{e.title}</b>
        </div>
      {:else}
        <p class="empty">没有日程安排</p>
      {/each}
    </section>

    <section class="card">
      <h2 class="card-title"><Icon name="todo" size={15} /> 今天要做 <a href="#/todos">全部 →</a></h2>
      {#each todos as t (t.id)}
        <div class="todo">
          <button
            class="tick"
            aria-label="完成"
            onclick={() => db.todos.patch(t.id, { done: 1, done_at: toTs(new Date()) })}
          ><Icon name="check" size={13} stroke={3} /></button>
          <span class="grow">{t.title}</span>
          <span class="small" class:overdue={t.due_at! * 1000 < clock.minute}>{fmtRelative(t.due_at!, now)}</span>
        </div>
      {:else}
        <p class="empty">今天的事都做完了</p>
      {/each}
    </section>

    <section class="card">
      <h2 class="card-title"><Icon name="flame" size={15} /> 习惯打卡 <a href="#/habits">详情 →</a></h2>
      <div class="habits">
        {#each habits as h (h.id)}
          {@const on = habitLogs.has(`${h.id}:${todayStr}`)}
          <button class="habit" class:on style="--c: {h.color ?? 'var(--accent)'}" onclick={() => toggleHabit(h.id, todayStr)}>
            <span class="emoji">{h.icon || '✨'}</span>
            <span>{h.name}</span>
          </button>
        {:else}
          <p class="empty">还没有习惯，去添加一个吧</p>
        {/each}
      </div>
    </section>
  </div>
</div>

<style>
  .hero h1 {
    font-size: 30px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }
  .span2 {
    grid-column: span 2;
  }
  .small {
    font-size: 12px;
  }

  .cds {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 12px;
    padding: 12px;
  }
  .cd {
    display: grid;
    gap: 2px;
    padding: 14px 16px;
    border-radius: 14px;
    background: color-mix(in srgb, var(--c) 10%, var(--surface));
    transition: transform 0.15s var(--ease);
  }
  .cd:hover {
    transform: translateY(-2px);
  }
  .cd-title {
    font-size: 13px;
    color: var(--text-2);
    font-weight: 500;
  }
  .cd-days {
    font-size: 34px;
    font-weight: 750;
    line-height: 1.15;
    color: var(--c);
    letter-spacing: -0.02em;
  }
  .cd-days small {
    font-size: 14px;
    margin-left: 3px;
    font-weight: 600;
  }

  .line {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 12px;
    border-radius: 10px;
    margin-bottom: 8px;
    font-size: 14px;
  }
  .line.past {
    opacity: 0.45;
  }
  .line.now {
    box-shadow: 0 0 0 1.5px var(--c);
  }
  .time {
    font-size: 12px;
    color: var(--text-2);
    min-width: 38px;
    line-height: 1.35;
  }
  .badge {
    font-size: 11px;
    font-weight: 600;
    color: var(--c);
  }

  .todo {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 2px;
    font-size: 14px;
    border-bottom: 1px solid var(--border);
  }
  .todo:last-child {
    border-bottom: 0;
  }
  .overdue {
    color: var(--danger);
  }

  .habits {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .habit {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--surface-2);
    font-size: 14px;
    transition:
      background 0.2s var(--ease),
      color 0.2s,
      transform 0.1s;
  }
  .habit:active {
    transform: scale(0.95);
  }
  .habit.on {
    background: var(--c);
    color: #fff;
  }
  .habits .empty {
    width: 100%;
  }

  @media (max-width: 760px) {
    .grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .span2 {
      grid-column: auto;
    }
    .cds {
      grid-template-columns: repeat(3, minmax(0, 1fr));
      gap: 8px;
      padding: 8px;
    }
    .cd {
      padding: 10px 12px;
    }
    .cd-days {
      font-size: 26px;
    }
    .cd-title,
    .cd .faint {
      font-size: 12px;
      white-space: nowrap;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }
</style>
