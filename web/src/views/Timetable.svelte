<script lang="ts">
  import { Plus } from '@lucide/svelte';
  import { createViewDay, createViewWeek, type CalendarConfig } from '@schedule-x/calendar';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import ScheduleX from '../components/ScheduleX.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import { courseInstances, toDate, type EventRef, type Range } from '$lib/schedule';
  import { db, PALETTE, saveSettings, settings } from '$lib/store.svelte';
  import { isoWeekday, pad, REMIND_OPTIONS, termWeek, toMinutes, WEEKDAYS } from '$lib/time';
  import type { Course } from '$lib/types';

  let showAll = $state(false);
  let range = $state<Range | null>(null);
  let termInput = $state('');

  const week = $derived(range ? termWeek(settings.term_start, range.start) : null);

  // 纵轴范围和列数随课程自动调整；变化时重建日历
  const layout = $derived.by(() => {
    let lo = 8 * 60;
    let hi = 18 * 60;
    for (const c of db.courses.items) {
      lo = Math.min(lo, Math.floor(toMinutes(c.start_time) / 60) * 60);
      hi = Math.max(hi, Math.ceil(toMinutes(c.end_time) / 60) * 60);
    }
    const nDays = db.courses.items.some((c) => c.weekday >= 6) ? 7 : 5;
    return { lo, hi, nDays, key: `${lo}-${hi}-${nDays}` };
  });

  const hm = (m: number) => `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;

  const config = $derived<CalendarConfig>({
    views: [createViewWeek(), createViewDay()],
    defaultView: 'week',
    dayBoundaries: { start: hm(layout.lo), end: hm(layout.hi) },
    weekOptions: { gridHeight: ((layout.hi - layout.lo) / 60) * 68, nDays: layout.nDays, eventOverlap: false },
    callbacks: {
      onEventClick(ev) {
        const c = db.courses.items.find((x) => x.id === (ev.ref as EventRef).id);
        if (c) edit(c);
      },
      onDoubleClickDateTime(d) {
        const at = toDate(d);
        const m = Math.round((at.getHours() * 60 + at.getMinutes()) / 30) * 30;
        edit(undefined, isoWeekday(at), hm(m), hm(m + 100));
      },
    },
  });

  const build = (r: Range) => courseInstances(db.courses.items, r, settings.term_start, showAll);

  // ---------- 课程表单 ----------

  let sheet = $state(false);
  let form = $state<Partial<Course>>({});

  function edit(c?: Course, weekday = 1, start = '08:00', end = '09:40') {
    form = c
      ? { ...c }
      : {
          name: '',
          teacher: '',
          room: '',
          weekday,
          start_time: start,
          end_time: end,
          weeks: '1-16',
          color: PALETTE[db.courses.items.length % PALETTE.length],
          remind_before: 15,
        };
    sheet = true;
  }

  const WEEKDAY_OPTIONS = WEEKDAYS.map((w, i) => ({ value: i + 1, label: `周${w}` }));
</script>

<Page title="课表" sub={week != null ? `第 ${week} 周` : '未设置开学日期'} class="max-w-7xl">
  {#snippet actions()}
    <label class="flex items-center gap-2 text-sm"><Switch bind:checked={showAll} />显示非本周</label>
    <Button onclick={() => edit(undefined, Math.min(isoWeekday(new Date()), layout.nDays))}><Plus />课程</Button>
  {/snippet}

  {#if !settings.term_start}
    <Card.Root class="bg-primary/5 mb-4">
      <Card.Content>
        <form
          class="flex flex-wrap items-center gap-3 text-sm"
          onsubmit={(e) => (e.preventDefault(), saveSettings({ term_start: termInput }))}
        >
          <span>设置开学日期（第 1 周的任意一天），用于计算周次和课程提醒：</span>
          <Input class="w-auto" type="date" required bind:value={termInput} />
          <Button type="submit" size="sm">保存</Button>
        </form>
      </Card.Content>
    </Card.Root>
  {/if}

  <Card.Root class="p-0">
    {#key layout.key}
      <ScheduleX {config} {build} onrange={(r) => (range = r)} />
    {/key}
  </Card.Root>
  <p class="text-muted-foreground mt-3 text-xs">双击空白时间格可在该位置添加课程</p>
</Page>

<Sheet
  bind:open={sheet}
  title={form.id ? '编辑课程' : '添加课程'}
  onsave={() => db.courses.save({ ...form, name: form.name?.trim() })}
  ondelete={form.id ? () => db.courses.remove(form.id!) : undefined}
>
  <Field label="课程名"><Input required bind:value={form.name} /></Field>
  <div class="grid grid-cols-2 gap-3">
    <Field label="教室"><Input bind:value={form.room} /></Field>
    <Field label="老师"><Input bind:value={form.teacher} /></Field>
  </div>
  <div class="grid grid-cols-2 gap-3">
    <Field label="星期"><Choice bind:value={form.weekday} options={WEEKDAY_OPTIONS} /></Field>
    <Field label="周次"><Input required bind:value={form.weeks} placeholder="1-16 或 1-8,10,12-16" /></Field>
  </div>
  <div class="grid grid-cols-2 gap-3">
    <Field label="开始"><Input required type="time" bind:value={form.start_time} /></Field>
    <Field label="结束"><Input required type="time" bind:value={form.end_time} /></Field>
  </div>
  <Field label="上课提醒"><Choice bind:value={form.remind_before} options={REMIND_OPTIONS} /></Field>
  <div class="grid gap-2"><span class="text-sm font-medium">颜色</span><ColorPick bind:value={form.color} /></div>
</Sheet>
