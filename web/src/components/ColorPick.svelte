<script lang="ts">
  import { PALETTE } from '../lib/store.svelte';

  let { value = $bindable() }: { value: string | null | undefined } = $props();
</script>

<div class="swatches" role="radiogroup" aria-label="颜色">
  {#each PALETTE as c}
    <button
      type="button"
      role="radio"
      aria-checked={value === c}
      aria-label={c}
      class:on={value === c}
      style="--c: {c}"
      onclick={() => (value = c)}
    ></button>
  {/each}
</div>

<style>
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  button {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 2px var(--surface) inset;
    border: 2px solid transparent;
    transition: transform 0.12s;
  }
  button:hover {
    transform: scale(1.1);
  }
  button.on {
    border-color: var(--c);
  }
</style>
