import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaView from './MediaView.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { project } from '../state/project.svelte';

describe('MediaView', () => {
  it('reloads_when_project_rev_changes', async () => {
    const api = new MockApi(fixtureTree(80));
    const id = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).items[0].id;
    renderWith(MediaView, { personId: id }, { api });
    await screen.findByRole('heading', { name: 'Media' });
    const name = (await api.getPerson(id)).ref.name;
    await api.apply({ type: 'saveName', person: id, name: { kind: 'birth', preferred: true, private: false, given: 'Reloaded', surnames: ['Person'] } });
    project.rev++;
    const fresh = (await api.getPerson(id)).ref.name;
    expect(fresh).not.toBe(name);
    await waitFor(() => expect(screen.getByText(fresh)).toBeInTheDocument());
  });

  /** Puts a recording straight on a person, the way an imported record can already hold one. */
  function seedPersonAudio(api: MockApi, id: string, mime: string | null, name: string) {
    const db = (api as unknown as { db: { media: Map<string, unknown>; persons: Map<string, { media: unknown[] }> } }).db;
    const m = { id: 'imported-1', title: null, mime, originalName: name, date: null, description: null, resolved: true, width: null, height: null, sha256: null };
    db.media.set(m.id, m);
    db.persons.get(id)!.media.push({ media: m, primary: false });
  }

  it('imported_audio_with_a_free_text_type_is_a_voice_row_not_a_broken_photo', async () => {
    const api = new MockApi(fixtureTree(40));
    const r = await api.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    seedPersonAudio(api, r.focus!, 'mp3', 'greeting.mp3');
    renderWith(MediaView, { personId: r.focus! }, { api });
    await waitFor(() => expect(screen.getAllByTestId('voice-sample')).toHaveLength(1));
    expect(document.querySelector('img')).toBeNull();
  });

  it('unlinked_media_left_by_a_deleted_note_is_listed_deletable_and_undo_brings_it_back', async () => {
    const api = new MockApi(fixtureTree(40));
    const r = await api.apply({ type: 'createPerson', draft: { given: 'Ivan', surnames: ['Koval'], sex: 'male' } });
    const target = { kind: 'person', id: r.focus! } as const;
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'Heard on the porch.' } });
    const note = (await api.getPerson(r.focus!)).notes[0];
    await api.attachFiles({ kind: 'note', id: note.id }, ['C:/Mock/porch.wav']);
    await api.apply({ type: 'deleteNote', target, note: note.id });
    renderWith(MediaView, { personId: r.focus! }, { api });
    const section = await screen.findByTestId('unlinked-media');
    expect(section.textContent).toContain('porch');
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Delete porch' }));
    await waitFor(() => expect(screen.queryByTestId('unlinked-media')).toBeNull());
    expect(await api.unlinkedMedia()).toHaveLength(0);
    // Undoing the media delete puts the file back in the list.
    await api.undo();
    project.rev++;
    await screen.findByTestId('unlinked-media');
  });

  it('audio_gets_a_player_and_no_image', async () => {
    const api = new MockApi(fixtureTree(40));
    const r = await api.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    const id = r.focus!;
    seedPersonAudio(api, id, 'audio/wav', 'voice.wav');
    const urls = vi.spyOn(api, 'mediaUrl');
    renderWith(MediaView, { personId: id }, { api });
    await screen.findByRole('heading', { name: 'Media' });
    await waitFor(() => expect(document.querySelector('audio')).not.toBeNull());
    expect(document.querySelector('img')).toBeNull();
    expect(urls.mock.calls.every((c) => c[1] !== 'thumb')).toBe(true);
  });
});
