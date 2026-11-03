<script lang="ts">
  import Field from '../Field.svelte';
  let { error }: { error?: string } = $props();
</script>

<Field label="Given name" hint="As written on the record" {error}>
  {#snippet children(f)}
    <input id={f.id} aria-describedby={f.describedBy} aria-invalid={f.invalid} />
  {/snippet}
</Field>
