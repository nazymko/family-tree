<script lang="ts">
  // Test and harness host: provides the Api context and renders one component with props.
  import type { Component } from 'svelte';
  import type { Api, Platform } from '../api/api';
  import { setApi } from '../api/context';

  let {
    api,
    platform,
    comp,
    props = {},
  }: { api: Api; platform: Platform; comp: Component<any>; props?: Record<string, unknown> } = $props(); // eslint-disable-line @typescript-eslint/no-explicit-any

  // svelte-ignore state_referenced_locally
  setApi(api, platform);
</script>

{#if comp}
  {@const C = comp}
  <C {...props} />
{/if}
