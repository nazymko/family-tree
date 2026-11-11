// Development only: loaded with a dynamic import from App.svelte inside `import.meta.env.DEV`, so a production
// build never contains it. Everything here is invented.
import type { Api, Id, OwnerRef } from '../api/api';
import { nav } from '../state/nav.svelte';
import { recordingView } from '../state/recording.svelte';

const min = (m: number, s = 0) => (m * 60 + s) * 1000;

/**
 * `?route=recording`: the home person gets an invented interview on its source, a story note, a citation and clips
 * of the interview on the note, the birth, the citation and the person, with a time-coded transcript.
 * `&view=note` shows the story note with its clip, `&view=player` the full player, `&view=person` the Recordings section.
 */
export async function showRecording(api: Api, home: Id): Promise<void> {
  const view = new URLSearchParams(location.search).get('view') ?? 'player';
  const silent = { silent: true };
  const person: OwnerRef = { kind: 'person', id: home };
  const source = (await api.apply({ type: 'saveSource', source: { title: 'Interview with Olena, summer 1987', author: 'Recorded by her grandson' } }, silent))
    .touched[0];
  const attached = await api.attachFiles({ kind: 'source', id: source }, ['C:/Mock/Recordings/Interview side A.m4a'], { silent: true });
  const media = attached.attached[0].id;
  await api.apply({ type: 'updateMedia', id: media, title: 'Interview with Olena, side A' }, silent);
  await api.apply(
    {
      type: 'saveNote',
      target: person,
      note: { kind: 'story', text: 'How the family came to the village.\nShe tells how the house was built the summer the river flooded, and who carried the beams.' },
    },
    silent,
  );
  await api.apply({ type: 'addCitation', target: person, citation: { source, page: 'Side A', quality: 2 } }, silent);
  const d = await api.getPerson(home);
  const note = d.notes.find((n) => n.kind === 'story')!;
  const birth = d.events.find((e) => e.kind === 'birth') ?? d.events[0];
  const citation = d.citations.find((c) => c.source?.id === source)!;
  const clip = (target: OwnerRef, start: number, end: number, label: string) =>
    api.apply({ type: 'addAudioClip', target, media, startMs: start, endMs: end, label }, silent);
  await clip({ kind: 'note', id: note.id }, min(4, 12), min(6, 3), 'How the house was built');
  if (birth) await clip({ kind: 'event', id: birth.id }, min(1, 5), min(1, 42), 'Where she was born');
  await clip({ kind: 'citation', id: citation.id }, min(11, 30), min(13, 2), 'The names of the neighbours');
  await clip(person, min(17, 40), min(19, 15), 'Her first day at school');
  await api.apply(
    {
      type: 'setMediaTranscription',
      id: media,
      regions: [
        { text: 'Tell me where you were born.', startMs: min(1, 5), endMs: min(1, 9), speaker: 'Grandson' },
        { text: 'In the house by the mill, the one with the blue shutters.', startMs: min(1, 9), endMs: min(1, 20), speaker: 'Olena' },
        { text: 'My mother said it snowed for three days after.', startMs: min(1, 20), endMs: min(1, 42), speaker: 'Olena' },
        { text: 'And the new house?', startMs: min(4, 12), endMs: min(4, 15), speaker: 'Grandson' },
        { text: 'That was the summer the river flooded. Father and uncle Petro carried the beams.', startMs: min(4, 15), endMs: min(4, 40), speaker: 'Olena' },
      ],
    },
    silent,
  );
  const rec = await api.getRecording(media);
  nav.go({ name: 'person', id: home });
  const scrollTo = (id: string) =>
    setTimeout(() => {
      const h = document.getElementById(id);
      h?.scrollIntoView({ block: 'start' });
    }, 600);
  if (view === 'note') scrollTo('h-notes');
  else if (view === 'person') scrollTo('h-recordings');
  else setTimeout(() => recordingView.show(media, rec.clips.find((c) => c.target.kind === 'event')?.clip.id), 400);
}
