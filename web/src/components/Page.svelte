<script lang="ts">
  import type { Snippet } from 'svelte';
  import { cn } from '$lib/utils';

  let {
    title,
    sub,
    actions,
    children,
    class: className,
  }: { title: string; sub?: string; actions?: Snippet; children: Snippet; class?: string } = $props();
</script>

<div class={cn('mx-auto w-full max-w-6xl px-4 py-6 md:px-8 md:py-8', className)}>
  <header class="mb-6 flex flex-wrap items-center gap-3">
    <div class="mr-auto">
      {#if sub}<p class="text-muted-foreground text-sm">{sub}</p>{/if}
      <h1 class="text-2xl font-semibold tracking-tight md:text-3xl">{title}</h1>
    </div>
    {@render actions?.()}
  </header>
  {@render children()}
</div>
