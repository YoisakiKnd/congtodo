<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';

  let {
    open = $bindable(false),
    title,
    children,
    onsave,
    ondelete,
  }: {
    open?: boolean;
    title: string;
    children: Snippet;
    onsave: () => unknown;
    ondelete?: () => unknown;
  } = $props();

  let busy = $state(false);

  async function run(fn: () => unknown) {
    busy = true;
    try {
      await fn();
      open = false;
    } catch {
      // 错误已由 store 弹出提示
    } finally {
      busy = false;
    }
  }
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="max-h-[90dvh] overflow-y-auto sm:max-w-lg">
    <form
      class="grid gap-6"
      onsubmit={(e) => {
        e.preventDefault();
        run(onsave);
      }}
    >
      <Dialog.Header>
        <Dialog.Title>{title}</Dialog.Title>
      </Dialog.Header>
      <div class="grid gap-4">{@render children()}</div>
      <Dialog.Footer class="flex-row gap-2">
        {#if ondelete}
          <Button variant="destructive" onclick={() => confirm('确定删除？') && run(ondelete)}>删除</Button>
        {/if}
        <span class="flex-1"></span>
        <Button variant="outline" onclick={() => (open = false)}>取消</Button>
        <Button type="submit" disabled={busy}>保存</Button>
      </Dialog.Footer>
    </form>
  </Dialog.Content>
</Dialog.Root>
