<script lang="ts">
  import Toast from './Toast.svelte';
  import { toasts } from '../state/toast.svelte';
  import { t } from '../i18n';

  const LIFETIME = 6000;

  /** Auto-dismiss after 6 s; the countdown pauses while the pointer or focus is on the toast. */
  function autoDismiss(node: HTMLElement, args: { id: number; keep: boolean }) {
    if (args.keep) return {};
    let remaining = LIFETIME;
    let started = Date.now();
    let timer: ReturnType<typeof setTimeout> | undefined = setTimeout(done, remaining);
    function done() {
      toasts.dismiss(args.id);
    }
    function pause() {
      if (timer === undefined) return;
      clearTimeout(timer);
      timer = undefined;
      remaining -= Date.now() - started;
    }
    function resume() {
      if (timer !== undefined) return;
      started = Date.now();
      timer = setTimeout(done, Math.max(remaining, 1500));
    }
    node.addEventListener('mouseenter', pause);
    node.addEventListener('mouseleave', resume);
    node.addEventListener('focusin', pause);
    node.addEventListener('focusout', resume);
    return {
      destroy() {
        if (timer !== undefined) clearTimeout(timer);
        node.removeEventListener('mouseenter', pause);
        node.removeEventListener('mouseleave', resume);
        node.removeEventListener('focusin', pause);
        node.removeEventListener('focusout', resume);
      },
    };
  }
</script>

<div class="region" role="region" aria-label={t('ui.notifications')} aria-live="polite">
  {#each toasts.items as t (t.id)}
    <div class="slot" use:autoDismiss={{ id: t.id, keep: t.kind === 'error' }}>
      <Toast
        message={t.text}
        kind={t.kind}
        actionLabel={t.action?.label}
        onaction={() => {
          t.action?.run();
          toasts.dismiss(t.id);
        }}
        ondismiss={() => toasts.dismiss(t.id)}
      />
    </div>
  {/each}
</div>

<style>
  .region {
    position: fixed;
    inset-inline-start: 50%;
    inset-block-end: var(--s-6);
    transform: translateX(-50%);
    display: grid;
    gap: var(--s-3);
    width: min(32rem, calc(100vw - 2 * var(--s-5)));
    z-index: 60;
    pointer-events: none;
  }
  .slot {
    pointer-events: auto;
  }
</style>
