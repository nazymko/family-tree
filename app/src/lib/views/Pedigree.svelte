<script lang="ts">
  import type { Id } from '../api/types';
  import ChartView from './ChartView.svelte';

  let { onOpenPerson }: { onOpenPerson?: (id: Id) => void } = $props();
</script>

<ChartView kind="pedigree" {onOpenPerson} />
