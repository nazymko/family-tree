<script lang="ts">
  import { defaultApi, setApi } from './lib/api/context';
  import { settings } from './lib/state/settings.svelte';
  import { nav } from './lib/state/nav.svelte';
  import { project } from './lib/state/project.svelte';
  import { observeApi } from './lib/state/observe';
  import Shell from './lib/views/Shell.svelte';
  import Welcome from './lib/views/Welcome.svelte';
  import ImportReport from './lib/views/ImportReport.svelte';

  document.documentElement.lang = 'en';

  // Plain browser (npm run dev): MockApi over a seeded 500-person tree. Inside Tauri: the real adapter.
  const bundle = defaultApi();
  const api = observeApi(bundle.api);
  setApi(api, bundle.platform);
  void settings.load(api);

  // Development only: `?route=family|people|...` opens the mock tree straight onto a view (used for screenshots).
  if (import.meta.env.DEV && typeof location !== 'undefined') {
    const wanted = new URLSearchParams(location.search).get('route');
    if (wanted && wanted !== 'welcome') {
      void (async () => {
        const info = await api.openProject('C:/Genealogy/Schmidt family');
        if (wanted === 'importReport') {
          const report = await api.importGedcom({
            file: 'C:/Mock/Family tree.ged',
            projectDir: 'C:/Users/me/OneDrive/Trees/Schmidt',
            projectName: 'Schmidt family',
            mediaRoots: [],
            dayOrder: 'dmy',
          });
          project.open(report.project, report);
        } else {
          project.open(info);
        }
        nav.focus(info.homePerson);
        if (wanted === 'person') nav.go({ name: 'person', id: info.homePerson });
        else if (wanted === 'export') {
          nav.go({ name: 'family' });
          setTimeout(() => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'E', ctrlKey: true, shiftKey: true, bubbles: true })), 400);
        } else nav.go({ name: wanted } as never);
      })();
    }
  }
</script>

{#if nav.route.name === 'welcome' || !project.info}
  <Welcome />
{:else if nav.route.name === 'importReport'}
  <ImportReport />
{:else}
  <Shell />
{/if}
