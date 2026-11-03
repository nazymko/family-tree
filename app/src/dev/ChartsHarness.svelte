<script lang="ts">
  // Dev-only page (charts.html): the pedigree and descendant views on MockApi, no shell.
  // Query: ?view=pedigree|descendants &mode=chart|outline &theme=dark &root=cycle|many
  import { setApi } from '../lib/api/context';
  import { MockApi } from '../lib/api/mock/MockApi';
  import { MockPlatform } from '../lib/api/mock/MockPlatform';
  import { fixtureTree } from '../lib/api/mock/fixtures';
  import { applyTheme } from '../lib/state/theme';
  import { nav } from '../lib/state/nav.svelte';
  import Pedigree from '../lib/views/Pedigree.svelte';
  import Descendants from '../lib/views/Descendants.svelte';

  const q = new URLSearchParams(location.search);
  const fx = fixtureTree(500);
  const api = new MockApi(fx);
  setApi(api, new MockPlatform());
  document.documentElement.lang = 'en';
  if (q.get('theme') === 'dark' || q.get('theme') === 'light') applyTheme(q.get('theme') as 'dark' | 'light');
  try {
    localStorage.setItem('gt.chartView', q.get('mode') ?? 'chart');
  } catch {
    // Storage is optional here.
  }
  const view = q.get('view') ?? 'pedigree';

  let ready = $state(false);
  void (async () => {
    // The mock tree is shallow: pick the person with the biggest chart for the requested view.
    const kind = view === 'descendants' ? 'descendants' : 'pedigree';
    let best = (await api.getSettings()).homePerson ?? fx.persons[0].id;
    const special = q.get('root');
    if (special === 'cycle') best = fx.specials.cycleA!;
    else if (special === 'many') best = fx.specials.manyPartners!;
    else {
      let count = 0;
      for (const p of fx.persons) {
        const m = await api.chartOpen({ kind, root: p.id, generations: 5 });
        await api.chartClose(m.chartId);
        if (m.nodeCount > count) {
          count = m.nodeCount;
          best = p.id;
        }
      }
    }
    nav.focus(best);
    ready = true;
  })();
</script>

<main>
  {#if ready}
    {#if view === 'descendants'}<Descendants />{:else}<Pedigree />{/if}
  {/if}
</main>

<style>
  main {
    box-sizing: border-box;
    height: 100vh;
    padding: var(--s-5);
    background: var(--paper-0);
  }
</style>
