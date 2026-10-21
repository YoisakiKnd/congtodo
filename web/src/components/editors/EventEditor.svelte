<script lang="ts">
  // 日程编辑器：全局挂载，openEvent() 打开
  import { untrack } from 'svelte';
  import ChannelPick from '../ChannelPick.svelte';
  import Choice from '../Choice.svelte';
  import ColorPick from '../ColorPick.svelte';
  import Field from '../Field.svelte';
  import Sheet from '../Sheet.svelte';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { Textarea } from '$lib/components/ui/textarea';
  import { db, PALETTE } from '$lib/store.svelte';
  import { fromInput, fromTs, hhmm, parseYmd, REMIND_OPTIONS, toInput, toTs, ymd } from '$lib/time';
  import type { ChannelIds, Repeat } from '$lib/types';
  import { ui, type EventTarget } from '$lib/ui.svelte';

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
    channels: null as ChannelIds,
  });

  function init(t: EventTarget) {
    const e = t.event;
    if (e) {
      const allDay = !!e.all_day;
      const conv = (ts: number | null) => (allDay ? (ts ? ymd(fromTs(ts)) : '') : toInput(ts));
      form = {
        id: e.id,
        title: e.title,
        all_day: allDay,
        start: conv(e.start_at),
        end: conv(e.end_at),
        repeat: e.repeat,
        remind_before: e.remind_before,
        color: e.color,
        note: e.note ?? '',
        channels: e.channels,
      };
      return;
    }
    // 新建：只给了日期时默认 9:00，给了时刻就从那个时刻开始
    const at = t.start ?? new Date();
    const start = t.start && !t.allDay ? at : new Date(at.getFullYear(), at.getMonth(), at.getDate(), t.start ? 9 : at.getHours() + 1);
    const end = new Date(start.getTime() + 3600_000);
    form = {
      id: 0,
      title: '',
      all_day: false,
      start: `${ymd(start)}T${hhmm(start)}`,
      end: `${ymd(end)}T${hhmm(end)}`,
      repeat: 'none',
      remind_before: 15,
      color: PALETTE[0],
      note: '',
      channels: null,
    };
  }

  $effect(() => {
    const t = ui.event;
    if (t) untrack(() => (init(t), (open = true)));
  });
  $effect(() => {
    if (!open) untrack(() => (ui.event = null));
  });

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
      channels: form.channels,
    });
  }
</script>

<Sheet bind:open title={form.id ? '编辑日程' : '新建日程'} onsave={save} ondelete={form.id ? () => db.events.remove(form.id) : undefined}>
  <Field label="标题"><Input required bind:value={form.title} /></Field>
  <label class="flex items-center gap-3 text-sm">
    <Switch checked={form.all_day} onCheckedChange={setAllDay} />全天
  </label>
  <div class="grid grid-cols-2 gap-3">
    {#if form.all_day}
      <Field label="开始"><Input required type="date" bind:value={form.start} /></Field>
      <Field label="结束"><Input type="date" bind:value={form.end} /></Field>
    {:else}
      <Field label="开始"><Input required type="datetime-local" bind:value={form.start} /></Field>
      <Field label="结束"><Input type="datetime-local" bind:value={form.end} /></Field>
    {/if}
  </div>
  <div class="grid grid-cols-2 gap-3">
    <Field label="重复"><Choice bind:value={form.repeat} options={REPEATS} /></Field>
    <Field label="提醒"><Choice bind:value={form.remind_before} options={REMIND_OPTIONS} /></Field>
  </div>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <ChannelPick bind:value={form.channels} />
  <Field label="备注"><Textarea bind:value={form.note} /></Field>
</Sheet>
