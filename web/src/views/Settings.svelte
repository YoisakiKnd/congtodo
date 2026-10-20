<script lang="ts">
  import { Bell, Monitor, Moon, Plus, Send, Sun } from '@lucide/svelte';
  import { setMode, userPrefersMode } from 'mode-watcher';
  import { toast } from 'svelte-sonner';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { Textarea } from '$lib/components/ui/textarea';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { api, token } from '$lib/api';
  import { db, loadAll, saveSettings, settings } from '$lib/store.svelte';
  import type { Channel, ChannelKind } from '$lib/types';

  // ---------- 令牌 & 通用设置 ----------
  let tokenInput = $state(token.get());
  let general = $state({ term_start: '', digest_time: '' });
  $effect(() => {
    general = { term_start: settings.term_start ?? '', digest_time: settings.digest_time ?? '' };
  });

  // ---------- 通知渠道 ----------
  type FieldDef = { key: string; label: string; placeholder?: string; required?: boolean };
  const KINDS: Record<ChannelKind, { label: string; desc: string; badge: string; fields: FieldDef[] }> = {
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
  };

  const PRESETS: Record<string, string> = {
    企业微信: '{"msgtype":"text","text":{"content":"{{title}}\\n{{body}}"}}',
    钉钉: '{"msgtype":"text","text":{"content":"{{title}}\\n{{body}}"}}',
    飞书: '{"msg_type":"text","content":{"text":"{{title}}\\n{{body}}"}}',
    Discord: '{"content":"**{{title}}**\\n{{body}}"}',
    Telegram: '{"chat_id":"<CHAT_ID>","text":"{{title}}\\n{{body}}"}',
  };

  let sheet = $state(false);
  let form = $state({ id: 0, name: '', kind: 'ntfy' as ChannelKind, enabled: 1, cfg: {} as Record<string, string>, headers: '' });

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
      };
    } else {
      form = { id: 0, name: KINDS[kind].label, kind, enabled: 1, cfg: {}, headers: '' };
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
    return db.channels.save({ id: form.id || undefined, name: form.name.trim(), kind: form.kind, enabled: form.enabled, config });
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
</script>

<Page title="设置" class="max-w-3xl">
  <div class="grid gap-4">
    <Card.Root>
      <Card.Header><Card.Title>外观</Card.Title></Card.Header>
      <Card.Content>
        <ToggleGroup.Root
          type="single"
          variant="outline"
          value={userPrefersMode.current}
          onValueChange={(v) => v && setMode(v as 'light' | 'dark' | 'system')}
        >
          <ToggleGroup.Item value="system"><Monitor />跟随系统</ToggleGroup.Item>
          <ToggleGroup.Item value="light"><Sun />浅色</ToggleGroup.Item>
          <ToggleGroup.Item value="dark"><Moon />深色</ToggleGroup.Item>
        </ToggleGroup.Root>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title>学期与每日概览</Card.Title>
        <Card.Description>开学日期所在周记为第 1 周；概览时间留空则不推送</Card.Description>
      </Card.Header>
      <Card.Content>
        <form
          class="grid gap-4"
          onsubmit={(e) => (e.preventDefault(), saveSettings(general).then(() => toast.success('已保存')))}
        >
          <div class="grid grid-cols-2 gap-3">
            <Field label="开学日期"><Input type="date" bind:value={general.term_start} /></Field>
            <Field label="每日概览推送"><Input type="time" bind:value={general.digest_time} /></Field>
          </div>
          <div><Button type="submit" size="sm">保存</Button></div>
        </form>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title class="flex items-center gap-2"><Bell class="size-4" />通知渠道</Card.Title>
        <Card.Description>提醒会推送到所有已启用的渠道</Card.Description>
      </Card.Header>
      <Card.Content class="grid gap-4">
        {#if db.channels.items.length}
          <div class="divide-y rounded-lg border">
            {#each db.channels.items as c (c.id)}
              <div class="flex items-center gap-3 px-3 py-2.5">
                <Badge variant="secondary" class="w-18 justify-center {KINDS[c.kind]?.badge}">{KINDS[c.kind]?.label ?? c.kind}</Badge>
                <button class="min-w-0 flex-1 truncate text-left font-medium" onclick={() => edit(c)}>{c.name}</button>
                <Switch
                  checked={!!c.enabled}
                  onCheckedChange={(v) => db.channels.patch(c.id, { enabled: +v })}
                  aria-label="启用"
                />
                <Button variant="outline" size="sm" disabled={testing === c.id} onclick={() => test(c)}><Send />测试</Button>
              </div>
            {/each}
          </div>
        {/if}
        <div class="grid gap-3 sm:grid-cols-3">
          {#each Object.entries(KINDS) as [k, v] (k)}
            <button
              class="hover:border-primary hover:bg-primary/5 grid gap-1 rounded-lg border border-dashed p-3 text-left transition-colors"
              onclick={() => edit(undefined, k as ChannelKind)}
            >
              <span class="flex items-center gap-1.5 text-sm font-semibold"><Plus class="size-3.5" />{v.label}</span>
              <span class="text-muted-foreground text-xs">{v.desc}</span>
            </button>
          {/each}
        </div>
      </Card.Content>
    </Card.Root>

    <Card.Root>
      <Card.Header>
        <Card.Title>访问令牌</Card.Title>
        <Card.Description>服务端设置了 CONGTODO_TOKEN 时需要填写，仅保存在本浏览器</Card.Description>
      </Card.Header>
      <Card.Content>
        <form
          class="flex gap-2"
          onsubmit={(e) => (e.preventDefault(), token.set(tokenInput.trim()), loadAll(), toast.success('已保存'))}
        >
          <Input type="password" autocomplete="off" bind:value={tokenInput} placeholder="未设置令牌时留空" />
          <Button type="submit" variant="outline">保存</Button>
        </form>
      </Card.Content>
    </Card.Root>
  </div>
</Page>

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
</Sheet>
