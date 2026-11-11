// Development only: loaded with a dynamic import from App.svelte inside `import.meta.env.DEV`, so a production
// build never contains it.
import type { Api } from '../api/api';
import { nav } from '../state/nav.svelte';

/** `?route=transcription`: the first mock photo gets invented transcribed pieces and opens in the viewer. */
export async function showTranscription(api: Api): Promise<void> {
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
