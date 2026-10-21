<script lang="ts">
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { db } from '$lib/store.svelte';
  import type { ChannelIds } from '$lib/types';

  // 不选任何渠道 = 默认（按各渠道在设置里勾选的提醒类型推送）
  let { value = $bindable() }: { value: ChannelIds | undefined } = $props();

  const selected = $derived((value ?? []).map(String));
</script>

<div class="grid gap-2">
  <span class="text-sm font-medium">推送到</span>
  {#if db.channels.items.length}
    <ToggleGroup.Root
      type="multiple"
      variant="outline"
      size="sm"
      spacing={1}
      class="flex-wrap"
      value={selected}
      onValueChange={(v: string[]) => (value = v.length ? v.map(Number) : null)}
    >
      {#each db.channels.items as c (c.id)}
        <ToggleGroup.Item value={String(c.id)} disabled={!c.enabled}>{c.name}</ToggleGroup.Item>
      {/each}
    </ToggleGroup.Root>
    <span class="text-muted-foreground text-xs">
      {selected.length ? '只推送到选中的渠道' : '未选择时按默认：推送到接收这类提醒的所有渠道'}
    </span>
  {:else}
    <span class="text-muted-foreground text-xs">还没有通知渠道，可在「设置」中添加</span>
  {/if}
</div>
