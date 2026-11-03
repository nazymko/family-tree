<script lang="ts">
  import type { Component } from 'svelte';
  import type { Api } from '../../api/api';
  import { setApi } from '../../api/context';
  import { MockPlatform } from '../../api/mock/MockPlatform';

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let { api, view: View, props = {} }: { api: Api; view: Component<any>; props?: Record<string, unknown> } = $props();
  // The api is fixed for a host's lifetime.
  // svelte-ignore state_referenced_locally
  setApi(api, new MockPlatform());
</script>

<View {...props} />
