<script lang="ts">
  import PagedList from '../PagedList.svelte';

  let {
    total,
    loadPage,
    onSelect,
    onActivate,
    selectedIndex = -1,
    initialPage,
  }: {
    total: number;
    loadPage: (o: number, l: number) => Promise<unknown[]>;
    onSelect: (i: number) => void;
    onActivate: (i: number) => void;
    selectedIndex?: number;
    initialPage?: { offset: number; items: unknown[] };
  } = $props();
</script>

<PagedList
  {total}
  rowHeight={40}
  pageSize={200}
  {loadPage}
  {selectedIndex}
  {onSelect}
  {onActivate}
  {initialPage}
  ariaLabel="People list"
  keyPrefix="t"
>
  {#snippet row(item, index)}
    <span>{item === null ? `loading ${index}` : item}</span>
  {/snippet}
</PagedList>
