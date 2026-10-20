<script lang="ts">
  import { Bell, Plus } from '@lucide/svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import * as Tabs from '$lib/components/ui/tabs';
  import { Textarea } from '$lib/components/ui/textarea';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { clock, db } from '$lib/store.svelte';
  import { addDays, fmtRelative, fromInput, startOfDay, toInput, toTs } from '$lib/time';
  import type { Todo } from '$lib/types';
  import { cn } from '$lib/utils';

  type Filter = 'open' | 'today' | 'done';
  let filter = $state<Filter>('open');
  let quick = $state('');

  const now = $derived(new Date(clock.minute));
  const endOfToday = $derived(toTs(addDays(startOfDay(now), 1)));
  const open = $derived(
    db.todos.items
      .filter((t) => !t.done)
      .toSorted(
        (a, b) =>
          b.priority - a.priority || (a.due_at ?? Infinity) - (b.due_at ?? Infinity) || b.created_at - a.created_at,
      ),
  );
  const dueToday = $derived(open.filter((t) => t.due_at != null && t.due_at < endOfToday));
  const list = $derived(
    filter === 'done'
      ? db.todos.items.filter((t) => t.done).toSorted((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0))
      : filter === 'today'
        ? dueToday
        : open,
  );

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
  let sheet = $state(false);
  let form = $state({ id: 0, title: '', note: '', due: '', remind: '', priority: '0' });

  function edit(t: Todo) {
    form = {
      id: t.id,
      title: t.title,
      note: t.note ?? '',
      due: toInput(t.due_at),
      remind: toInput(t.remind_at),
      priority: String(t.priority),
    };
    sheet = true;
  }

  const PRIORITY = [
    { label: '普通', badge: '' },
    { label: '重要', badge: 'bg-amber-500/15 text-amber-600 dark:text-amber-400' },
    { label: '紧急', badge: 'bg-red-500/15 text-red-600 dark:text-red-400' },
  ];
</script>

<Page title="待办">
  {#snippet actions()}
    <Tabs.Root bind:value={filter}>
      <Tabs.List>
        <Tabs.Trigger value="open">未完成 <span class="num text-muted-foreground">{open.length || ''}</span></Tabs.Trigger>
        <Tabs.Trigger value="today">今天 <span class="num text-muted-foreground">{dueToday.length || ''}</span></Tabs.Trigger>
        <Tabs.Trigger value="done">已完成</Tabs.Trigger>
      </Tabs.List>
    </Tabs.Root>
  {/snippet}

  {#if filter !== 'done'}
    <form class="relative mb-4" onsubmit={addQuick}>
      <Plus class="text-muted-foreground absolute top-1/2 left-3 size-4 -translate-y-1/2" />
      <Input class="h-11 pl-9 text-base md:text-sm" placeholder="添加待办，回车保存" bind:value={quick} />
    </form>
  {/if}

  <Card.Root class="py-2">
    <Card.Content class="px-2">
      {#each list as t (t.id)}
        <div class="hover:bg-muted/60 flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors">
          <Checkbox checked={!!t.done} onCheckedChange={() => toggle(t)} aria-label="切换完成" />
          <button class="min-w-0 flex-1 text-left" onclick={() => edit(t)}>
            <p class={cn(t.done && 'text-muted-foreground line-through')}>{t.title}</p>
            {#if t.note}<p class="text-muted-foreground truncate text-xs">{t.note}</p>{/if}
          </button>
          {#if t.priority && !t.done}
            <Badge variant="secondary" class={PRIORITY[t.priority].badge}>{PRIORITY[t.priority].label}</Badge>
          {/if}
          {#if t.remind_at && !t.done}<Bell class="text-muted-foreground size-3.5" />{/if}
          {#if t.due_at}
            <span
              class={cn(
                'num text-xs',
                !t.done && t.due_at * 1000 < clock.now ? 'text-destructive font-medium' : 'text-muted-foreground',
              )}
            >
              {fmtRelative(t.due_at, now)}
            </span>
          {/if}
        </div>
      {:else}
        <p class="text-muted-foreground py-10 text-center text-sm">
          {filter === 'done' ? '还没有完成的事项' : '全部清空了，休息一下吧 ☕'}
        </p>
      {/each}
    </Card.Content>
  </Card.Root>
</Page>

<Sheet
  bind:open={sheet}
  title="编辑待办"
  onsave={() =>
    db.todos.patch(form.id, {
      title: form.title.trim(),
      note: form.note || null,
      due_at: fromInput(form.due),
      remind_at: fromInput(form.remind),
      priority: Number(form.priority),
    })}
  ondelete={() => db.todos.remove(form.id)}
>
  <Field label="标题"><Input required bind:value={form.title} /></Field>
  <div class="grid grid-cols-2 gap-3">
    <Field label="截止时间"><Input type="datetime-local" bind:value={form.due} /></Field>
    <Field label="提醒时间"><Input type="datetime-local" bind:value={form.remind} /></Field>
  </div>
  <div class="grid gap-2">
    <span class="text-sm font-medium">优先级</span>
    <ToggleGroup.Root type="single" variant="outline" bind:value={form.priority} class="w-full">
      {#each PRIORITY as p, i (i)}
        <ToggleGroup.Item value={String(i)} class="flex-1">{p.label}</ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
  </div>
  <Field label="备注"><Textarea bind:value={form.note} /></Field>
</Sheet>
