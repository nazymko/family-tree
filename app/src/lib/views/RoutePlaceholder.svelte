<script lang="ts">
  let { title, note }: { title: string; note?: string } = $props();
</script>

<section class="ph">
  <h1>{title}</h1>
  {#if note}<p>{note}</p>{/if}
</section>

<style>
  .ph {
    display: grid;
    gap: var(--s-3);
  }
  p {
    color: var(--ink-1);
  }
</style>
