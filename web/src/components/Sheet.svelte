<script lang="ts">
  import type { Snippet } from 'svelte';

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

  let dlg: HTMLDialogElement;
  let busy = $state(false);

  $effect(() => {
    if (open && !dlg.open) dlg.showModal();
    else if (!open && dlg.open) dlg.close();
  });

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

<!-- 原生 <dialog>：自带焦点管理、Esc 关闭和顶层渲染 -->
<dialog bind:this={dlg} onclose={() => (open = false)} onclick={(e) => e.target === dlg && (open = false)}>
  {#if open}
    <form
      onsubmit={(e) => {
        e.preventDefault();
        run(onsave);
      }}
    >
      <h3>{title}</h3>
      <div class="fields">{@render children()}</div>
      <footer>
        {#if ondelete}
          <button type="button" class="btn ghost danger" onclick={() => confirm('确定删除？') && run(ondelete)}>删除</button>
        {/if}
        <span class="grow"></span>
        <button type="button" class="btn ghost" onclick={() => (open = false)}>取消</button>
        <button class="btn primary" disabled={busy}>保存</button>
      </footer>
    </form>
  {/if}
</dialog>

<style>
  dialog {
    width: min(480px, calc(100vw - 24px));
    max-height: calc(100dvh - 48px);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 22px;
    background: var(--surface);
    color: var(--text);
    box-shadow: var(--shadow-lg);
    overflow: auto;
  }
  dialog[open] {
    animation: pop 0.22s var(--ease);
  }
  dialog::backdrop {
    background: rgb(10 10 8 / 0.35);
    backdrop-filter: blur(3px);
  }
  form {
    padding: 22px;
  }
  h3 {
    font-size: 18px;
    margin-bottom: 18px;
  }
  footer {
    display: flex;
    gap: 8px;
    margin-top: 22px;
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(12px) scale(0.98);
    }
  }
  @media (max-width: 560px) {
    dialog {
      margin: auto 0 0;
      width: 100vw;
      max-width: 100vw;
      border-radius: 22px 22px 0 0;
      padding-bottom: env(safe-area-inset-bottom);
    }
    @keyframes pop {
      from {
        transform: translateY(40%);
      }
    }
  }
</style>
