<script lang="ts">
  import DateField from '../DateField.svelte';

  // Holds the value the way a dialog does, so the field sees its own changes come back.
  let { initial = '', spy }: { initial?: string; spy: (raw: string) => void } = $props();
  // svelte-ignore state_referenced_locally
  let value = $state(initial);
</script>

<DateField
  label="Date of death"
  {value}
  onChange={(raw) => {
    value = raw;
    spy(raw);
  }}
  order="dmy"
/>
