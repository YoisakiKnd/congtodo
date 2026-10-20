<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { clock, db } from '../lib/store.svelte';
  import { addDays, fmtRelative, fromInput, startOfDay, toInput, toTs } from '../lib/time';
  import type { Todo } from '../lib/types';

  type Filter = 'open' | 'today' | 'done';
  let filter = $state<Filter>('open');
  let quick = $state('');

  const now = $derived(new Date(clock.minute));
  const endOfToday = $derived(toTs(addDays(startOfDay(now), 1)));
  const list = $derived.by(() => {
    const items = db.todos.items;
    if (filter === 'done') return items.filter((t) => t.done).toSorted((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0));
    const open = items
      .filter((t) => !t.done)
      .toSorted(
        (a, b) =>
          b.priority - a.priority || (a.due_at ?? Infinity) - (b.due_at ?? Infinity) || b.created_at - a.created_at,
      );
    return filter === 'today' ? open.filter((t) => t.due_at != null && t.due_at < endOfToday) : open;
  });
  const counts = $derived({
    open: db.todos.items.filter((t) => !t.done).length,
    today: db.todos.items.filter((t) => !t.done && t.due_at != null && t.due_at < endOfToday).length,
  });

  async function addQuick(e: SubmitEvent) {
    e.preventDefault();
    const title = quick.trim();
    if (!title) return;
    quick = '';
    await db.todos.add({ title, due_at: filter === 'today' ? endOfToday - 60 : null });
  }

  function toggle(t: Todo) {
    db.todos.patch(t.id, t.done ? { done: 0, done_at: null } : { done: 1, done_at: toTs(new Date()) });
  }

  // 编辑表单：datetime 用字符串绑定，保存时转回时间戳
  let open = $state(false);
  let form = $state({ id: 0, title: '', note: '', due: '', remind: '', priority: 0 });

  function edit(t: Todo) {
    form = {
      id: t.id,
      title: t.title,
      note: t.note ?? '',
      due: toInput(t.due_at),
      remind: toInput(t.remind_at),
      priority: t.priority,
    };
    open = true;
  }

  const PRIORITY = ['普通', '重要', '紧急'];
  const PRIORITY_COLOR = ['var(--text-3)', 'var(--warn)', 'var(--danger)'];
</script>

<div class="page">
  <header class="page-head">
    <h1>待办</h1>
    <span class="grow"></span>
    <div class="seg">
      <button class:on={filter === 'open'} onclick={() => (filter = 'open')}>未完成 {counts.open || ''}</button>
      <button class:on={filter === 'today'} onclick={() => (filter = 'today')}>今天 {counts.today || ''}</button>
      <button class:on={filter === 'done'} onclick={() => (filter = 'done')}>已完成</button>
    </div>
  </header>

  {#if filter !== 'done'}
    <form class="quick card" onsubmit={addQuick}>
      <Icon name="plus" />
      <input class="grow" placeholder="添加待办，回车保存" bind:value={quick} />
    </form>
  {/if}

  <div class="card list">
    {#each list as t (t.id)}
      <div class="item" class:done={t.done}>
        <button class="tick" class:on={t.done} style="--c: {PRIORITY_COLOR[t.priority]}" aria-label="切换完成" onclick={() => toggle(t)}>
          <Icon name="check" size={13} stroke={3} />
        </button>
        <button class="body" onclick={() => edit(t)}>
          <span class="title">{t.title}</span>
          {#if t.note}<span class="note">{t.note}</span>{/if}
        </button>
        <span class="meta">
          {#if t.remind_at && !t.done}<Icon name="bell" size={13} />{/if}
          {#if t.due_at}
            <span class:overdue={!t.done && t.due_at * 1000 < clock.now}>{fmtRelative(t.due_at, now)}</span>
          {/if}
        </span>
      </div>
    {:else}
      <p class="empty">{filter === 'done' ? '还没有完成的事项' : '全部清空了，休息一下吧 ☕'}</p>
    {/each}
  </div>
</div>

<Sheet
  bind:open
  title="编辑待办"
  onsave={() =>
    db.todos.patch(form.id, {
      title: form.title.trim(),
      note: form.note || null,
      due_at: fromInput(form.due),
      remind_at: fromInput(form.remind),
      priority: form.priority,
    })}
  ondelete={() => db.todos.remove(form.id)}
>
  <label class="field">标题<input required bind:value={form.title} /></label>
  <div class="row">
    <label class="field">截止时间<input type="datetime-local" bind:value={form.due} /></label>
    <label class="field">提醒时间<input type="datetime-local" bind:value={form.remind} /></label>
  </div>
  <label class="field">
    优先级
    <div class="seg">
      {#each PRIORITY as p, i}
        <button type="button" class:on={form.priority === i} onclick={() => (form.priority = i)}>{p}</button>
      {/each}
    </div>
  </label>
  <label class="field">备注<textarea bind:value={form.note}></textarea></label>
</Sheet>

<style>
  .quick {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 16px;
    margin-bottom: 14px;
    color: var(--text-3);
  }
  .quick input {
    height: 46px;
    border: 0;
    background: none;
    outline: none;
    font-size: 15px;
    color: var(--text);
  }
  .list {
    padding: 6px 8px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: 12px;
    transition: background 0.12s;
  }
  .item:hover {
    background: var(--surface-2);
  }
  .item + .item {
    margin-top: 2px;
  }
  .body {
    flex: 1;
    min-width: 0;
    display: grid;
    text-align: left;
    padding: 6px 0;
  }
  .title {
    font-size: 15px;
  }
  .note {
    font-size: 12px;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .done .title {
    color: var(--text-3);
    text-decoration: line-through;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-2);
    flex: none;
  }
  .overdue {
    color: var(--danger);
    font-weight: 600;
  }
</style>
