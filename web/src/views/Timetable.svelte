<script lang="ts">
  import { Clock, Download, FileUp, Plus, Trash2 } from '@lucide/svelte';
  import { createViewDay, createViewWeek, type CalendarConfig } from '@schedule-x/calendar';
  import { toast } from 'svelte-sonner';
  import ChannelPick from '../components/ChannelPick.svelte';
  import Choice from '../components/Choice.svelte';
  import ColorPick from '../components/ColorPick.svelte';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import ScheduleX from '../components/ScheduleX.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import * as Tabs from '$lib/components/ui/tabs';
  import { createCellRenderer } from '$lib/calendar-cells';
  import { courseInstances, toDate, type EventRef, type Range } from '$lib/schedule';
  import { db, ensureHolidays, holidays, PALETTE, saveSettings, settings } from '$lib/store.svelte';
  import { isoWeekday, pad, REMIND_OPTIONS, termWeek, toMinutes, WEEKDAYS, ymd } from '$lib/time';
  import {
    CSV_TEMPLATE,
    DEFAULT_PERIODS,
    parseCsv,
    parseWakeUp,
    type ImportResult,
    type Period,
  } from '$lib/timetable-import';
  import type { ChannelIds, Course } from '$lib/types';

  let showAll = $state(false);
  let range = $state<Range | null>(null);
  let termInput = $state('');

  const week = $derived(range ? termWeek(settings.term_start, range.start) : null);
  /** 作息时间表（第 n 节的起止时间） */
  const periods = $derived.by<Period[]>(() => {
    try {
      return settings.periods ? JSON.parse(settings.periods) : [];
    } catch {
      return [];
    }
  });
  const skipHoliday = $derived(settings.course_skip_holiday !== '0');

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

  // 周标题上的「休 / 班」角标
  const cells = createCellRenderer();
  $effect(() => {
    void holidays.size;
    cells.repaint();
  });

  function onrange(r: Range) {
    range = r;
    for (let y = r.start.getFullYear(); y <= r.end.getFullYear(); y++) ensureHolidays(y);
  }

  const build = (r: Range) =>
    courseInstances(db.courses.items, r, settings.term_start, showAll, (d) => skipHoliday && !!holidays.get(ymd(d))?.off);

  // ---------- 课程表单 ----------

  let sheet = $state(false);
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

  function edit(c?: Course, weekday = 1, start = '08:00', end = '09:40') {
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
          weekday,
          start_time: start,
          end_time: end,
          weeks: '1-16',
          color: PALETTE[db.courses.items.length % PALETTE.length],
          remind_before: 15,
          period_from: 1,
          period_to: 2,
          channels: null,
        };
    byPeriod = periods.length > 0 && (c ? c.period_from != null : true);
    sheet = true;
  }

  function saveCourse() {
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
  const PERIOD_OPTIONS = $derived(periods.map((p, i) => ({ value: i + 1, label: `第 ${i + 1} 节 ${p.start}` })));
  const PERIOD_END_OPTIONS = $derived(periods.map((p, i) => ({ value: i + 1, label: `第 ${i + 1} 节 ${p.end}` })));

  // ---------- 作息时间 ----------

  let periodSheet = $state(false);
  let periodForm = $state<Period[]>([]);
  let gen = $state({ start: '08:00', length: 45, gap: 10, count: 8 });

  function editPeriods() {
    periodForm = periods.length ? periods.map((p) => ({ ...p })) : DEFAULT_PERIODS.map((p) => ({ ...p }));
    periodSheet = true;
  }

  function generate() {
    let m = toMinutes(gen.start);
    periodForm = Array.from({ length: gen.count }, () => {
      const p = { start: hm(m), end: hm(m + gen.length) };
      m += gen.length + gen.gap;
      return p;
    });
  }

  async function savePeriods() {
    const list = periodForm.filter((p) => p.start && p.end);
    await saveSettings({ periods: JSON.stringify(list) });
    // 按节次录入的课程跟着新作息更新时间
    const changed = db.courses.items.filter(
      (c) =>
        c.period_from &&
        c.period_to &&
        list[c.period_to - 1] &&
        (c.start_time !== list[c.period_from - 1].start || c.end_time !== list[c.period_to - 1].end),
    );
    await Promise.all(
      changed.map((c) => db.courses.patch(c.id, { start_time: list[c.period_from! - 1].start, end_time: list[c.period_to! - 1].end })),
    );
    if (changed.length) toast.success(`已同步更新 ${changed.length} 门课的时间`);
  }

  // ---------- 导入 ----------

  let importSheet = $state(false);
  let importTab = $state<'wakeup' | 'csv'>('wakeup');
  let parsed = $state<ImportResult | null>(null);
  let importOpts = $state({ replace: false, applyTerm: true, applyPeriods: true });

  async function pickFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      parsed = importTab === 'wakeup' ? parseWakeUp(text) : parseCsv(text, periods);
    } catch (err) {
      parsed = null;
      toast.error((err as Error).message);
    }
  }

  async function doImport() {
    if (!parsed?.courses.length) {
      toast.error('请先选择要导入的文件');
      throw new Error('nothing');
    }
    if (importOpts.replace) await Promise.all(db.courses.items.map((c) => db.courses.remove(c.id)));
    const patch: Record<string, string> = {};
    if (importOpts.applyTerm && parsed.termStart) patch.term_start = parsed.termStart;
    if (importOpts.applyPeriods && parsed.periods) patch.periods = JSON.stringify(parsed.periods);
    if (Object.keys(patch).length) await saveSettings(patch);
    // 同名课程用同一个颜色
    const colors = new Map<string, string>();
    for (const c of parsed.courses) {
      if (!colors.has(c.name)) colors.set(c.name, PALETTE[colors.size % PALETTE.length]);
      await db.courses.add({ ...c, color: colors.get(c.name)!, remind_before: 15 });
    }
    toast.success(`已导入 ${parsed.courses.length} 条课程安排`);
    parsed = null;
  }

  function downloadTemplate() {
    const url = URL.createObjectURL(new Blob(['﻿' + CSV_TEMPLATE], { type: 'text/csv' }));
    const a = Object.assign(document.createElement('a'), { href: url, download: 'congtodo-课表模板.csv' });
    a.click();
    URL.revokeObjectURL(url);
  }
</script>

<Page title="课表" sub={week != null ? `第 ${week} 周` : '未设置开学日期'} class="max-w-7xl">
  {#snippet actions()}
    <label class="flex items-center gap-2 text-sm"><Switch bind:checked={showAll} />显示非本周</label>
    <Button variant="outline" onclick={editPeriods}><Clock />作息</Button>
    <Button variant="outline" onclick={() => ((parsed = null), (importSheet = true))}><FileUp />导入</Button>
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
      <ScheduleX {config} {build} {onrange} custom={{ weekGridDate: cells.custom.weekGridDate }} />
    {/key}
  </Card.Root>
  <p class="text-muted-foreground mt-3 text-xs">
    双击空白时间格可在该位置添加课程{skipHoliday ? '；法定假日的课程淡化显示且不提醒（可在设置中关闭）' : ''}
  </p>
</Page>

<Sheet
  bind:open={sheet}
  title={form.id ? '编辑课程' : '添加课程'}
  onsave={saveCourse}
  ondelete={form.id ? () => db.courses.remove(form.id) : undefined}
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
  {#if periods.length}
    <label class="flex items-center gap-3 text-sm"><Switch bind:checked={byPeriod} />按节次</label>
  {/if}
  {#if byPeriod && periods.length}
    <div class="grid grid-cols-2 gap-3">
      <Field label="从"><Choice bind:value={form.period_from} options={PERIOD_OPTIONS} /></Field>
      <Field label="到"><Choice bind:value={form.period_to} options={PERIOD_END_OPTIONS} /></Field>
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

<Sheet bind:open={periodSheet} title="作息时间" onsave={savePeriods}>
  <div class="bg-muted/50 grid gap-3 rounded-lg p-3">
    <span class="text-sm font-medium">按规律生成</span>
    <div class="grid grid-cols-4 gap-2">
      <Field label="第一节"><Input type="time" bind:value={gen.start} /></Field>
      <Field label="每节(分)"><Input type="number" min="10" bind:value={gen.length} /></Field>
      <Field label="课间(分)"><Input type="number" min="0" bind:value={gen.gap} /></Field>
      <Field label="节数"><Input type="number" min="1" max="20" bind:value={gen.count} /></Field>
    </div>
    <Button variant="outline" size="sm" class="justify-self-start" onclick={generate}>生成</Button>
  </div>
  <div class="grid max-h-80 gap-2 overflow-y-auto pr-1">
    {#each periodForm as p, i (i)}
      <div class="flex items-center gap-2">
        <span class="text-muted-foreground w-12 shrink-0 text-sm">第 {i + 1} 节</span>
        <Input type="time" bind:value={p.start} />
        <span class="text-muted-foreground">–</span>
        <Input type="time" bind:value={p.end} />
        <button type="button" class="text-muted-foreground hover:text-destructive" aria-label="删除" onclick={() => periodForm.splice(i, 1)}>
          <Trash2 class="size-4" />
        </button>
      </div>
    {/each}
  </div>
  <Button
    variant="outline"
    size="sm"
    class="justify-self-start"
    onclick={() => {
      const last = periodForm.at(-1);
      const m = last ? toMinutes(last.end) + 10 : 8 * 60;
      periodForm.push({ start: hm(m), end: hm(m + 45) });
    }}
  >
    <Plus />添加一节
  </Button>
  <span class="text-muted-foreground text-xs">中午、晚上的时间可以单独修改。保存后，按节次录入的课程会自动更新时间。</span>
</Sheet>

<Sheet bind:open={importSheet} title="导入课表" onsave={doImport}>
  <Tabs.Root bind:value={importTab} onValueChange={() => (parsed = null)}>
    <Tabs.List class="w-full">
      <Tabs.Trigger value="wakeup" class="flex-1">WakeUp 课程表</Tabs.Trigger>
      <Tabs.Trigger value="csv" class="flex-1">CSV / Excel</Tabs.Trigger>
    </Tabs.List>
  </Tabs.Root>
  {#if importTab === 'wakeup'}
    <p class="text-muted-foreground text-xs">在 WakeUp 课程表中「导出 → 导出为文件」，得到 .wakeup_schedule 文件后在这里选择。会同时导入作息时间和开学日期。</p>
  {:else}
    <p class="text-muted-foreground text-xs">
      用 Excel 编辑后「另存为 CSV」。列名支持：课程名、星期、开始时间、结束时间、节次（如 3-4，需先设置作息）、周次、教室、老师。
    </p>
    <Button variant="outline" size="sm" class="justify-self-start" onclick={downloadTemplate}><Download />下载模板</Button>
  {/if}
  <Input type="file" accept={importTab === 'wakeup' ? '.wakeup_schedule,.txt,.json' : '.csv,.tsv,.txt'} onchange={pickFile} />

  {#if parsed}
    <div class="grid gap-2 rounded-lg border p-3">
      <span class="text-sm font-medium">识别到 {parsed.courses.length} 条课程安排</span>
      <div class="text-muted-foreground grid max-h-40 gap-0.5 overflow-y-auto text-xs">
        {#each parsed.courses as c, i (i)}
          <span>周{WEEKDAYS[c.weekday - 1]} {c.start_time}-{c.end_time} {c.name} · 第 {c.weeks} 周{c.room ? ` · ${c.room}` : ''}</span>
        {/each}
      </div>
      {#each parsed.warnings as w (w)}<span class="text-xs text-amber-600 dark:text-amber-400">{w}</span>{/each}
      {#if parsed.termStart}
        <label class="flex items-center gap-2 text-sm"><Checkbox bind:checked={importOpts.applyTerm} />开学日期设为 {parsed.termStart}</label>
      {/if}
      {#if parsed.periods}
        <label class="flex items-center gap-2 text-sm"><Checkbox bind:checked={importOpts.applyPeriods} />使用文件中的作息时间（{parsed.periods.length} 节）</label>
      {/if}
      <label class="flex items-center gap-2 text-sm">
        <Checkbox bind:checked={importOpts.replace} />先清空现有的 {db.courses.items.length} 门课程
      </label>
    </div>
  {/if}
</Sheet>
