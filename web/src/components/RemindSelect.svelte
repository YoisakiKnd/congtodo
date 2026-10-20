<script lang="ts">
  import { REMIND_OPTIONS } from '../lib/time';

  let { value = $bindable(), label = '提醒' }: { value: number | null | undefined; label?: string } = $props();
</script>

<label class="field">
  {label}
  <select bind:value>
    {#each REMIND_OPTIONS as o}
      <option value={o.value}>{o.label}</option>
    {/each}
  </select>
</label>
