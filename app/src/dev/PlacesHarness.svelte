<script lang="ts">
  // Dev-only page (places.html): the Places view on MockApi with invented records, for screenshots.
  // Query: ?lang=uk|en &theme=dark &view=events|import &who=ours|others &kind=death
  import { setApi } from '../lib/api/context';
  import { MockApi } from '../lib/api/mock/MockApi';
  import { MockPlatform } from '../lib/api/mock/MockPlatform';
  import { fixtureTree } from '../lib/api/mock/fixtures';
  import { applyTheme } from '../lib/state/theme';
  import { setLanguage } from '../lib/i18n';
  import PlacesView from '../lib/places/PlacesView.svelte';
  import { seedPlaceDemo } from '../lib/places/demoData';

  const q = new URLSearchParams(location.search);
  const api = new MockApi(fixtureTree(10));
  const platform = new MockPlatform();
  setApi(api, platform);
  document.documentElement.lang = 'en';
  if (q.get('lang') === 'uk' || q.get('lang') === 'en') {
    setLanguage(q.get('lang') as 'uk' | 'en');
    void api.setSettings({ language: q.get('lang') as 'uk' | 'en' });
  }
  if (q.get('theme') === 'dark' || q.get('theme') === 'light') applyTheme(q.get('theme') as 'dark' | 'light');

  let ready = $state(false);
  void (async () => {
    await seedPlaceDemo(api, 400);
    ready = true;
    await new Promise((r) => setTimeout(r, 400));
    // Choose the village, then click what the query asks for.
    const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
    const click = async (find: () => HTMLElement | null | undefined) => {
      for (let i = 0; i < 50; i++) {
        const el = find();
        if (el) return el.click();
        await wait(60);
      }
    };
    const input = document.querySelector<HTMLInputElement>('[data-filter-input] , input');
    if (input) {
      input.value = 'Zelenivka';
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
    await click(() => [...document.querySelectorAll<HTMLElement>('ul button')].find((b) => b.textContent?.trim().startsWith('Zelenivka')));
    const who = q.get('who');
    if (who) await click(() => [...document.querySelectorAll<HTMLElement>('label')].find((l) => l.textContent?.trim() === (who === 'ours' ? (q.get('lang') === 'uk' ? 'Рідні' : 'Relatives') : q.get('lang') === 'uk' ? 'Інші' : 'Others'))?.querySelector('input'));
    if (q.get('view') === 'import') {
      await click(() => [...document.querySelectorAll<HTMLElement>('button')].find((b) => /Import records|Імпорт записів/.test(b.textContent ?? '')));
      await click(() => [...document.querySelectorAll<HTMLElement>('button')].find((b) => /Choose files|Обрати файли/.test(b.textContent ?? '')));
      await wait(300);
      await click(() => [...document.querySelectorAll<HTMLElement>('button')].find((b) => /^(Preview|Перегляд)$/.test(b.textContent?.trim() ?? '')));
    }
  })();
</script>

<main>
  {#if ready}<PlacesView onOpenPerson={() => {}} />{/if}
</main>

<style>
  main {
    box-sizing: border-box;
    min-height: 100vh;
    padding: var(--s-4) var(--s-5);
  }
</style>
