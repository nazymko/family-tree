<script lang="ts">
  import { defaultApi, setApi } from './lib/api/context';
  import { settings } from './lib/state/settings.svelte';
  import { nav } from './lib/state/nav.svelte';
  import { project } from './lib/state/project.svelte';
  import { observeApi } from './lib/state/observe';
  import Shell from './lib/views/Shell.svelte';
  import Welcome from './lib/views/Welcome.svelte';
  import ImportReport from './lib/views/ImportReport.svelte';
  import { setLanguage } from './lib/i18n';

  setLanguage('system');

  // Plain browser (npm run dev): MockApi over a seeded 500-person tree. Inside Tauri: the real adapter.
  const bundle = defaultApi();
  const api = observeApi(bundle.api);
  setApi(api, bundle.platform);
  void settings.load(api);

  /** Development only (`?route=transcription`): the first mock photo gets invented transcribed pieces and opens in the viewer. */
  async function showTranscription() {
    const page = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 200);
    for (const row of page.items) {
      const m = (await api.getPerson(row.id)).media[0]?.media;
      if (!m) continue;
      const regions = [
        { area: { x: 0.06, y: 0.08, w: 0.42, h: 0.1 }, text: 'Dear Marta,' },
        { area: { x: 0.06, y: 0.24, w: 0.58, h: 0.16 }, text: 'the orchard is in bloom and the new well is dug.' },
        { area: { x: 0.86, y: 0.12, w: 0.08, h: 0.6 }, rotation: 90 as const, text: 'Lviv, 3 May 1921' },
        { area: { x: 0.5, y: 0.78, w: 0.4, h: 0.1 }, text: 'Your loving sister' },
      ];
      await api.apply({ type: 'setMediaTranscription', id: m.id, regions }, { silent: true });
      nav.focus(row.id);
      nav.go({ name: 'media' });
      setTimeout(() => document.querySelector<HTMLElement>('.grid button')?.click(), 400);
      // `&edit` shows edit mode with the second piece chosen; otherwise the first piece's text pops up.
      const edit = new URLSearchParams(location.search).has('edit');
      setTimeout(() => {
        if (edit) document.querySelector<HTMLElement>('.bar button[aria-pressed="false"]')?.click();
        else document.querySelector<HTMLElement>('.layer button')?.focus();
      }, 900);
      if (edit) setTimeout(() => document.querySelectorAll<HTMLElement>('.layer button')[1]?.click(), 1300);
      return;
    }
  }

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
        if (wanted === 'transcription') {
          await showTranscription();
          return;
        }
        if (wanted === 'person' && info.homePerson) nav.go({ name: 'person', id: info.homePerson });
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
