<script lang="ts">
  import { ImagePlus, Pin, Plus, Repeat, X } from '@lucide/svelte';
  import { toast } from 'svelte-sonner';
  import ChannelPick from '../components/ChannelPick.svelte';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { api } from '$lib/api';
  import { compressImage } from '$lib/image';
  import { clock, db, PALETTE } from '$lib/store.svelte';
  import {
    addDays,
    countdownTarget,
    dayDiff,
    fmtRelative,
    fromInput,
    fromTs,
    pad,
    REMIND_OPTIONS,
    split,
    startOfDay,
    toInput,
    toTs,
  } from '$lib/time';
  import type { ChannelIds, Countdown } from '$lib/types';
  import { cn } from '$lib/utils';

  const now = $derived(new Date(clock.minute));
  const sorted = $derived(
    db.countdowns.items
      .map((c) => {
        const next = countdownTarget(c, now);
        return { c, next, days: dayDiff(now, fromTs(next.ts)) };
      })
      .toSorted((a, b) => {
        const pa = a.days < 0 ? 1 : 0;
        const pb = b.days < 0 ? 1 : 0;
        return pa - pb || b.c.pinned - a.c.pinned || (pa ? b.next.ts - a.next.ts : a.next.ts - b.next.ts);
      }),
  );

  let sheet = $state(false);
  let uploading = $state(false);
  let form = $state({
    id: 0,
    title: '',
    target: '',
    color: PALETTE[0] as string | null,
    pinned: false,
    yearly: false,
    remind_before: 1440 as number | null,
    image_id: null as number | null,
    channels: null as ChannelIds,
  });
  // 编辑时替换 / 移除了图片：保存后删除旧文件
  let replaced: number | null = null;

  function edit(c?: Countdown) {
    replaced = null;
    form = c
      ? {
          id: c.id,
          title: c.title,
          target: toInput(c.target_at),
          color: c.color,
          pinned: !!c.pinned,
          yearly: c.repeat === 'yearly',
          remind_before: c.remind_before,
          image_id: c.image_id,
          channels: c.channels,
        }
      : {
          id: 0,
          title: '',
          target: toInput(toTs(addDays(startOfDay(new Date()), 7))),
          color: PALETTE[0],
          pinned: false,
          yearly: false,
          remind_before: 1440,
          image_id: null,
          channels: null,
        };
    sheet = true;
  }

  async function pickImage(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    uploading = true;
    try {
      const id = await api.upload(await compressImage(file));
      if (form.image_id && replaced == null) replaced = form.image_id;
      form.image_id = id;
    } catch (err) {
      toast.error(`上传失败：${(err as Error).message}`);
    } finally {
      uploading = false;
    }
  }

  function removeImage() {
    if (form.image_id && replaced == null) replaced = form.image_id;
    form.image_id = null;
  }

  async function save() {
    await db.countdowns.save({
      id: form.id || undefined,
      title: form.title.trim(),
      target_at: fromInput(form.target)!,
      color: form.color,
      pinned: +form.pinned,
      repeat: form.yearly ? 'yearly' : 'none',
      remind_before: form.remind_before,
      image_id: form.image_id,
      channels: form.channels,
    });
    if (replaced) api.del(`/files/${replaced}`).catch(() => {});
  }

  async function remove() {
    const img = db.countdowns.items.find((c) => c.id === form.id)?.image_id;
    await db.countdowns.remove(form.id);
    if (img) api.del(`/files/${img}`).catch(() => {});
  }
</script>

<Page title="倒数日">
  {#snippet actions()}
    <Button onclick={() => edit()}><Plus />新建</Button>
  {/snippet}

  <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
    {#each sorted as { c, next, days } (c.id)}
      {@const p = split(next.ts * 1000 - clock.now)}
      {@const past = days < 0}
      {@const img = c.image_id ? `/api/files/${c.image_id}` : null}
      <button
        class={cn(
          'relative grid min-h-44 gap-1 overflow-hidden rounded-xl p-5 text-left shadow-xs ring-1 ring-foreground/10 transition-all hover:-translate-y-0.5 hover:shadow-md',
          past && !img && 'opacity-60 grayscale',
          img && 'text-white',
        )}
        style="--c: {c.color ?? 'var(--primary)'}; background: {img
          ? `linear-gradient(to top, rgb(0 0 0 / 0.65), rgb(0 0 0 / 0.15)), url(${img}) center / cover`
          : 'linear-gradient(145deg, color-mix(in oklch, var(--c) 16%, var(--card)), var(--card) 70%)'}"
        onclick={() => edit(c)}
      >
        <span class={cn('flex items-center justify-between text-sm font-medium', !img && 'text-muted-foreground')}>
          <span class="flex items-center gap-1.5">{c.title}{#if c.repeat === 'yearly'}<Repeat class="size-3.5" />{/if}</span>
          {#if c.pinned}<Pin class="size-3.5" />{/if}
        </span>
        <span class="num text-5xl font-bold tracking-tight" style={img ? '' : 'color: var(--c)'}>
          {#if past}<span class="mr-1 text-base font-semibold">已经</span>{/if}{days === 0 ? '就是今天' : Math.abs(days)}{#if days !== 0}<span
              class="ml-1 text-base font-semibold">天</span
            >{/if}
        </span>
        {#if !past && days > 0}
          <span class={cn('num font-mono text-xs', img ? 'text-white/80' : 'text-muted-foreground')}>
            {p.days}天 {pad(p.hours)}:{pad(p.minutes)}:{pad(p.seconds)}
          </span>
        {/if}
        <span class={cn('mt-auto text-xs', img ? 'text-white/75' : 'text-muted-foreground/70')}>
          {fmtRelative(next.ts, now)}{c.repeat === 'yearly' && next.years ? ` · ${next.years} 周年` : ''}
        </span>
      </button>
    {:else}
      <p class="text-muted-foreground col-span-full rounded-xl border border-dashed py-12 text-center text-sm">
        添加一个值得期待的日子吧：考试、生日、纪念日……
      </p>
    {/each}
  </div>
</Page>

<Sheet bind:open={sheet} title={form.id ? '编辑倒数日' : '新建倒数日'} onsave={save} ondelete={form.id ? remove : undefined}>
  <Field label="名称"><Input required bind:value={form.title} placeholder="例如：期末考试、妈妈生日" /></Field>
  <Field label={form.yearly ? '日期（第一次的日期，如出生日）' : '目标时间'}>
    <Input required type="datetime-local" bind:value={form.target} />
  </Field>
  <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.yearly} />每年重复（生日、纪念日）</label>
  <Field label="提醒"><Choice bind:value={form.remind_before} options={REMIND_OPTIONS} /></Field>
  <div class="grid gap-2">
    <span class="text-sm font-medium">背景图片</span>
    {#if form.image_id}
      <div class="relative h-28 overflow-hidden rounded-lg bg-cover bg-center" style="background-image: url(/api/files/{form.image_id})">
        <button type="button" class="absolute top-2 right-2 rounded-full bg-black/60 p-1 text-white" aria-label="移除图片" onclick={removeImage}>
          <X class="size-4" />
        </button>
      </div>
    {:else}
      <label
        class="text-muted-foreground hover:border-primary hover:text-foreground flex h-20 cursor-pointer items-center justify-center gap-2 rounded-lg border border-dashed text-sm"
      >
        <ImagePlus class="size-4" />{uploading ? '上传中…' : '选择图片'}
        <input type="file" accept="image/*" class="hidden" onchange={pickImage} disabled={uploading} />
      </label>
    {/if}
  </div>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.pinned} />置顶</label>
  <ChannelPick bind:value={form.channels} />
</Sheet>
