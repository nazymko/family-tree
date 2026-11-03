<script lang="ts">
  import type { Component } from 'svelte';
  import type { Api, Platform } from '../../api/api';
  import { setApi } from '../../api/context';
  import { MockPlatform } from '../../api/mock/MockPlatform';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let { api, comp, props = {} }: { api: Api; comp: Component<any>; props?: Record<string, unknown> } = $props();
  const platform: Platform = new MockPlatform();
  // svelte-ignore state_referenced_locally
  setApi(api, platform);
  const C = $derived(comp);
</script>

<C {...props} />
