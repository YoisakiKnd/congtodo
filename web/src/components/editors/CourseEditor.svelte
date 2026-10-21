<script lang="ts">
  // 课程编辑器：全局挂载，openCourse() 打开
  import { untrack } from 'svelte';
  import ChannelPick from '../ChannelPick.svelte';
  import Choice from '../Choice.svelte';
  import ColorPick from '../ColorPick.svelte';
  import Field from '../Field.svelte';
  import Sheet from '../Sheet.svelte';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { db, PALETTE, settings } from '$lib/store.svelte';
  import { REMIND_OPTIONS, WEEKDAYS } from '$lib/time';
  import { parsePeriods } from '$lib/timetable-import';
  import type { ChannelIds } from '$lib/types';
  import { ui, type CourseTarget } from '$lib/ui.svelte';

  const periods = $derived(parsePeriods(settings.periods));

  let open = $state(false);
  let byPeriod = $state(false);
  let form = $state({
    id: 0,
    name: '',
    teacher: '',
    room: '',
    weekday: 1,
    start_time: '08:00',
    end_time: '09:40',
    weeks: '1-16',
    color: PALETTE[0] as string | null,
    remind_before: 15 as number | null,
    period_from: 1,
    period_to: 2,
    channels: null as ChannelIds,
  });

  function init(t: CourseTarget) {
    const c = t.course;
    form = c
      ? {
          id: c.id,
          name: c.name,
          teacher: c.teacher ?? '',
          room: c.room ?? '',
          weekday: c.weekday,
          start_time: c.start_time,
          end_time: c.end_time,
          weeks: c.weeks,
          color: c.color,
          remind_before: c.remind_before,
          period_from: c.period_from ?? 1,
          period_to: c.period_to ?? 2,
          channels: c.channels,
        }
      : {
          id: 0,
          name: '',
          teacher: '',
          room: '',
          weekday: t.weekday ?? 1,
          start_time: t.start ?? '08:00',
          end_time: t.end ?? '09:40',
          weeks: '1-16',
          color: PALETTE[db.courses.items.length % PALETTE.length],
          remind_before: 15,
          period_from: 1,
          period_to: 2,
          channels: null,
        };
    // 有作息表时默认按节次录入；双击时间格新建的按时间
    byPeriod = periods.length > 0 && (c ? c.period_from != null : !t.start);
  }

  $effect(() => {
    const t = ui.course;
    if (t) untrack(() => (init(t), (open = true)));
  });
  $effect(() => {
    if (!open) untrack(() => (ui.course = null));
  });

  function save() {
    const p = byPeriod && periods.length;
    return db.courses.save({
      id: form.id || undefined,
      name: form.name.trim(),
      teacher: form.teacher || null,
      room: form.room || null,
      weekday: form.weekday,
      start_time: p ? periods[form.period_from - 1].start : form.start_time,
      end_time: p ? periods[form.period_to - 1].end : form.end_time,
      weeks: form.weeks,
      color: form.color,
      remind_before: form.remind_before,
      period_from: p ? form.period_from : null,
      period_to: p ? form.period_to : null,
      channels: form.channels,
    });
  }

  const WEEKDAY_OPTIONS = WEEKDAYS.map((w, i) => ({ value: i + 1, label: `周${w}` }));
  const FROM = $derived(periods.map((p, i) => ({ value: i + 1, label: `第 ${i + 1} 节 ${p.start}` })));
  const TO = $derived(periods.map((p, i) => ({ value: i + 1, label: `第 ${i + 1} 节 ${p.end}` })));
</script>

<Sheet bind:open title={form.id ? '编辑课程' : '添加课程'} onsave={save} ondelete={form.id ? () => db.courses.remove(form.id) : undefined}>
  <Field label="课程名"><Input required bind:value={form.name} /></Field>
  <div class="grid grid-cols-2 gap-3">
    <Field label="教室"><Input bind:value={form.room} /></Field>
    <Field label="老师"><Input bind:value={form.teacher} /></Field>
  </div>
  <div class="grid grid-cols-2 gap-3">
    <Field label="星期"><Choice bind:value={form.weekday} options={WEEKDAY_OPTIONS} /></Field>
    <Field label="周次"><Input required bind:value={form.weeks} placeholder="1-16 或 1-8,10,12-16" /></Field>
  </div>
  {#if periods.length}
    <label class="flex items-center gap-3 text-sm"><Switch bind:checked={byPeriod} />按节次</label>
  {/if}
  {#if byPeriod && periods.length}
    <div class="grid grid-cols-2 gap-3">
      <Field label="从"><Choice bind:value={form.period_from} options={FROM} /></Field>
      <Field label="到"><Choice bind:value={form.period_to} options={TO} /></Field>
    </div>
  {:else}
    <div class="grid grid-cols-2 gap-3">
      <Field label="开始"><Input required type="time" bind:value={form.start_time} /></Field>
      <Field label="结束"><Input required type="time" bind:value={form.end_time} /></Field>
    </div>
  {/if}
  <Field label="上课提醒"><Choice bind:value={form.remind_before} options={REMIND_OPTIONS} /></Field>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
  <ChannelPick bind:value={form.channels} />
</Sheet>
