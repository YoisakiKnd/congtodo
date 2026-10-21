<script lang="ts">
  // 作息时间 + 开学日期：全局挂载，openPeriods() 打开
  import { Plus, Trash2 } from '@lucide/svelte';
  import { untrack } from 'svelte';
  import { toast } from 'svelte-sonner';
  import Field from '../Field.svelte';
  import Sheet from '../Sheet.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { db, saveSettings, settings } from '$lib/store.svelte';
  import { pad, toMinutes } from '$lib/time';
  import { DEFAULT_PERIODS, parsePeriods, type Period } from '$lib/timetable-import';
  import { ui } from '$lib/ui.svelte';

  const hm = (m: number) => `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;

  let open = $state(false);
  let list = $state<Period[]>([]);
  let termStart = $state('');
  let gen = $state({ start: '08:00', length: 45, gap: 10, count: 8 });

  $effect(() => {
    if (ui.periods)
      untrack(() => {
        const cur = parsePeriods(settings.periods);
        list = (cur.length ? cur : DEFAULT_PERIODS).map((p) => ({ ...p }));
        termStart = settings.term_start ?? '';
        open = true;
      });
  });
  $effect(() => {
    if (!open) untrack(() => (ui.periods = false));
  });

  function generate() {
    let m = toMinutes(gen.start);
    list = Array.from({ length: gen.count }, () => {
      const p = { start: hm(m), end: hm(m + gen.length) };
      m += gen.length + gen.gap;
      return p;
    });
  }

  async function save() {
    const periods = list.filter((p) => p.start && p.end);
    await saveSettings({ periods: JSON.stringify(periods), term_start: termStart });
    // 按节次录入的课程跟着新作息更新时间
    const changed = db.courses.items.filter(
      (c) =>
        c.period_from &&
        c.period_to &&
        periods[c.period_to - 1] &&
        (c.start_time !== periods[c.period_from - 1].start || c.end_time !== periods[c.period_to - 1].end),
    );
    await Promise.all(
      changed.map((c) =>
        db.courses.patch(c.id, { start_time: periods[c.period_from! - 1].start, end_time: periods[c.period_to! - 1].end }),
      ),
    );
    if (changed.length) toast.success(`已同步更新 ${changed.length} 门课的时间`);
  }
</script>

<Sheet bind:open title="学期与作息" onsave={save}>
  <Field label="开学日期" hint="所在周记为第 1 周，用于计算周次和单双周">
    <Input type="date" bind:value={termStart} />
  </Field>
  <div class="bg-muted/50 grid gap-3 rounded-lg p-3">
    <span class="text-sm font-medium">按规律生成作息</span>
    <div class="grid grid-cols-4 gap-2">
      <Field label="第一节"><Input type="time" bind:value={gen.start} /></Field>
      <Field label="每节(分)"><Input type="number" min="10" bind:value={gen.length} /></Field>
      <Field label="课间(分)"><Input type="number" min="0" bind:value={gen.gap} /></Field>
      <Field label="节数"><Input type="number" min="1" max="20" bind:value={gen.count} /></Field>
    </div>
    <Button variant="outline" size="sm" class="justify-self-start" onclick={generate}>生成</Button>
  </div>
  <div class="grid max-h-72 gap-2 overflow-y-auto pr-1">
    {#each list as p, i (i)}
      <div class="flex items-center gap-2">
        <span class="text-muted-foreground w-12 shrink-0 text-sm">第 {i + 1} 节</span>
        <Input type="time" bind:value={p.start} />
        <span class="text-muted-foreground">–</span>
        <Input type="time" bind:value={p.end} />
        <button type="button" class="text-muted-foreground hover:text-destructive" aria-label="删除" onclick={() => list.splice(i, 1)}>
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
      const last = list.at(-1);
      const m = last ? toMinutes(last.end) + 10 : 8 * 60;
      list.push({ start: hm(m), end: hm(m + 45) });
    }}
  >
    <Plus />添加一节
  </Button>
  <span class="text-muted-foreground text-xs">中午、晚上的时间可以单独修改。保存后，按节次录入的课程会自动更新时间。</span>
</Sheet>
