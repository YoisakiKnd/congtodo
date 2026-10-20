<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  let { label, hint, children, class: className }: { label: string; hint?: string; children: Snippet; class?: string } =
    $props();
</script>

<!-- 用 <label> 包裹，点击文字即可聚焦输入框 -->
<label class={cn('grid min-w-0 gap-2', className)}>
  <span class="text-sm leading-none font-medium">{label}</span>
  {@render children()}
  {#if hint}<span class="text-muted-foreground text-xs">{hint}</span>{/if}
</label>
