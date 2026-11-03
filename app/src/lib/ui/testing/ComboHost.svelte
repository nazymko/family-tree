<script lang="ts">
  import Combobox from '../Combobox.svelte';
  type Place = { id: number; name: string };
  const all: Place[] = [
    { id: 1, name: 'Hamburg' },
    { id: 2, name: 'Hannover' },
    { id: 3, name: 'Berlin' },
  ];
  let { onPick, onCreate }: { onPick?: (p: Place) => void; onCreate?: (t: string) => void } = $props();
  let text = $state('');
  let value = $state<Place | null>(null);
  const items = $derived(all.filter((p) => p.name.toLowerCase().startsWith(text.toLowerCase())));
</script>

<Combobox
  label="Place"
  {items}
  {value}
  {text}
  onText={(s) => (text = s)}
  onPick={(p) => {
    value = p;
    text = p.name;
    onPick?.(p);
  }}
  render={(p) => p.name}
  createLabel={(t) => `Add place "${t}"`}
  onCreate={(t) => onCreate?.(t)}
/>
