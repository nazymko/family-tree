<script lang="ts">
  import type { Api } from '../../api/api';
  import { setApi } from '../../api/context';
  import { MockPlatform } from '../../api/mock/MockPlatform';
  import type { OpenBranch } from '../../api/types';
  import ChartCanvas from '../ChartCanvas.svelte';
  import type { PrintLayout } from '../sheets';

  // A stand-in for the chart screen: it keeps the layout choice and the openings, as the screen does.
  let { api, root, kind = 'around', withControl = true, person }: { api: Api; root: string; kind?: 'around' | 'pedigree'; withControl?: boolean; person: string } = $props();
  // The api is fixed for a host's lifetime.
  // svelte-ignore state_referenced_locally
  setApi(api, new MockPlatform());
  let layout = $state<PrintLayout>('today');
  let openings = $state<OpenBranch[]>([]);
</script>

<button onclick={() => (openings = [{ person, siblings: 'all' }])}>open-branch</button>
<ChartCanvas {kind} {root} generations={4} onRecentre={() => {}} onOpenPerson={() => {}} printLayout={layout} onPrintLayout={withControl ? (v) => (layout = v) : undefined} {openings} />
