<script lang="ts">
  import { Bell, CalendarClock, Inbox, ListTodo, Plus, Repeat, Settings2, Share2, Sun, Trash2, Users, X } from '@lucide/svelte';
  import { toast } from 'svelte-sonner';
  import ChannelPick from '../components/ChannelPick.svelte';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Textarea } from '$lib/components/ui/textarea';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { auth, clock, db, deleteList, lists, PALETTE, removeMember, saveList, shareList } from '$lib/store.svelte';
  import { addDays, fmtRelative, fromInput, startOfDay, toInput, toTs } from '$lib/time';
  import type { ChannelIds, Todo, TodoList, TodoRepeat } from '$lib/types';
  import { cn } from '$lib/utils';

  // 视图：收件箱（不属于任何清单）/ 今天 / 全部 / 某个清单
  type View = 'inbox' | 'today' | 'all' | number;
  let view = $state<View>('inbox');
  let status = $state<'open' | 'done'>('open');
  let quick = $state('');

  const now = $derived(new Date(clock.minute));
  const endOfToday = $derived(toTs(addDays(startOfDay(now), 1)));
  const listById = $derived(new Map(lists.items.map((l) => [l.id, l])));
  const currentList = $derived(typeof view === 'number' ? listById.get(view) : undefined);

  const top = $derived(db.todos.items.filter((t) => !t.parent_id));
  const childrenOf = $derived.by(() => {
    const m = new Map<number, Todo[]>();
    for (const t of db.todos.items) if (t.parent_id) m.set(t.parent_id, [...(m.get(t.parent_id) ?? []), t]);
    return m;
  });

  const inView = (t: Todo, v: View) =>
    v === 'all' ||
    (v === 'inbox' && (!t.list_id || !listById.has(t.list_id))) ||
    (v === 'today' && !t.done && t.due_at != null && t.due_at < endOfToday) ||
    (typeof v === 'number' && t.list_id === v);

  const openCount = (v: View) => top.filter((t) => !t.done && inView(t, v)).length;

  const list = $derived.by(() => {
    const items = top.filter((t) => inView(t, view) && (status === 'done' ? t.done : !t.done));
    return status === 'done'
      ? items.toSorted((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0))
      : items.toSorted(
          (a, b) =>
            b.priority - a.priority || (a.due_at ?? Infinity) - (b.due_at ?? Infinity) || b.created_at - a.created_at,
        );
  });

  async function addQuick(e: SubmitEvent) {
    e.preventDefault();
    const title = quick.trim();
    if (!title) return;
    quick = '';
    await db.todos.add({
      title,
      due_at: view === 'today' ? endOfToday - 60 : null,
      list_id: typeof view === 'number' ? view : null,
    });
  }

  async function toggle(t: Todo) {
    if (t.done) return db.todos.patch(t.id, { done: 0, done_at: null });
    const row = await db.todos.patch(t.id, { done: 1, done_at: toTs(new Date()) });
    if (row._next) {
      // 重复待办：服务端推进到下一次，子任务也已重置
      toast.success(`已完成，下次：${fmtRelative(row.due_at!, new Date())}`);
      await db.todos.load();
    }
  }

  const REPEATS: { value: TodoRepeat; label: string }[] = [
    { value: 'none', label: '不重复' },
    { value: 'daily', label: '每天' },
    { value: 'weekdays', label: '工作日' },
    { value: 'weekly', label: '每周' },
    { value: 'monthly', label: '每月' },
    { value: 'yearly', label: '每年' },
  ];
  const PRIORITY = [
    { label: '普通', badge: '' },
    { label: '重要', badge: 'bg-amber-500/15 text-amber-600 dark:text-amber-400' },
    { label: '紧急', badge: 'bg-red-500/15 text-red-600 dark:text-red-400' },
  ];

  // ---------- 编辑待办 ----------
  let sheet = $state(false);
  let form = $state({
    id: 0,
    title: '',
    note: '',
    due: '',
    remind: '',
    priority: '0',
    repeat: 'none' as TodoRepeat,
    list_id: null as number | null,
    channels: null as ChannelIds,
  });
  let newSub = $state('');
  const subs = $derived(form.id ? (childrenOf.get(form.id) ?? []) : []);
  const LIST_OPTIONS = $derived([
    { value: null as number | null, label: '收件箱' },
    ...lists.items.map((l) => ({ value: l.id as number | null, label: l.shared ? `${l.name}（${l.owner}）` : l.name })),
  ]);

  function edit(t: Todo) {
    form = {
      id: t.id,
      title: t.title,
      note: t.note ?? '',
      due: toInput(t.due_at),
      remind: toInput(t.remind_at),
      priority: String(t.priority),
      repeat: t.repeat,
      list_id: t.list_id,
      channels: t.channels,
    };
    newSub = '';
    sheet = true;
  }

  function saveTodo() {
    const due = fromInput(form.due);
    if (form.repeat !== 'none' && !due) {
      toast.error('重复待办需要设置截止时间');
      throw new Error('repeat needs due');
    }
    return db.todos.patch(form.id, {
      title: form.title.trim(),
      note: form.note || null,
      due_at: due,
      remind_at: fromInput(form.remind),
      priority: Number(form.priority),
      repeat: form.repeat,
      list_id: form.list_id,
      channels: form.channels,
    });
  }

  async function addSub(e: Event) {
    e.preventDefault();
    const title = newSub.trim();
    if (!title) return;
    newSub = '';
    await db.todos.add({ title, parent_id: form.id, list_id: form.list_id });
  }

  // ---------- 清单设置 ----------
  let listSheet = $state(false);
  let listForm = $state({ id: 0, name: '', color: PALETTE[6] as string | null });
  let shareName = $state('');
  const editingList = $derived(listForm.id ? listById.get(listForm.id) : undefined);

  function editList(l?: TodoList) {
    listForm = l ? { id: l.id, name: l.name, color: l.color } : { id: 0, name: '', color: PALETTE[(lists.items.length + 4) % PALETTE.length] };
    shareName = '';
    listSheet = true;
  }

  async function saveListForm() {
    if (editingList?.shared) return;
    const row = await saveList({ id: listForm.id || undefined, name: listForm.name, color: listForm.color });
    view = row.id;
  }

  async function share() {
    const name = shareName.trim();
    if (!name || !listForm.id) return;
    await shareList(listForm.id, name);
    shareName = '';
    toast.success(`已分享给「${name}」`);
  }

  async function leave(l: TodoList) {
    if (!confirm(`退出「${l.name}」？退出后将看不到这个清单。`)) return;
    await removeMember(l.id, auth.user!.id);
    listSheet = false;
    view = 'inbox';
  }

  const NAV: { view: View; label: string; icon: typeof Inbox }[] = [
    { view: 'inbox', label: '收件箱', icon: Inbox },
    { view: 'today', label: '今天', icon: Sun },
    { view: 'all', label: '全部', icon: ListTodo },
  ];
  const title = $derived(
    view === 'inbox' ? '收件箱' : view === 'today' ? '今天' : view === 'all' ? '全部待办' : (currentList?.name ?? '待办'),
  );
</script>

{#snippet navItem(v: View, label: string, Icon: typeof Inbox | null, color?: string | null, shared?: boolean)}
  <button
    class={cn(
      'hover:bg-muted flex h-9 shrink-0 items-center gap-2.5 rounded-md px-3 text-sm whitespace-nowrap transition-colors',
      view === v && 'bg-muted font-medium',
    )}
    onclick={() => (view = v)}
  >
    {#if Icon}<Icon class="text-muted-foreground size-4" />{:else}<span class="size-2.5 rounded-full" style="background: {color ?? 'var(--primary)'}"></span>{/if}
    <span class="flex-1 text-left">{label}</span>
    {#if shared}<Users class="text-muted-foreground size-3.5" />{/if}
    {#if openCount(v)}<span class="num text-muted-foreground text-xs">{openCount(v)}</span>{/if}
  </button>
{/snippet}

<Page {title} sub={currentList?.shared ? `${currentList.owner} 分享的清单` : undefined}>
  {#snippet actions()}
    {#if currentList}
      <Button variant="outline" size="sm" onclick={() => editList(currentList)}>
        {#if currentList.shared}<Users />清单信息{:else}<Settings2 />清单设置{/if}
      </Button>
    {/if}
    <Tabs.Root bind:value={status}>
      <Tabs.List>
        <Tabs.Trigger value="open">未完成</Tabs.Trigger>
        <Tabs.Trigger value="done">已完成</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
  {/snippet}

  <div class="grid gap-4 md:grid-cols-[13rem_minmax(0,1fr)]">
    <!-- 清单导航：桌面端竖排，移动端横向滚动 -->
    <nav class="-mx-4 flex gap-1 overflow-x-auto px-4 pb-1 md:mx-0 md:flex-col md:overflow-visible md:p-0">
      {#each NAV as n (n.view)}{@render navItem(n.view, n.label, n.icon)}{/each}
      <div class="text-muted-foreground mt-3 mb-1 hidden px-3 text-xs font-medium md:block">清单</div>
      {#each lists.items as l (l.id)}{@render navItem(l.id, l.name, null, l.color, l.shared || l.members.length > 0)}{/each}
      <button
        class="text-muted-foreground hover:text-foreground flex h-9 shrink-0 items-center gap-2.5 rounded-md px-3 text-sm whitespace-nowrap"
        onclick={() => editList()}
      >
        <Plus class="size-4" />新建清单
      </button>
    </nav>

    <div class="min-w-0">
      {#if status === 'open'}
        <form class="relative mb-4" onsubmit={addQuick}>
          <Plus class="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
          <Input
            class="h-11 pl-9 text-base md:text-sm"
            placeholder={currentList ? `添加到「${currentList.name}」，回车保存` : '添加待办，回车保存'}
            bind:value={quick}
          />
        </form>
      {/if}

      <Card.Root class="py-2">
        <Card.Content class="px-2">
          {#each list as t (t.id)}
            {@const kids = childrenOf.get(t.id) ?? []}
            {@const doneKids = kids.filter((k) => k.done).length}
            {@const owner = t.user_id !== auth.user?.id}
            <div class="hover:bg-muted/60 flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors">
              <Checkbox checked={!!t.done} onCheckedChange={() => toggle(t)} aria-label="切换完成" />
              <button class="min-w-0 flex-1 text-left" onclick={() => edit(t)}>
                <p class={cn(t.done && 'text-muted-foreground line-through')}>{t.title}</p>
                {#if t.note}<p class="text-muted-foreground truncate text-xs">{t.note}</p>{/if}
              </button>
              {#if view === 'all' || view === 'today'}
                {@const l = t.list_id ? listById.get(t.list_id) : undefined}
                {#if l}<Badge variant="outline" class="hidden sm:inline-flex"><span class="size-1.5 rounded-full" style="background: {l.color}"></span>{l.name}</Badge>{/if}
              {/if}
              {#if kids.length}<span class="num text-muted-foreground text-xs">{doneKids}/{kids.length}</span>{/if}
              {#if owner}<Badge variant="secondary" class="hidden sm:inline-flex">共享</Badge>{/if}
              {#if t.priority && !t.done}<Badge variant="secondary" class={PRIORITY[t.priority].badge}>{PRIORITY[t.priority].label}</Badge>{/if}
              {#if t.repeat !== 'none'}<Repeat class="text-muted-foreground size-3.5" />{/if}
              {#if t.remind_at && !t.done}<Bell class="text-muted-foreground size-3.5" />{/if}
              {#if t.due_at}
                <span class={cn('num text-xs', !t.done && t.due_at * 1000 < clock.now ? 'text-destructive font-medium' : 'text-muted-foreground')}>
                  {fmtRelative(t.due_at, now)}
                </span>
              {/if}
            </div>
            {#if kids.length && status === 'open'}
              <div class="mb-1 ml-10 grid border-l pl-3">
                {#each kids as k (k.id)}
                  <label class="flex items-center gap-2.5 py-1 text-sm">
                    <Checkbox checked={!!k.done} onCheckedChange={() => toggle(k)} />
                    <span class={cn(k.done && 'text-muted-foreground line-through')}>{k.title}</span>
                  </label>
                {/each}
              </div>
            {/if}
          {:else}
            <p class="text-muted-foreground py-10 text-center text-sm">
              {status === 'done' ? '还没有完成的事项' : '全部清空了，休息一下吧 ☕'}
            </p>
          {/each}
        </Card.Content>
      </Card.Root>
    </div>
  </div>
</Page>

<Sheet bind:open={sheet} title="编辑待办" onsave={saveTodo} ondelete={() => db.todos.remove(form.id)}>
  <Field label="标题"><Input required bind:value={form.title} /></Field>
  <div class="grid grid-cols-2 gap-3">
    <Field label="清单"><Choice bind:value={form.list_id} options={LIST_OPTIONS} /></Field>
    <Field label="重复"><Choice bind:value={form.repeat} options={REPEATS} /></Field>
  </div>
  <div class="grid grid-cols-2 gap-3">
    <Field label="截止时间"><Input type="datetime-local" bind:value={form.due} /></Field>
    <Field label="提醒时间"><Input type="datetime-local" bind:value={form.remind} /></Field>
  </div>
  {#if form.repeat !== 'none'}
    <p class="text-muted-foreground -mt-2 flex items-center gap-1.5 text-xs">
      <CalendarClock class="size-3.5" />勾选完成后自动推进到下一次，提醒时间同步顺延，子任务重新开始
    </p>
  {/if}
  <div class="grid gap-2">
    <span class="text-sm font-medium">优先级</span>
    <ToggleGroup.Root type="single" variant="outline" bind:value={form.priority} class="w-full">
      {#each PRIORITY as p, i (i)}
        <ToggleGroup.Item value={String(i)} class="flex-1">{p.label}</ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
  </div>

  <div class="grid gap-2">
    <span class="text-sm font-medium">子任务</span>
    {#each subs as k (k.id)}
      <div class="flex items-center gap-2.5 text-sm">
        <Checkbox checked={!!k.done} onCheckedChange={() => toggle(k)} />
        <span class={cn('flex-1', k.done && 'text-muted-foreground line-through')}>{k.title}</span>
        <button type="button" class="text-muted-foreground hover:text-destructive" aria-label="删除子任务" onclick={() => db.todos.remove(k.id)}>
          <X class="size-4" />
        </button>
      </div>
    {/each}
    <div class="flex gap-2">
      <Input placeholder="添加子任务" bind:value={newSub} onkeydown={(e) => e.key === 'Enter' && addSub(e)} />
      <Button variant="outline" onclick={addSub}><Plus /></Button>
    </div>
  </div>

  <ChannelPick bind:value={form.channels} />
  <Field label="备注"><Textarea bind:value={form.note} /></Field>
</Sheet>

<Sheet
  bind:open={listSheet}
  title={editingList?.shared ? editingList.name : listForm.id ? '清单设置' : '新建清单'}
  onsave={saveListForm}
  ondelete={listForm.id && !editingList?.shared
    ? async () => {
        // 清单里的待办会移回各自创建者的收件箱
        await deleteList(listForm.id);
        view = 'inbox';
      }
    : undefined}
>
  {#if editingList?.shared}
    <p class="text-sm">由 <b>{editingList.owner}</b> 创建并分享给你。你可以查看和编辑其中的待办。</p>
    <p class="text-muted-foreground text-xs">成员：{[editingList.owner, ...editingList.members.map((m) => m.username)].join('、')}</p>
    <Button variant="destructive" onclick={() => leave(editingList)}>退出清单</Button>
  {:else}
    <Field label="名称"><Input required bind:value={listForm.name} placeholder="例如：家庭购物" /></Field>
    <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={listForm.color} /></div>
    {#if editingList}
      <div class="grid gap-2">
        <span class="flex items-center gap-1.5 text-sm font-medium"><Share2 class="size-4" />共享给其他用户</span>
        <div class="flex gap-2">
          <Input placeholder="对方的用户名" bind:value={shareName} onkeydown={(e) => e.key === 'Enter' && (e.preventDefault(), share())} />
          <Button variant="outline" onclick={share}>分享</Button>
        </div>
        {#each editingList.members as m (m.id)}
          <div class="flex items-center gap-2 text-sm">
            <span class="bg-primary/15 text-primary grid size-6 place-items-center rounded-full text-xs font-semibold uppercase">{m.username.slice(0, 1)}</span>
            <span class="flex-1">{m.username}</span>
            <button type="button" class="text-muted-foreground hover:text-destructive" aria-label="移除成员" onclick={() => removeMember(editingList.id, m.id)}>
              <Trash2 class="size-4" />
            </button>
          </div>
        {/each}
        <span class="text-muted-foreground text-xs">成员可以查看和编辑清单里的待办；提醒只发给待办的创建者。</span>
      </div>
    {/if}
  {/if}
</Sheet>
