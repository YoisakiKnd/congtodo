<script lang="ts">
  // 数据导出 / 导入（JSON）
  import { Download, Upload } from '@lucide/svelte';
  import { toast } from 'svelte-sonner';
  import { Button, buttonVariants } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { api } from '$lib/api';
  import { loadAll } from '$lib/store.svelte';

  let importing = $state(false);

  const LABELS: Record<string, string> = {
    todos: '待办',
    todo_lists: '清单',
    events: '日程',
    courses: '课程',
    habits: '习惯',
    habit_logs: '打卡',
    countdowns: '倒数日',
    channels: '渠道',
    feeds: '订阅',
    focus_sessions: '专注',
  };

  async function importFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = '';
    if (!file) return;
    let data: unknown;
    try {
      data = JSON.parse(await file.text());
    } catch {
      return toast.error('文件不是有效的 JSON');
    }
    if (!confirm('导入会把文件中的数据追加到当前账号（不会覆盖或删除现有数据）。继续？')) return;
    importing = true;
    try {
      const r = await api.post<{ imported: Record<string, number> }>('/import', data);
      const summary = Object.entries(r.imported)
        .filter(([, n]) => n)
        .map(([k, n]) => `${LABELS[k] ?? k} ${n}`)
        .join('、');
      toast.success(summary ? `已导入：${summary}` : '文件里没有可导入的数据');
      await loadAll();
    } catch (err) {
      toast.error(`导入失败：${(err as Error).message}`);
    } finally {
      importing = false;
    }
  }
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>数据</Card.Title>
    <Card.Description>导出当前账号的全部数据为 JSON（不含倒数日图片），可用于备份或迁移到另一个账号 / 另一台服务器</Card.Description>
  </Card.Header>
  <Card.Content class="flex-row flex-wrap gap-2">
    <Button variant="outline" href="/api/export" download><Download />导出 JSON</Button>
    <label class="{buttonVariants({ variant: 'outline' })} cursor-pointer has-disabled:opacity-50">
      <Upload />{importing ? '导入中…' : '导入 JSON'}
      <input type="file" accept=".json,application/json" class="sr-only" onchange={importFile} disabled={importing} />
    </label>
  </Card.Content>
</Card.Root>
