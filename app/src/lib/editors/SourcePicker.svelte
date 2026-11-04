<script lang="ts" module>
  import type { SourceDto } from '../api/api';
  export type SourceChoice = SourceDto | { title: string };
</script>

<script lang="ts">
  import Combobox from '../ui/Combobox.svelte';
  import { getApi } from '../api/context';
  import { t } from '../i18n';

  let { value = $bindable(null), allowCreate = true }: { value?: SourceChoice | null; allowCreate?: boolean } = $props();

  const api = getApi();
  let text = $state(value?.title ?? '');
  let items = $state<SourceDto[]>([]);
  let seq = 0;

  $effect(() => {
    const q = text;
    const mine = ++seq;
    void api.suggestSources(q, 8).then((r) => {
      if (mine === seq) items = r;
    });
  });

  const selected = $derived(value && 'id' in value ? value : null);
</script>

<Combobox
  label={t('citation.source')}
  {items}
  value={selected}
  {text}
  onText={(s) => {
    text = s;
    if (value && value.title !== s) value = null;
  }}
  onPick={(s) => {
    value = s;
    text = s.title ?? '';
  }}
  render={(s) => s.title ?? t('citation.untitled')}
  createLabel={allowCreate ? (x) => t('citation.createSource', { title: x }) : undefined}
  onCreate={allowCreate
    ? (t) => {
        value = { title: t };
        text = t;
      }
    : undefined}
  hint={value && !('id' in value) ? t('citation.newSourceHint') : undefined}
/>
