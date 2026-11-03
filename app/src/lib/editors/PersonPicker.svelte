<script lang="ts">
  import type { Id, PersonRef } from '../api/types';
  import { getApi } from '../api/context';
  import Combobox from '../ui/Combobox.svelte';
  import { t } from '../i18n';

  let {
    label,
    value,
    onPick,
    exclude = [],
    hint,
    error,
  }: {
    label: string;
    value: PersonRef | null;
    onPick: (p: PersonRef | null) => void;
    exclude?: Id[];
    hint?: string;
    error?: string;
  } = $props();

  const api = getApi();
  // svelte-ignore state_referenced_locally
  let text = $state(value?.name ?? '');
  let hits = $state<PersonRef[]>([]);
  let seq = 0;

  const items = $derived(hits.filter((p) => !exclude.includes(p.id)));
  const onlyExcluded = $derived(text.trim().length > 0 && hits.length > 0 && items.length === 0);

  $effect(() => {
    const n = value?.name ?? '';
    if (value && n !== text) text = n;
  });

  async function search(q: string) {
    const mine = ++seq;
    if (!q.trim()) {
      hits = [];
      return;
    }
    try {
      const r = await api.search(q, 12);
      if (mine !== seq) return;
      hits = r.filter((h) => h.kind === 'person' && h.person).map((h) => h.person!);
    } catch {
      if (mine === seq) hits = [];
    }
  }

  function onText(s: string) {
    text = s;
    if (!s.trim() && value) onPick(null);
    void search(s);
  }
</script>

<Combobox
  {label}
  {items}
  {value}
  {text}
  {onText}
  onPick={(p) => {
    text = p.name;
    onPick(p);
  }}
  render={(p) => p.name}
  detail={(p) => p.lifespan || undefined}
  {hint}
  error={error ?? (onlyExcluded ? t('fields.differentPerson') : undefined)}
/>
