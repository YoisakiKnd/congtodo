<script lang="ts">
  // 日历订阅：导出（给手机日历订阅）+ 导入外部 ICS
  import { CalendarPlus, Copy, Link, Plus, RefreshCw } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import ColorPick from '../../components/ColorPick.svelte';
  import Field from '../../components/Field.svelte';
  import Sheet from '../../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { api } from '$lib/api';
  import { clock, db, PALETTE } from '$lib/store.svelte';
  import { fmtRelative } from '$lib/time';
  import type { Feed } from '$lib/types';

  // ---------- 导出 ----------
  let token = $state<string | null>(null);
  onMount(() => api.get<{ token: string | null }>('/ics/token').then((r) => (token = r.token)).catch(() => {}));
  const url = $derived(token ? `${location.origin}/api/ics/${token}.ics` : '');
  const webcal = $derived(url.replace(/^https?:/, 'webcal:'));

  async function generate() {
    if (token && !confirm('重新生成后旧的订阅地址会立即失效，已订阅的设备需要重新添加。继续？')) return;
    token = (await api.post<{ token: string }>('/ics/token')).token;
  }
  async function disable() {
    if (!confirm('关闭后订阅地址失效，已订阅的日历将不再更新。')) return;
    await api.del('/ics/token');
    token = null;
  }
  async function copy() {
    await navigator.clipboard.writeText(url);
    toast.success('已复制订阅地址');
  }

  // ---------- 导入 ----------
  let sheet = $state(false);
  let syncing = $state(0);
  let form = $state({ id: 0, name: '', url: '', color: PALETTE[6] as string | null, enabled: true });
  const now = $derived(new Date(clock.minute));

  function edit(f?: Feed) {
    form = f
      ? { id: f.id, name: f.name, url: f.url, color: f.color, enabled: !!f.enabled }
      : { id: 0, name: '', url: '', color: PALETTE[(db.feeds.items.length + 6) % PALETTE.length], enabled: true };
    sheet = true;
  }

  async function sync(id: number) {
    syncing = id;
    try {
      const r = await api.post<{ events: number }>(`/feeds/${id}/sync`);
      toast.success(`同步完成：${r.events} 个事件`);
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      syncing = 0;
      await db.feeds.load();
    }
  }

  async function save() {
    const row = await db.feeds.save({ id: form.id || undefined, name: form.name.trim(), url: form.url.trim(), color: form.color, enabled: +form.enabled });
    if (!form.id && row && 'id' in row) sync(row.id);
  }
</script>

<Card.Root>
  <Card.Header>
    <Card.Title class="flex items-center gap-2"><Link class="size-4" />订阅到手机日历</Card.Title>
    <Card.Description>
      把日程、课程、倒数日和有截止时间的待办同步到 iPhone / Android / Outlook 的系统日历（只读，约每小时更新）
    </Card.Description>
  </Card.Header>
  <Card.Content class="grid gap-3">
    {#if token}
      <div class="flex gap-2">
        <Input readonly value={url} class="font-mono text-xs" onfocus={(e) => e.currentTarget.select()} />
        <Button variant="outline" onclick={copy}><Copy />复制</Button>
      </div>
      <div class="flex flex-wrap gap-2">
        <Button variant="outline" size="sm" href={webcal}><CalendarPlus />在本机日历中打开</Button>
        <Button variant="ghost" size="sm" onclick={generate}>重新生成</Button>
        <Button variant="ghost" size="sm" class="text-destructive" onclick={disable}>关闭订阅</Button>
      </div>
      <span class="text-muted-foreground text-xs">知道这个地址的人都能看到你的日程，请不要公开分享。iPhone：设置 → 日历 → 账户 → 添加已订阅的日历。</span>
    {:else}
      <div><Button size="sm" onclick={generate}>生成订阅地址</Button></div>
    {/if}
  </Card.Content>
</Card.Root>

<Card.Root>
  <Card.Header>
    <Card.Title>订阅外部日历</Card.Title>
    <Card.Description>导入学校校历、节假日、工作日历等 ICS 地址，在日历页只读显示，每 6 小时自动同步</Card.Description>
    <Card.Action><Button variant="outline" size="sm" onclick={() => edit()}><Plus />添加</Button></Card.Action>
  </Card.Header>
  <Card.Content class="grid gap-2">
    {#each db.feeds.items as f (f.id)}
      <div class="flex items-center gap-3 text-sm">
        <span class="size-2.5 shrink-0 rounded-full" style="background: {f.color}"></span>
        <button class="min-w-0 flex-1 text-left" onclick={() => edit(f)}>
          <span class="block truncate font-medium">{f.name}</span>
          <span class="text-muted-foreground block truncate text-xs">
            {#if f.last_error}<span class="text-destructive">同步失败：{f.last_error}</span>
            {:else if f.last_sync}上次同步 {fmtRelative(f.last_sync, now)}
            {:else}等待同步{/if}
          </span>
        </button>
        <Switch checked={!!f.enabled} onCheckedChange={(v) => db.feeds.patch(f.id, { enabled: +v })} aria-label="启用" />
        <Button variant="ghost" size="icon-sm" aria-label="立即同步" disabled={syncing === f.id} onclick={() => sync(f.id)}>
          <RefreshCw class={syncing === f.id ? 'animate-spin' : ''} />
        </Button>
      </div>
    {:else}
      <p class="text-muted-foreground text-sm">还没有订阅</p>
    {/each}
  </Card.Content>
</Card.Root>

<Sheet bind:open={sheet} title={form.id ? '编辑订阅' : '订阅日历'} onsave={save} ondelete={form.id ? () => db.feeds.remove(form.id) : undefined}>
  <Field label="名称"><Input required bind:value={form.name} placeholder="例如：校历" /></Field>
  <Field label="ICS 地址" hint="支持 https:// 和 webcal:// 开头的地址">
    <Input required type="text" bind:value={form.url} placeholder="https://example.com/calendar.ics" />
  </Field>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.enabled} />启用</label>
</Sheet>
