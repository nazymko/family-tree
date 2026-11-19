<script lang="ts">
  // Dev-only page (maps.html): the Map on MockApi with many synthetic old maps, for screenshots and frame times.
  // Query: ?layers=40 (how many) &series=5 &shown=all|none|one (which are shown at first) &lang=uk|en &theme=dark &panel=1
  import { setApi } from '../lib/api/context';
  import { MockApi } from '../lib/api/mock/MockApi';
  import { MockPlatform } from '../lib/api/mock/MockPlatform';
  import { fixtureTree } from '../lib/api/mock/fixtures';
  import { applyTheme } from '../lib/state/theme';
  import { setLanguage } from '../lib/i18n';
  import MapView from '../lib/map/MapView.svelte';

  const q = new URLSearchParams(location.search);
  const api = new MockApi(fixtureTree(60));
  const platform = new MockPlatform();
  setApi(api, platform);
  Object.assign(window, { __api: api, __platform: platform });
  document.documentElement.lang = 'en';
  if (q.get('lang') === 'uk' || q.get('lang') === 'en') {
    setLanguage(q.get('lang') as 'uk' | 'en');
    void api.setSettings({ language: q.get('lang') as 'uk' | 'en' });
  }
  if (q.get('theme') === 'dark' || q.get('theme') === 'light') applyTheme(q.get('theme') as 'dark' | 'light');

  const SERIES = ['Three-verst map', 'Two-verst map', 'Schubert', 'Strelbitsky', 'Rizzi-Zannoni', 'Beauplan', 'Atlas of the Commonwealth'];
  const count = Math.max(0, Math.min(200, Number(q.get('layers') ?? 40)));
  const series = Math.max(1, Math.min(SERIES.length, Number(q.get('series') ?? 5)));
  const shown = q.get('shown') ?? 'one';

  let ready = $state(false);
  void (async () => {
    // Invented sheets in a grid over an invented region; each series a little offset from the others.
    for (let i = 0; i < count; i++) {
      const s = i % series;
      const k = Math.floor(i / series);
      const lat = 51.5 - Math.floor(k / 4) * 0.8 - s * 0.15;
      const lon = 27 + (k % 4) * 1.1 + s * 0.2;
      const t = 0.02;
      const c = (la: number, lo: number) => ({ latE6: Math.round(la * 1e6), lonE6: Math.round(lo * 1e6) });
      const visible = shown === 'all' || (shown === 'one' && s === 2);
      await api.addMapLayerFile(`C:/Maps/${SERIES[s]} ${k + 1}.jpg`, `${SERIES[s]}, sheet ${k + 1}`, { series: SERIES[s], year: String(1800 + s * 20 + k), opacity: 70, visible, corners: [c(lat, lon + t), c(lat, lon + 1.1 - t), c(lat - 0.8, lon + 1.1), c(lat - 0.8, lon)] });
    }
    ready = true;
  })();
</script>

<main>
  {#if ready}<MapView onOpenPerson={() => {}} />{/if}
</main>

<style>
  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
  }
</style>
