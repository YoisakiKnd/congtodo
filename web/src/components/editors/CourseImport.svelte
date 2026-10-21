<script lang="ts">
  // 课表导入：WakeUp 课程表导出文件 / CSV。全局挂载，openImport() 打开
  import { Download } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { toast } from 'svelte-sonner';
  import Sheet from '../Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import * as Tabs from '$lib/components/ui/tabs';
  import { db, PALETTE, saveSettings, settings } from '$lib/store.svelte';
  import { WEEKDAYS } from '$lib/time';
  import { CSV_TEMPLATE, parseCsv, parsePeriods, parseWakeUp, type ImportResult } from '$lib/timetable-import';
  import { ui } from '$lib/ui.svelte';

  let open = $state(false);
  let tab = $state<'wakeup' | 'csv'>('wakeup');
  let parsed = $state<ImportResult | null>(null);
  let opts = $state({ replace: false, applyTerm: true, applyPeriods: true });

  $effect(() => {
    if (ui.importCourses) untrack(() => ((parsed = null), (open = true)));
  });
  $effect(() => {
    if (!open) untrack(() => (ui.importCourses = false));
  });

  async function pickFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      parsed = tab === 'wakeup' ? parseWakeUp(text) : parseCsv(text, parsePeriods(settings.periods));
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
    if (opts.replace) await Promise.all(db.courses.items.map((c) => db.courses.remove(c.id)));
    const patch: Record<string, string> = {};
    if (opts.applyTerm && parsed.termStart) patch.term_start = parsed.termStart;
    if (opts.applyPeriods && parsed.periods) patch.periods = JSON.stringify(parsed.periods);
    if (Object.keys(patch).length) await saveSettings(patch);
    // 同名课程用同一个颜色
    const colors = new Map<string, string>();
    for (const c of parsed.courses) {
      if (!colors.has(c.name)) colors.set(c.name, PALETTE[colors.size % PALETTE.length]);
      await db.courses.add({ ...c, color: colors.get(c.name)!, remind_before: 15 });
    }
    toast.success(`已导入 ${parsed.courses.length} 条课程安排，上课前 15 分钟提醒`);
    parsed = null;
  }

  function downloadTemplate() {
    const url = URL.createObjectURL(new Blob(['﻿' + CSV_TEMPLATE], { type: 'text/csv' }));
    Object.assign(document.createElement('a'), { href: url, download: 'congtodo-课表模板.csv' }).click();
    URL.revokeObjectURL(url);
  }
</script>

<Sheet bind:open title="导入课表" onsave={doImport}>
  <Tabs.Root bind:value={tab} onValueChange={() => (parsed = null)}>
    <Tabs.List class="w-full">
      <Tabs.Trigger value="wakeup" class="flex-1">WakeUp 课程表</Tabs.Trigger>
      <Tabs.Trigger value="csv" class="flex-1">CSV / Excel</Tabs.Trigger>
    </Tabs.List>
  </Tabs.Root>
  {#if tab === 'wakeup'}
    <p class="text-muted-foreground text-xs">
      在 WakeUp 课程表中「导出 → 导出为文件」，得到 .wakeup_schedule 文件后在这里选择。会同时导入作息时间、开学日期和单双周。
    </p>
  {:else}
    <p class="text-muted-foreground text-xs">
      用 Excel 编辑后「另存为 CSV」。列名支持：课程名、星期、开始时间、结束时间、节次（如 3-4，需先设置作息）、周次、教室、老师。
    </p>
    <Button variant="outline" size="sm" class="justify-self-start" onclick={downloadTemplate}><Download />下载模板</Button>
  {/if}
  <Input type="file" accept={tab === 'wakeup' ? '.wakeup_schedule,.txt,.json' : '.csv,.tsv,.txt'} onchange={pickFile} />

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
        <label class="flex items-center gap-2 text-sm"><Checkbox bind:checked={opts.applyTerm} />开学日期设为 {parsed.termStart}</label>
      {/if}
      {#if parsed.periods}
        <label class="flex items-center gap-2 text-sm">
          <Checkbox bind:checked={opts.applyPeriods} />使用文件中的作息时间（{parsed.periods.length} 节）
        </label>
      {/if}
      {#if db.courses.items.length}
        <label class="flex items-center gap-2 text-sm">
          <Checkbox bind:checked={opts.replace} />先清空现有的 {db.courses.items.length} 门课程
        </label>
      {/if}
    </div>
  {/if}
</Sheet>
