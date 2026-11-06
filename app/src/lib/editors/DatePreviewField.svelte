<script lang="ts">
  import TextField from '../ui/TextField.svelte';
  import { getApi } from '../api/context';
  import { settings } from '../state/settings.svelte';
  import { t } from '../i18n';
  import { dateText } from '../i18n/backend';

  let { value = $bindable(''), label = 'Date' }: { value?: string; label?: string } = $props();
  const api = getApi();
  let hint = $state<string | undefined>();
  let seq = 0;

  $effect(() => {
    const raw = value.trim();
    const order = settings.value.dayOrder;
    if (!raw) {
      hint = undefined;
      return;
    }
    const mine = ++seq;
    void api.parseDate(raw, order).then((p) => {
      if (mine !== seq) return;
      hint = p.status === 'unparsed' ? t('fields.notUnderstoodHint') : t('fields.understoodHint', { text: dateText(p.understoodAs) });
    });
  });
</script>

<TextField {label} bind:value {hint} autocomplete="off" />
