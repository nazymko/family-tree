<script lang="ts">
  import type { Id, PlaceOption } from '../api/types';
  import { getApi } from '../api/context';
  import Combobox from '../ui/Combobox.svelte';
  import { t } from '../i18n';

  let {
    label,
    value,
    text,
    onChange,
    onOption,
  }: {
    label: string;
    value: PlaceOption | null;
    text: string;
    onChange: (v: { placeId?: Id; placeText?: string } | null) => void;
    /** Also told which existing place was chosen (for callers that need its text). */
    onOption?: (p: PlaceOption) => void;
  } = $props();

  const api = getApi();
  // svelte-ignore state_referenced_locally
  let typed = $state(value ? value.full : text);
  // svelte-ignore state_referenced_locally
  let picked = $state<PlaceOption | null>(value);
  let items = $state<PlaceOption[]>([]);
  let seq = 0;

  async function fetchItems(prefix: string) {
    const mine = ++seq;
    try {
      const r = await api.suggestPlaces(prefix, 8);
      if (mine === seq) items = r;
    } catch {
      if (mine === seq) items = [];
    }
  }

  function onText(s: string) {
    typed = s;
    picked = null;
    void fetchItems(s.trim());
    // Typing alone already records the text, so Save works without choosing a row.
    onChange(s.trim() ? { placeText: s.trim() } : null);
  }
</script>

<Combobox
  {label}
  {items}
  value={picked}
  text={typed}
  {onText}
  onPick={(p) => {
    picked = p;
    typed = p.full;
    onChange({ placeId: p.id });
    onOption?.(p);
  }}
  render={(p) => p.name}
  detail={(p) => p.full}
  createLabel={(x) => t('fields.create', { text: x })}
  onCreate={(t) => {
    typed = t;
    picked = null;
    onChange({ placeText: t });
  }}
  hint={t('fields.placeHint')}
/>
