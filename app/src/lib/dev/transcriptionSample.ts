// Development only: loaded with a dynamic import from App.svelte inside `import.meta.env.DEV`, so a production
// build never contains it.
import type { Api, Id, TextRegionInput } from '../api/api';
import { nav } from '../state/nav.svelte';

/** An invented letter photographed sideways: a 300 by 400 page whose writing runs down a 400 by 300 picture. */
const LINES: { text: string; u: number; v: number; len: number }[] = [
  { text: 'Dear Marta,', u: 30, v: 70, len: 150 },
  { text: 'the orchard is in bloom', u: 30, v: 120, len: 240 },
  { text: 'and the new well is dug.', u: 30, v: 160, len: 240 },
  { text: 'Your loving sister', u: 100, v: 320, len: 175 },
];

function sidewaysLetter(): string {
  const lines = LINES.map((l) => `<text x="${l.u}" y="${l.v - 8}" font-family="Georgia, serif" font-style="italic" font-size="22" fill="#3a2e22">${l.text}</text>`).join('');
  // Page point (u, v) lands on the picture at (400 - v, u): the page turned a quarter clockwise.
  const svg =
    `<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600" viewBox="0 0 400 300">` +
    `<rect width="400" height="300" fill="#6b5a48"/>` +
    `<g transform="translate(400 0) rotate(90)"><rect x="4" y="4" width="292" height="392" fill="#efe6d2"/>${lines}</g></svg>`;
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}

/** The pieces of the sideways letter, where its lines are on the picture upright by EXIF, each turned 90. */
function sidewaysPieces(): TextRegionInput[] {
  return LINES.map((l) => ({
    area: { x: (400 - l.v) / 400, y: l.u / 300, w: 34 / 400, h: l.len / 300 },
    rotation: 90 as const,
    text: l.text,
  }));
}

/**
 * `?route=transcription`: the first mock photo gets invented transcribed pieces and opens in the viewer.
 * `&sideways` shows an invented letter photographed sideways instead, `&turned` with the picture turned left once.
 */
export async function showTranscription(api: Api): Promise<void> {
  const params = new URLSearchParams(location.search);
  const page = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 200);
  for (const row of page.items) {
    const m = (await api.getPerson(row.id)).media[0]?.media;
    if (!m) continue;
    const sideways = params.has('sideways');
    const regions = sideways
      ? sidewaysPieces()
      : [
          { area: { x: 0.06, y: 0.08, w: 0.42, h: 0.1 }, text: 'Dear Marta,' },
          { area: { x: 0.06, y: 0.24, w: 0.58, h: 0.16 }, text: 'the orchard is in bloom and the new well is dug.' },
          { area: { x: 0.86, y: 0.12, w: 0.08, h: 0.6 }, rotation: 90 as const, text: 'Lviv, 3 May 1921' },
          { area: { x: 0.5, y: 0.78, w: 0.4, h: 0.1 }, text: 'Your loving sister' },
        ];
    if (sideways) {
      const letter = sidewaysLetter();
      const own = api.mediaUrl.bind(api);
      api.mediaUrl = (id: Id, size) => (id === m.id ? letter : own(id, size));
    }
    await api.apply({ type: 'setMediaTranscription', id: m.id, regions }, { silent: true });
    if (params.has('turned')) await api.apply({ type: 'setMediaRotation', id: m.id, rotation: 270 }, { silent: true });
    nav.focus(row.id);
    nav.go({ name: 'media' });
    setTimeout(() => document.querySelector<HTMLElement>('.grid button')?.click(), 400);
    // `&edit` shows edit mode with the second piece chosen; otherwise the first piece's text pops up.
    const edit = params.has('edit');
    setTimeout(() => {
      if (edit) document.querySelector<HTMLElement>('.bar button[aria-pressed="false"]')?.click();
      else document.querySelector<HTMLElement>('.layer button')?.focus();
    }, 900);
    if (edit) setTimeout(() => document.querySelectorAll<HTMLElement>('.layer button')[1]?.click(), 1300);
    return;
  }
}
