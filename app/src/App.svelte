<script lang="ts">
  import { defaultApi, setApi } from './lib/api/context';
  import { settings } from './lib/state/settings.svelte';

  document.documentElement.lang = 'en';

  // Plain browser (npm run dev): MockApi over a seeded 500-person tree. Inside Tauri: the real adapter.
  const { api, platform } = defaultApi();
  setApi(api, platform);
  void settings.load(api);
</script>

<main>
  <h1>Gene Tree</h1>
</main>
