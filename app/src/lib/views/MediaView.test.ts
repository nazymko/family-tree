import { describe, expect, it, vi } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
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

  it('audio_gets_a_player_and_no_image', async () => {
    const api = new MockApi(fixtureTree(40));
    const r = await api.apply({ type: 'createPerson', draft: { given: 'Olena', surnames: ['Koval'], sex: 'female' } });
    const id = r.focus!;
    await api.attachFiles({ kind: 'person', id }, ['C:/Mock/voice.wav']);
    const urls = vi.spyOn(api, 'mediaUrl');
    renderWith(MediaView, { personId: id }, { api });
    await screen.findByRole('heading', { name: 'Media' });
    await waitFor(() => expect(document.querySelector('audio')).not.toBeNull());
    expect(document.querySelector('img')).toBeNull();
    expect(urls.mock.calls.every((c) => c[1] !== 'thumb')).toBe(true);
  });
});
