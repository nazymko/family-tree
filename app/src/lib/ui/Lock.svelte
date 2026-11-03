<script lang="ts">
  let { size = '0.95rem' }: { size?: string } = $props();
</script>

<svg
  class="lock"
  role="img"
  aria-label="Private"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
>
  <title>Private</title>
  <rect x="3" y="11" width="18" height="11" rx="2" />
  <path d="M7 11V7a5 5 0 0 1 10 0v4" />
</svg>

<style>
  .lock {
    flex: none;
    vertical-align: -0.12em;
  }
</style>
