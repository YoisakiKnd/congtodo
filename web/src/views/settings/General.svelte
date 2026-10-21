<script lang="ts">
  // 外观 + 时区 + 提醒相关的个人设置
  import { Monitor, Moon, Sun } from '@lucide/svelte';
  import { setMode, userPrefersMode } from 'mode-watcher';
  import { toast } from 'svelte-sonner';
  import Choice from '../../components/Choice.svelte';
  import Field from '../../components/Field.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { browserTimezone, saveSettings, settings } from '$lib/store.svelte';
  import { WEEKDAYS } from '$lib/time';

  const ZONES: string[] = (Intl as any).supportedValuesOf?.('timeZone') ?? [];

  let form = $state(read());
  function read() {
    return {
      timezone: settings.timezone ?? browserTimezone(),
      term_start: settings.term_start ?? '',
      digest_time: settings.digest_time ?? '',
      weekly_day: settings.weekly_day ?? '',
      weekly_time: settings.weekly_time ?? '20:00',
      quiet: !!settings.quiet_start,
      quiet_start: settings.quiet_start || '23:00',
      quiet_end: settings.quiet_end || '07:00',
      skip_holiday: settings.course_skip_holiday !== '0',
    };
  }
  // 设置从服务端加载完成后同步到表单
  $effect(() => {
    void settings.timezone;
    form = read();
  });

  const WEEKLY = [{ value: '', label: '不推送' }, ...WEEKDAYS.map((w, i) => ({ value: String(i + 1), label: `每周${w}` }))];

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (form.timezone && ZONES.length && !ZONES.includes(form.timezone) && form.timezone !== 'UTC') {
      return toast.error('请选择列表中的时区');
    }
    await saveSettings({
      timezone: form.timezone,
      term_start: form.term_start,
      digest_time: form.digest_time,
      weekly_day: form.weekly_day,
      weekly_time: form.weekly_day ? form.weekly_time : '',
      quiet_start: form.quiet ? form.quiet_start : '',
      quiet_end: form.quiet ? form.quiet_end : '',
      course_skip_holiday: form.skip_holiday ? '1' : '0',
    });
    toast.success('已保存');
  }

  const mismatch = $derived(settings.timezone && settings.timezone !== browserTimezone());
</script>

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
    <Card.Title>时间与提醒</Card.Title>
    <Card.Description>服务器按这里的时区计算课程、习惯等「几点几分」的提醒</Card.Description>
  </Card.Header>
  <Card.Content>
    <form class="grid gap-4" onsubmit={save}>
      <div class="grid gap-3 sm:grid-cols-2">
        <Field label="时区">
          <Input list="tz-list" bind:value={form.timezone} placeholder="Asia/Shanghai" />
          <datalist id="tz-list">{#each ZONES as z (z)}<option value={z}></option>{/each}</datalist>
        </Field>
        <Field label="开学日期" hint="所在周记为第 1 周"><Input type="date" bind:value={form.term_start} /></Field>
      </div>
      {#if mismatch}
        <p class="-mt-2 text-xs text-amber-600 dark:text-amber-400">
          当前设备的时区是 {browserTimezone()}，和账号时区不同。
          <button type="button" class="underline" onclick={() => (form.timezone = browserTimezone())}>改为当前时区</button>
        </p>
      {/if}

      <div class="grid gap-3 sm:grid-cols-3">
        <Field label="每日概览" hint="留空则不推送"><Input type="time" bind:value={form.digest_time} /></Field>
        <Field label="每周回顾"><Choice bind:value={form.weekly_day} options={WEEKLY} /></Field>
        {#if form.weekly_day}
          <Field label="回顾时间"><Input type="time" bind:value={form.weekly_time} /></Field>
        {/if}
      </div>

      <div class="grid gap-2">
        <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.quiet} />免打扰时段</label>
        {#if form.quiet}
          <div class="flex items-center gap-2">
            <Input type="time" class="w-32" bind:value={form.quiet_start} />
            <span class="text-muted-foreground">至</span>
            <Input type="time" class="w-32" bind:value={form.quiet_end} />
          </div>
          <span class="text-muted-foreground text-xs">这段时间内的提醒会推迟到结束时再发送；可以跨午夜（如 23:00 至 07:00）</span>
        {/if}
      </div>

      <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.skip_holiday} />法定节假日不提醒上课</label>
      <div><Button type="submit" size="sm">保存</Button></div>
    </form>
  </Card.Content>
</Card.Root>
