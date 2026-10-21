<script lang="ts">
  // 大号面板（习惯、倒数日、专注）：ui.panel 等于 name 时打开
  import type { Snippet } from 'svelte';
  import * as Dialog from '$lib/components/ui/dialog';
  import { ui, type PanelName } from '$lib/ui.svelte';

  let {
    name,
    title,
    actions,
    children,
  }: { name: PanelName; title: string; actions?: Snippet; children: Snippet } = $props();
</script>

<Dialog.Root open={ui.panel === name} onOpenChange={(v) => !v && (ui.panel = null)}>
  <Dialog.Content class="max-h-[92dvh] overflow-y-auto sm:max-w-4xl">
    <Dialog.Header class="flex-row flex-wrap items-center gap-3 pr-10">
      <Dialog.Title class="mr-auto text-lg">{title}</Dialog.Title>
      {@render actions?.()}
    </Dialog.Header>
    {@render children()}
  </Dialog.Content>
</Dialog.Root>
