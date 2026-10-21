<script lang="ts">
  // 通知渠道（含浏览器推送）+ 推送记录
  import { Bell, BellRing, CircleAlert, CircleCheck, Clock, Plus, RefreshCw, Send } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import Field from '../../components/Field.svelte';
  import Sheet from '../../components/Sheet.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { Textarea } from '$lib/components/ui/textarea';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { api } from '$lib/api';
  import { currentEndpoint, deviceName, isIosBrowser, pushSupported, subscribePush } from '$lib/push';
  import { clock, db } from '$lib/store.svelte';
  import { fmtRelative } from '$lib/time';
  import type { Channel, ChannelKind, NotifyKind, OutboxItem } from '$lib/types';

  type FieldDef = { key: string; label: string; placeholder?: string; required?: boolean };
  type KindDef = { label: string; desc: string; badge: string; fields: FieldDef[] };
  const KINDS: Record<ChannelKind, KindDef> = {
    ntfy: {
      label: 'ntfy',
      desc: '开源推送，支持自建，全平台 App',
      badge: 'bg-teal-500/15 text-teal-700 dark:text-teal-400',
      fields: [
        { key: 'server', label: '服务器', placeholder: 'https://ntfy.sh' },
        { key: 'topic', label: 'Topic', required: true, placeholder: '一个难猜的名字' },
        { key: 'token', label: 'Access Token（可选）' },
        { key: 'priority', label: '优先级 1-5', placeholder: '3' },
      ],
    },
    bark: {
      label: 'Bark',
      desc: 'iOS 推送，支持自建服务器',
      badge: 'bg-red-500/15 text-red-700 dark:text-red-400',
      fields: [
        { key: 'server', label: '服务器', placeholder: 'https://api.day.app' },
        { key: 'key', label: 'Device Key', required: true },
        { key: 'group', label: '分组', placeholder: 'Congtodo' },
        { key: 'sound', label: '铃声', placeholder: 'minuet' },
        { key: 'level', label: '级别', placeholder: 'active / timeSensitive / critical' },
      ],
    },
    webhook: {
      label: 'Webhook',
      desc: '企业微信 / 飞书 / 钉钉 / Discord / Telegram / 自定义',
      badge: 'bg-primary/15 text-primary',
      fields: [
        { key: 'url', label: 'URL', required: true, placeholder: 'https://…' },
        { key: 'method', label: '方法', placeholder: 'POST' },
      ],
    },
    webpush: {
      label: '浏览器',
      desc: '直接推送到这台设备的浏览器 / 已安装的网页应用',
      badge: 'bg-sky-500/15 text-sky-700 dark:text-sky-400',
      fields: [],
    },
  };
  const ADDABLE: ChannelKind[] = ['ntfy', 'bark', 'webhook'];

  const NOTIFY_KINDS: { value: NotifyKind; label: string }[] = [
    { value: 'todo', label: '待办' },
    { value: 'event', label: '日程' },
    { value: 'course', label: '课程' },
    { value: 'habit', label: '习惯' },
    { value: 'countdown', label: '倒数日' },
    { value: 'digest', label: '每日概览' },
    { value: 'weekly', label: '每周回顾' },
  ];

  const PRESETS: Record<string, string> = {
    企业微信: '{"msgtype":"text","text":{"content":"{{title}}\\n{{body}}"}}',
    钉钉: '{"msgtype":"text","text":{"content":"{{title}}\\n{{body}}"}}',
    飞书: '{"msg_type":"text","content":{"text":"{{title}}\\n{{body}}"}}',
    Discord: '{"content":"**{{title}}**\\n{{body}}"}',
    Telegram: '{"chat_id":"<CHAT_ID>","text":"{{title}}\\n{{body}}"}',
  };

  // ---------- 渠道编辑 ----------
  let sheet = $state(false);
  let form = $state({
    id: 0,
    name: '',
    kind: 'ntfy' as ChannelKind,
    enabled: 1,
    cfg: {} as Record<string, string>,
    headers: '',
    kinds: [] as string[],
  });

  function edit(c?: Channel, kind: ChannelKind = 'ntfy') {
    if (c) {
      const { headers, ...rest } = c.config ?? {};
      form = {
        id: c.id,
        name: c.name,
        kind: c.kind,
        enabled: c.enabled,
        cfg: Object.fromEntries(Object.entries(rest).map(([k, v]) => [k, String(v ?? '')])),
        headers: headers ? JSON.stringify(headers, null, 2) : '',
        kinds: c.kinds ?? [],
      };
    } else {
      form = { id: 0, name: KINDS[kind].label, kind, enabled: 1, cfg: {}, headers: '', kinds: [] };
    }
    sheet = true;
  }

  function saveChannel() {
    const config: Record<string, unknown> = Object.fromEntries(Object.entries(form.cfg).filter(([, v]) => v !== ''));
    if (config.priority) config.priority = Number(config.priority);
    if (form.kind === 'webhook' && form.headers.trim()) {
      try {
        config.headers = JSON.parse(form.headers);
      } catch {
        toast.error('Headers 不是合法 JSON');
        throw new Error('invalid headers');
      }
    }
    return db.channels.save({
      id: form.id || undefined,
      name: form.name.trim(),
      kind: form.kind,
      enabled: form.enabled,
      config,
      // 全选等同于「全部类型」，存 null，以后新增的提醒类型也会收到
      kinds: form.kinds.length && form.kinds.length < NOTIFY_KINDS.length ? (form.kinds as NotifyKind[]) : null,
    });
  }

  let testing = $state(0);
  async function test(c: Channel) {
    testing = c.id;
    try {
      await api.post(`/channels/${c.id}/test`);
      toast.success(`「${c.name}」发送成功`);
    } catch (e) {
      toast.error(`发送失败：${(e as Error).message}`);
    } finally {
      testing = 0;
    }
  }

  // ---------- 浏览器推送 ----------
  let endpoint = $state<string | null>(null);
  let subscribing = $state(false);
  onMount(async () => (endpoint = await currentEndpoint()));
  const thisDevice = $derived(db.channels.items.find((c) => c.kind === 'webpush' && c.config?.endpoint === endpoint));

  async function enablePush() {
    subscribing = true;
    try {
      const sub = await subscribePush();
      endpoint = sub.endpoint;
      if (!db.channels.items.some((c) => c.kind === 'webpush' && c.config?.endpoint === sub.endpoint)) {
        const ch = await db.channels.add({ name: deviceName(), kind: 'webpush', config: sub, enabled: 1 });
        await test(ch);
      }
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      subscribing = false;
    }
  }

  // ---------- 推送记录 ----------
  let outbox = $state<OutboxItem[]>([]);
  let showAll = $state(false);
  const loadOutbox = () => api.get<OutboxItem[]>('/outbox').then((r) => (outbox = r)).catch(() => {});
  onMount(loadOutbox);
  const now = $derived(new Date(clock.minute));
  const kindsLabel = (c: Channel) =>
    c.kinds?.length ? c.kinds.map((k) => NOTIFY_KINDS.find((x) => x.value === k)?.label ?? k).join('、') : '全部提醒';
</script>

<Card.Root>
  <Card.Header>
    <Card.Title class="flex items-center gap-2"><Bell class="size-4" />通知渠道</Card.Title>
    <Card.Description>每个渠道可以只接收部分类型的提醒；单个条目也可以在编辑时指定推送渠道</Card.Description>
  </Card.Header>
  <Card.Content class="grid gap-4">
    {#if pushSupported() && !thisDevice}
      <div class="bg-primary/5 flex flex-wrap items-center gap-3 rounded-lg p-3 text-sm">
        <BellRing class="text-primary size-5" />
        <span class="min-w-0 flex-1">在这台设备上直接接收提醒，不需要安装其他 App</span>
        <Button size="sm" disabled={subscribing} onclick={enablePush}>{subscribing ? '正在开启…' : '开启浏览器通知'}</Button>
      </div>
    {:else if isIosBrowser()}
      <p class="text-muted-foreground bg-muted/50 rounded-lg p-3 text-xs">
        iPhone / iPad 需要先在 Safari 中「分享 → 添加到主屏幕」，从主屏幕打开后才能开启浏览器通知（iOS 16.4+）。
      </p>
    {/if}

    {#if db.channels.items.length}
      <div class="divide-y rounded-lg border">
        {#each db.channels.items as c (c.id)}
          <div class="flex items-center gap-3 px-3 py-2.5">
            <Badge variant="secondary" class="w-18 justify-center {KINDS[c.kind]?.badge}">{KINDS[c.kind]?.label ?? c.kind}</Badge>
            <button class="min-w-0 flex-1 text-left" onclick={() => edit(c)}>
              <span class="block truncate font-medium">
                {c.name}{#if c.kind === 'webpush' && c.config?.endpoint === endpoint}<span class="text-muted-foreground ml-1 text-xs font-normal">（这台设备）</span>{/if}
              </span>
              <span class="text-muted-foreground block truncate text-xs">{kindsLabel(c)}</span>
            </button>
            <Switch checked={!!c.enabled} onCheckedChange={(v) => db.channels.patch(c.id, { enabled: +v })} aria-label="启用" />
            <Button variant="outline" size="sm" disabled={testing === c.id} onclick={() => test(c)}><Send />测试</Button>
          </div>
        {/each}
      </div>
    {/if}
    <div class="grid gap-3 sm:grid-cols-3">
      {#each ADDABLE as k (k)}
        <button
          class="hover:border-primary hover:bg-primary/5 grid gap-1 rounded-lg border border-dashed p-3 text-left transition-colors"
          onclick={() => edit(undefined, k)}
        >
          <span class="flex items-center gap-1.5 text-sm font-semibold"><Plus class="size-3.5" />{KINDS[k].label}</span>
          <span class="text-muted-foreground text-xs">{KINDS[k].desc}</span>
        </button>
      {/each}
    </div>
  </Card.Content>
</Card.Root>

<Card.Root>
  <Card.Header>
    <Card.Title>推送记录</Card.Title>
    <Card.Description>最近 100 条；失败会在 1、5、30 分钟后自动重试，免打扰期间的消息会显示「排队中」</Card.Description>
    <Card.Action><Button variant="ghost" size="icon-sm" aria-label="刷新" onclick={loadOutbox}><RefreshCw /></Button></Card.Action>
  </Card.Header>
  <Card.Content class="grid gap-2">
    {#each showAll ? outbox : outbox.slice(0, 8) as o (o.id)}
      <div class="flex items-start gap-3 text-sm">
        {#if o.status === 'sent'}
          <CircleCheck class="mt-0.5 size-4 shrink-0 text-emerald-600" aria-label="已发送" />
        {:else if o.status === 'failed'}
          <CircleAlert class="text-destructive mt-0.5 size-4 shrink-0" aria-label="失败" />
        {:else}
          <Clock class="text-muted-foreground mt-0.5 size-4 shrink-0" aria-label="排队中" />
        {/if}
        <div class="min-w-0 flex-1">
          <p class="truncate">{o.title} · <span class="text-muted-foreground">{o.body.split('\n')[0]}</span></p>
          <p class="text-muted-foreground text-xs">
            {o.channel ?? '已删除的渠道'} ·
            {#if o.status === 'sent'}已发送 {fmtRelative(o.sent_at ?? o.created_at, now)}
            {:else if o.status === 'failed'}失败（{o.attempts} 次）：{o.error}
            {:else}排队中，{fmtRelative(o.next_at, now)}发送{o.attempts ? `（已重试 ${o.attempts} 次：${o.error}）` : ''}{/if}
          </p>
        </div>
      </div>
    {:else}
      <p class="text-muted-foreground text-sm">还没有推送记录</p>
    {/each}
    {#if outbox.length > 8}
      <Button variant="ghost" size="sm" class="justify-self-start" onclick={() => (showAll = !showAll)}>
        {showAll ? '收起' : `查看全部 ${outbox.length} 条`}
      </Button>
    {/if}
  </Card.Content>
</Card.Root>

<Sheet
  bind:open={sheet}
  title={form.id ? `编辑 ${KINDS[form.kind].label}` : `添加 ${KINDS[form.kind].label}`}
  onsave={saveChannel}
  ondelete={form.id ? () => db.channels.remove(form.id) : undefined}
>
  <Field label="名称"><Input required bind:value={form.name} /></Field>
  {#each KINDS[form.kind].fields as f (f.key)}
    <Field label={f.label}><Input required={f.required} placeholder={f.placeholder} bind:value={form.cfg[f.key]} /></Field>
  {/each}
  {#if form.kind === 'webpush'}
    <p class="text-muted-foreground text-xs">浏览器推送的订阅信息由浏览器生成，无需填写。取消订阅或清除浏览器数据后会自动停用。</p>
  {/if}
  {#if form.kind === 'webhook'}
    <Field label="Headers（JSON，可选）">
      <Textarea class="font-mono text-xs" bind:value={form.headers} placeholder={'{"Authorization": "Bearer xxx"}'} />
    </Field>
    <div class="grid gap-2">
      <span class="text-sm font-medium">Body 模板（可选）</span>
      <div class="flex flex-wrap gap-1.5">
        {#each Object.entries(PRESETS) as [name, tpl] (name)}
          <Button variant="secondary" size="xs" onclick={() => (form.cfg.template = tpl)}>{name}</Button>
        {/each}
      </div>
      <Textarea class="font-mono text-xs" bind:value={form.cfg.template} placeholder={'留空发送 {"title","body","kind","time"}'} />
      <span class="text-muted-foreground text-xs">可用变量：{'{{title}}'} {'{{body}}'} {'{{kind}}'}</span>
    </div>
  {/if}
  <div class="grid gap-2">
    <span class="text-sm font-medium">接收哪些提醒</span>
    <ToggleGroup.Root type="multiple" variant="outline" size="sm" spacing={1} class="flex-wrap" bind:value={form.kinds}>
      {#each NOTIFY_KINDS as k (k.value)}
        <ToggleGroup.Item value={k.value}>{k.label}</ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
    <span class="text-muted-foreground text-xs">不选 = 接收全部类型</span>
  </div>
</Sheet>
