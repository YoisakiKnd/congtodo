<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { api, token } from '../lib/api';
  import { db, loadAll, notify, saveSettings, settings } from '../lib/store.svelte';
  import type { Channel, ChannelKind } from '../lib/types';

  // ---------- 外观 ----------
  type Theme = 'auto' | 'light' | 'dark';
  let theme = $state<Theme>((localStorage.getItem('congtodo.theme') as Theme) || 'auto');
  function setTheme(t: Theme) {
    theme = t;
    try {
      localStorage.setItem('congtodo.theme', t);
    } catch {}
    if (t === 'auto') delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = t;
  }

  // ---------- 令牌 & 通用设置 ----------
  let tokenInput = $state(token.get());
  let general = $state({ term_start: settings.term_start ?? '', digest_time: settings.digest_time ?? '' });
  $effect(() => {
    general = { term_start: settings.term_start ?? '', digest_time: settings.digest_time ?? '' };
  });

  // ---------- 通知渠道 ----------
  type FieldDef = { key: string; label: string; placeholder?: string; required?: boolean };
  const KINDS: Record<ChannelKind, { label: string; desc: string; fields: FieldDef[] }> = {
    ntfy: {
      label: 'ntfy',
      desc: '开源推送，支持自建，全平台 App',
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

  let open = $state(false);
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
    open = true;
  }

  function saveChannel() {
    const config: Record<string, unknown> = Object.fromEntries(Object.entries(form.cfg).filter(([, v]) => v !== ''));
    if (config.priority) config.priority = Number(config.priority);
    if (form.kind === 'webhook' && form.headers.trim()) {
      try {
        config.headers = JSON.parse(form.headers);
      } catch {
        notify('Headers 不是合法 JSON');
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
      notify(`「${c.name}」发送成功`);
    } catch (e) {
      notify(`发送失败：${(e as Error).message}`);
    } finally {
      testing = 0;
    }
  }
</script>

<div class="page narrow">
  <header class="page-head"><h1>设置</h1></header>

  <section class="card">
    <h2 class="card-title">外观</h2>
    <div class="seg">
      <button class:on={theme === 'auto'} onclick={() => setTheme('auto')}>跟随系统</button>
      <button class:on={theme === 'light'} onclick={() => setTheme('light')}><Icon name="sun" size={14} /> 浅色</button>
      <button class:on={theme === 'dark'} onclick={() => setTheme('dark')}><Icon name="moon" size={14} /> 深色</button>
    </div>
  </section>

  <section class="card">
    <h2 class="card-title">学期与每日概览</h2>
    <form class="fields" onsubmit={(e) => (e.preventDefault(), saveSettings(general).then(() => notify('已保存')))}>
      <div class="row">
        <label class="field">开学日期<input type="date" bind:value={general.term_start} /><span class="hint">所在周记为第 1 周</span></label>
        <label class="field">每日概览推送<input type="time" bind:value={general.digest_time} /><span class="hint">留空则不推送</span></label>
      </div>
      <div><button class="btn primary sm">保存</button></div>
    </form>
  </section>

  <section class="card">
    <h2 class="card-title"><Icon name="bell" size={15} /> 通知渠道 <span class="hint">提醒会推送到所有已启用渠道</span></h2>
    {#each db.channels.items as c (c.id)}
      <div class="channel">
        <span class="kind {c.kind}">{KINDS[c.kind]?.label ?? c.kind}</span>
        <button class="grow name" onclick={() => edit(c)}>{c.name}</button>
        <label class="check" title="启用">
          <input type="checkbox" checked={!!c.enabled} onchange={(e) => db.channels.patch(c.id, { enabled: +e.currentTarget.checked })} />
        </label>
        <button class="btn sm" disabled={testing === c.id} onclick={() => test(c)}><Icon name="send" size={14} /> 测试</button>
      </div>
    {:else}
      <p class="empty">还没有通知渠道，选择一种添加：</p>
    {/each}
    <div class="add">
      {#each Object.entries(KINDS) as [k, v]}
        <button class="kind-card" onclick={() => edit(undefined, k as ChannelKind)}>
          <b><Icon name="plus" size={14} /> {v.label}</b>
          <span>{v.desc}</span>
        </button>
      {/each}
    </div>
  </section>

  <section class="card">
    <h2 class="card-title">访问令牌</h2>
    <form class="fields" onsubmit={(e) => (e.preventDefault(), token.set(tokenInput.trim()), loadAll(), notify('已保存'))}>
      <label class="field">
        CONGTODO_TOKEN
        <input type="password" autocomplete="off" bind:value={tokenInput} placeholder="服务端未设置令牌时留空" />
        <span class="hint">仅保存在本浏览器</span>
      </label>
      <div><button class="btn sm">保存</button></div>
    </form>
  </section>
</div>

<Sheet bind:open title={form.id ? `编辑 ${KINDS[form.kind].label}` : `添加 ${KINDS[form.kind].label}`} onsave={saveChannel} ondelete={form.id ? () => db.channels.remove(form.id) : undefined}>
  <label class="field">名称<input required bind:value={form.name} /></label>
  {#each KINDS[form.kind].fields as f (f.key)}
    <label class="field">{f.label}<input required={f.required} placeholder={f.placeholder} bind:value={form.cfg[f.key]} /></label>
  {/each}
  {#if form.kind === 'webhook'}
    <label class="field">
      Headers（JSON，可选）
      <textarea bind:value={form.headers} placeholder={'{"Authorization": "Bearer xxx"}'}></textarea>
    </label>
    <div class="field">
      Body 模板（可选）
      <div class="presets">
        {#each Object.entries(PRESETS) as [name, tpl]}
          <button type="button" class="btn sm" onclick={() => (form.cfg.template = tpl)}>{name}</button>
        {/each}
      </div>
      <textarea bind:value={form.cfg.template} placeholder={'留空发送 {"title","body","kind","time"}'}></textarea>
      <span class="hint">可用变量：{'{{title}}'} {'{{body}}'} {'{{kind}}'}</span>
    </div>
  {/if}
</Sheet>

<style>
  .narrow {
    max-width: 720px;
  }
  section + section {
    margin-top: 16px;
  }
  .card-title .hint {
    margin-left: auto;
  }
  .channel {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .name {
    text-align: left;
    font-weight: 500;
  }
  .kind {
    font-size: 11px;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 6px;
    background: var(--surface-2);
    color: var(--text-2);
    min-width: 64px;
    text-align: center;
  }
  .kind.ntfy {
    background: color-mix(in srgb, #338574 18%, transparent);
    color: #338574;
  }
  .kind.bark {
    background: color-mix(in srgb, #e5484d 15%, transparent);
    color: #e5484d;
  }
  .kind.webhook {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .add {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
    margin-top: 14px;
  }
  .kind-card {
    display: grid;
    gap: 4px;
    text-align: left;
    padding: 12px 14px;
    border-radius: 12px;
    border: 1px dashed var(--border);
    transition: border-color 0.15s, background 0.15s;
  }
  .kind-card:hover {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .kind-card b {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .kind-card span {
    font-size: 12px;
    color: var(--text-3);
  }
  .presets {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
</style>
