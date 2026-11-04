<script lang="ts">
  import Dialog from '../ui/Dialog.svelte';
  import Kbd from '../ui/Kbd.svelte';
  import { currentPlatform, formatKeys, type Shortcut } from '../state/shortcuts';
  import { t } from '../i18n';

  let { open, onclose, shortcuts }: { open: boolean; onclose: () => void; shortcuts: Shortcut[] } = $props();
  const plat = currentPlatform();
</script>

<Dialog {open} title={t('shortcuts.title')} {onclose}>
  <table class="sheet">
    <caption class="sr">{t('shortcuts.title')}</caption>
    <thead class="sr"><tr><th scope="col">{t('shortcuts.action')}</th><th scope="col">{t('shortcuts.keys')}</th></tr></thead>
    <tbody>
      {#each shortcuts as s (s.id)}
        <tr>
          <th scope="row">{s.label}</th>
          <td><Kbd>{formatKeys(s.keys, plat)}</Kbd></td>
        </tr>
      {/each}
    </tbody>
  </table>
</Dialog>

<style>
  .sheet {
    border-collapse: collapse;
    min-width: min(28rem, 80vw);
  }
  th[scope='row'] {
    text-align: start;
    font-weight: 400;
  }
  th,
  td {
    padding: var(--s-2) var(--s-4) var(--s-2) 0;
    border-block-end: 1px solid var(--rule);
  }
  td {
    text-align: end;
    white-space: nowrap;
  }
  .sr {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
