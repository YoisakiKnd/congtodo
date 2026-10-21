<script lang="ts">
  // 单系列柱状图（完成率 0-1）：柱宽 ≤ 24px、顶端 4px 圆角、底部平直，柱间 2px 以上留白；
  // 悬停显示数值。系列颜色由调用方给（习惯的颜色），文字一律用文字色。
  // percent：value 是比例（0-1）时在提示里显示百分比；否则只显示 detail
  let {
    values,
    color,
    label,
    percent = true,
  }: { values: { label: string; value: number; detail: string }[]; color: string; label: string; percent?: boolean } = $props();

  const H = 64; // 绘图区高度（px）
  let W = $state(300); // 实际宽度（px），按容器测量，保证圆角和柱宽不变形
  const slot = $derived(W / Math.max(values.length, 1));
  const bar = $derived(Math.max(2, Math.min(slot * 0.6, 24))); // 不填满槽位，留出空气
  let hover = $state<number | null>(null);

  // 顶部圆角、底部平直的柱子
  function path(i: number, v: number) {
    const h = Math.max(v * H, v > 0 ? 2 : 0);
    if (!h) return '';
    const x = i * slot + (slot - bar) / 2;
    const r = Math.min(4, bar / 2, h);
    const y = H - h;
    return `M${x},${H} V${y + r} Q${x},${y} ${x + r},${y} H${x + bar - r} Q${x + bar},${y} ${x + bar},${y + r} V${H} Z`;
  }
</script>

<div class="relative" bind:clientWidth={W}>
  <svg width={W} height={H + 1} class="block overflow-visible" role="img" aria-label={label}>
    <!-- 基线：1px 发丝线 -->
    <line x1="0" x2={W} y1={H + 0.5} y2={H + 0.5} stroke="var(--border)" stroke-width="1" />
    {#each values as v, i (i)}
      <path d={path(i, v.value)} fill={color} opacity={hover == null || hover === i ? 1 : 0.45} />
      <!-- 命中区比柱子大：整列 -->
      <rect
        x={i * slot}
        y="0"
        width={slot}
        height={H}
        fill="transparent"
        role="presentation"
        onmouseenter={() => (hover = i)}
        onmouseleave={() => (hover = null)}
      />
    {/each}
  </svg>
  {#if hover != null}
    {@const v = values[hover]}
    <div
      class="bg-popover text-popover-foreground pointer-events-none absolute -top-2 z-10 -translate-x-1/2 -translate-y-full rounded-md px-2 py-1 text-xs whitespace-nowrap shadow-md ring-1 ring-foreground/10"
      style="left: {((hover + 0.5) / values.length) * 100}%"
    >
      <span class="text-muted-foreground">{v.label}</span>
      {#if percent}<b class="num ml-1">{Math.round(v.value * 100)}%</b>{/if}
      <span class={percent ? 'text-muted-foreground ml-1' : 'num ml-1 font-semibold'}>{v.detail}</span>
    </div>
  {/if}
  <!-- 表格视图：给读屏 -->
  <table class="sr-only">
    <caption>{label}</caption>
    <tbody>
      {#each values as v (v.label)}<tr><th>{v.label}</th><td>{percent ? `${Math.round(v.value * 100)}%（${v.detail}）` : v.detail}</td></tr>{/each}
    </tbody>
  </table>
</div>
