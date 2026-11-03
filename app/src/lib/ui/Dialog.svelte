<script lang="ts">
  import type { Snippet } from 'svelte';
  import IconButton from './IconButton.svelte';

  let {
    open,
    title,
    onclose,
    children,
    footer,
  }: { open: boolean; title: string; onclose: () => void; children: Snippet; footer?: Snippet } = $props();

  const uid = $props.id();
  let panel = $state<HTMLElement>();

  const FOCUSABLE =
    'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

  function focusables(): HTMLElement[] {
    return panel ? Array.from(panel.querySelectorAll<HTMLElement>(FOCUSABLE)) : [];
  }

  $effect(() => {
    if (!open || !panel) return;
    const opener = document.activeElement as HTMLElement | null;
    const body = panel.querySelector<HTMLElement>('.body');
    const first = body?.querySelector<HTMLElement>(FOCUSABLE) ?? focusables()[0] ?? panel;
    first.focus();
    return () => opener?.focus?.();
  });

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    } else if (e.key === 'Tab') {
      const list = focusables();
      if (list.length === 0) {
        e.preventDefault();
        return;
      }
      const first = list[0];
      const last = list[list.length - 1];
      const at = document.activeElement;
      if (e.shiftKey && (at === first || !panel?.contains(at))) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && (at === last || !panel?.contains(at))) {
        e.preventDefault();
        first.focus();
      }
    }
  }
</script>

{#if open}
  <div class="scrim" role="presentation" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
    <div
      bind:this={panel}
      class="dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="{uid}-title"
      tabindex="-1"
      {onkeydown}
    >
      <header>
        <h2 id="{uid}-title">{title}</h2>
        <IconButton label="Close" icon="x" onclick={onclose} />
      </header>
      <div class="body">{@render children()}</div>
      {#if footer}<footer>{@render footer()}</footer>{/if}
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: grid;
    place-items: center;
    padding: var(--s-6);
    background: var(--scrim);
  }
  .dialog {
    display: flex;
    flex-direction: column;
    width: min(40rem, 100%);
    max-height: calc(100dvh - 2 * var(--s-6));
    background: var(--paper-1);
    color: var(--ink-0);
    border: 1px solid var(--rule);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    animation: pop var(--dur-base) var(--ease);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
    padding: var(--s-4) var(--s-4) var(--s-4) var(--s-6);
    border-bottom: 1px solid var(--rule);
  }
  .body {
    padding: var(--s-6);
    overflow: auto;
    display: grid;
    gap: var(--s-5);
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--s-3);
    padding: var(--s-4) var(--s-6);
    border-top: 1px solid var(--rule);
    background: var(--paper-0);
    border-radius: 0 0 var(--r-2) var(--r-2);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
</style>
