<script lang="ts">
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';
  import { t } from '../i18n';
  import IconButton from './IconButton.svelte';

  const api = getApi();
  const reversed = $derived(settings.value.treeDirection === 'reversed');

  /** Flips the "Tree direction" setting, so every tree view follows. */
  async function flip(): Promise<void> {
    try {
      await settings.update(api, { treeDirection: reversed ? 'normal' : 'reversed' });
    } catch {
      // The setting stays as it was; the button keeps showing it.
    }
  }
</script>

<!-- The icon shows the tree as it is drawn now; the tooltip says what a click does. -->
<IconButton
  label={t('tree.flip')}
  title={t(reversed ? 'tree.flipToNormal' : 'tree.flipToReversed')}
  icon={reversed ? 'tree-reversed' : 'tree-normal'}
  pressed={reversed}
  onclick={flip}
/>
