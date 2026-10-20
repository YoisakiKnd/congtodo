<script lang="ts">
  import { PALETTE } from '$lib/store.svelte';
  import { cn } from '$lib/utils';

  let { value = $bindable() }: { value: string | null | undefined } = $props();
</script>

<div class="flex flex-wrap gap-2" role="radiogroup" aria-label="颜色">
  {#each PALETTE as c (c)}
    <button
      type="button"
      role="radio"
      aria-checked={value === c}
      aria-label={c}
      class={cn(
        'size-7 rounded-full ring-offset-2 ring-offset-background transition-transform hover:scale-110',
        value === c && 'ring-2',
      )}
      style="background: {c}; --tw-ring-color: {c}"
      onclick={() => (value = c)}
    ></button>
  {/each}
</div>
