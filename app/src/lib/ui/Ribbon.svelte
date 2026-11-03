<script lang="ts">
  let { title = 'Home person', size = '1.25rem' }: { title?: string; size?: string } = $props();
</script>

<svg class="ribbon" role="img" aria-label={title} width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
  <title>{title}</title>
  <path d="M6 2h12a1 1 0 0 1 1 1v19l-7-4.5L5 22V3a1 1 0 0 1 1-1z" />
</svg>

<style>
  .ribbon {
    flex: none;
    color: var(--ribbon);
  }
</style>
