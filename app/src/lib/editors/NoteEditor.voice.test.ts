import { beforeEach, describe, expect, it, vi } from 'vitest';
import { act, fireEvent, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import NoteEditor from './NoteEditor.svelte';
import VoiceSamples from '../media/VoiceSamples.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { MockPlatform } from '../api/mock/MockPlatform';
import { fixtureTree } from '../api/mock/fixtures';
import type { MediaLinkDto, NoteDto, OwnerRef } from '../api/api';

async function setup() {
  const api = new MockApi(fixtureTree(40));
  const r = await api.apply({ type: 'createPerson', draft: { given: 'Ivan', surnames: ['Koval'], sex: 'male' } });
  const target: OwnerRef = { kind: 'person', id: r.focus! };
  return { api, target };
}

async function noteOf(api: MockApi, target: OwnerRef): Promise<NoteDto> {
  const d = await api.getPerson(target.id);
  return d.notes[d.notes.length - 1];
}

describe('NoteEditor voice samples', () => {
  let platform: MockPlatform;
  beforeEach(() => {
    platform = new MockPlatform();
  });

  it('attaches_a_picked_file_to_an_existing_note_and_shows_a_player', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'Olena Koval told this story.' } });
    const note = await noteOf(api, target);
    platform.next.files = ['C:/Mock/story.wav'];
    const spy = vi.spyOn(api, 'attachFiles');
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: 'Add voice sample' }));
    await waitFor(() => expect(screen.getAllByTestId('voice-sample')).toHaveLength(1));
    expect(spy).toHaveBeenCalledWith({ kind: 'note', id: note.id }, ['C:/Mock/story.wav']);
    expect(document.querySelector('audio')).not.toBeNull();
    const call = platform.calls.find((c) => c.method === 'pickFiles')!;
    expect((call.args[0] as { extensions: string[] }).extensions).toContain('flac');
  });

  it('drop_adds_and_remove_detaches', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'A recording.' } });
    const note = await noteOf(api, target);
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/a.mp3']));
    await waitFor(() => expect(screen.getAllByTestId('voice-sample')).toHaveLength(1));
    const user = userEvent.setup();
    await user.click(screen.getByRole('button', { name: /^Remove / }));
    await waitFor(() => expect(screen.queryAllByTestId('voice-sample')).toHaveLength(0));
    expect((await noteOf(api, target)).media).toHaveLength(0);
  });

  it('retitle_updates_the_media_title', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'Titled.' } });
    const note = await noteOf(api, target);
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/a.wav']));
    await waitFor(() => expect(screen.getAllByTestId('voice-sample')).toHaveLength(1));
    const box = screen.getByRole('textbox', { name: /^Title of / });
    await fireEvent.input(box, { target: { value: 'Grandmother singing' } });
    await fireEvent.change(box);
    await waitFor(async () => expect((await noteOf(api, target)).media[0].media.title).toBe('Grandmother singing'));
  });

  it('new_note_stages_files_and_attaches_them_on_save', async () => {
    const { api, target } = await setup();
    const onSaved = vi.fn();
    renderWith(NoteEditor, { target, onSaved, onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/first.ogg']));
    expect(await screen.findByText('first.ogg')).toBeInTheDocument();
    expect(screen.queryAllByTestId('voice-sample')).toHaveLength(0);
    const user = userEvent.setup();
    await user.click(screen.getByRole('textbox', { name: 'Note' }));
    await user.paste('Said on the porch.');
    await user.click(screen.getByRole('button', { name: 'Save note' }));
    await waitFor(() => expect(onSaved).toHaveBeenCalled());
    expect((await noteOf(api, target)).media).toHaveLength(1);
  });

  it('refused_file_is_listed_with_a_reason', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'x' } });
    const note = await noteOf(api, target);
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/photo.jpg']));
    expect(await screen.findByText(/photo\.jpg/)).toBeInTheDocument();
    expect(screen.queryAllByTestId('voice-sample')).toHaveLength(0);
  });

  it('refusal_is_worded_by_the_app_in_the_language_in_use', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'x' } });
    const note = await noteOf(api, target);
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/photo.jpg']));
    expect(await screen.findByText(/photo\.jpg: Only sound recordings can be attached to a note\./)).toBeInTheDocument();
    vi.spyOn(api, 'attachFiles').mockResolvedValueOnce({ attached: [], failed: [{ path: 'C:/Mock/notes.docx', reason: 'unsupported file type' }] });
    await act(() => platform.simulateDrop(['C:/Mock/notes.docx']));
    expect(await screen.findByText(/notes\.docx: unsupported type; we attach MP3/)).toBeInTheDocument();
  });

  it('each_title_field_has_its_own_name', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'Two.' } });
    const note = await noteOf(api, target);
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/one.wav', 'C:/Mock/two.wav']));
    await waitFor(() => expect(screen.getAllByTestId('voice-sample')).toHaveLength(2));
    expect(screen.getByRole('textbox', { name: 'Title of one' })).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: 'Title of two' })).toBeInTheDocument();
  });

  it('starting_one_player_pauses_the_other', async () => {
    const { api, target } = await setup();
    await api.apply({ type: 'saveNote', target, note: { kind: 'general', text: 'Two.' } });
    const note = await noteOf(api, target);
    renderWith(NoteEditor, { target, note, onSaved: vi.fn(), onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/one.wav', 'C:/Mock/two.wav']));
    await waitFor(() => expect(document.querySelectorAll('audio')).toHaveLength(2));
    const [a, b] = Array.from(document.querySelectorAll('audio'));
    const pause = vi.fn();
    Object.defineProperty(a, 'paused', { value: false, configurable: true });
    a.pause = pause;
    await fireEvent(b, new Event('play'));
    expect(pause).toHaveBeenCalledTimes(1);
  });

  it('a_partly_failed_first_save_is_not_left_dirty', async () => {
    const { api, target } = await setup();
    const onSaved = vi.fn();
    renderWith(NoteEditor, { target, onSaved, onCancel: vi.fn() }, { api, platform });
    await act(() => platform.simulateDrop(['C:/Mock/ok.wav']));
    const user = userEvent.setup();
    await user.click(screen.getByRole('textbox', { name: 'Note' }));
    await user.paste('Partly saved.');
    vi.spyOn(api, 'attachFiles').mockResolvedValueOnce({ attached: [], failed: [{ path: 'C:/Mock/ok.wav', reason: 'boom' }] });
    await user.click(screen.getByRole('button', { name: 'Save note' }));
    expect(await screen.findByText(/ok\.wav: boom/)).toBeInTheDocument();
    expect(onSaved).not.toHaveBeenCalled();
    // The text is saved, so closing must not ask about discarding it.
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(screen.queryByText(/discard/i)).toBeNull();
  });
});

describe('VoiceSamples', () => {
  const dto = (over: Record<string, unknown>): MediaLinkDto =>
    ({
      primary: false,
      media: { id: 'm1', mime: 'audio/wav', title: 'Hello', originalName: 'h.wav', resolved: true, ...over },
    }) as unknown as MediaLinkDto;

  it('player_has_src_and_length_shows_after_durationchange', async () => {
    const api = new MockApi(fixtureTree(10));
    renderWith(VoiceSamples, { items: [dto({})] }, { api });
    const audio = document.querySelector('audio')!;
    expect(audio.getAttribute('src')).toBeTruthy();
    expect(screen.getByTestId('voice-length').textContent).toMatch(/Loading/);
    Object.defineProperty(audio, 'duration', { value: 75, configurable: true });
    await fireEvent(audio, new Event('durationchange'));
    await waitFor(() => expect(screen.getByTestId('voice-length').textContent).toContain('1:15'));
  });

  it('missing_file_shows_a_message_not_a_player', () => {
    renderWith(VoiceSamples, { items: [dto({ resolved: false })] }, { api: new MockApi(fixtureTree(10)) });
    expect(screen.getByTestId('voice-missing')).toBeInTheDocument();
    expect(document.querySelector('audio')).toBeNull();
  });

  it('playback_error_replaces_the_player', async () => {
    renderWith(VoiceSamples, { items: [dto({})] }, { api: new MockApi(fixtureTree(10)) });
    await fireEvent(document.querySelector('audio')!, new Event('error'));
    expect(await screen.findByTestId('voice-error')).toBeInTheDocument();
    expect(document.querySelector('audio')).toBeNull();
  });

  it('non_audio_links_are_skipped', () => {
    renderWith(VoiceSamples, { items: [dto({ mime: 'image/jpeg' })] }, { api: new MockApi(fixtureTree(10)) });
    expect(screen.queryAllByTestId('voice-sample')).toHaveLength(0);
  });
});
