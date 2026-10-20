<script lang="ts" generics="T">
  import * as Select from '$lib/components/ui/select';

  // 选项值可以是任意类型（数字、null 等），内部用下标作为 Select 的字符串值
  let {
    value = $bindable(),
    options,
    class: className = 'w-full',
  }: { value: T; options: { value: T; label: string }[]; class?: string } = $props();

  const idx = $derived(String(Math.max(0, options.findIndex((o) => o.value === value))));
</script>

<Select.Root type="single" value={idx} onValueChange={(v) => (value = options[+v].value)}>
  <Select.Trigger class={className}>{options[+idx]?.label}</Select.Trigger>
  <Select.Content>
    {#each options as o, i (i)}
      <Select.Item value={String(i)} label={o.label} />
    {/each}
  </Select.Content>
</Select.Root>
