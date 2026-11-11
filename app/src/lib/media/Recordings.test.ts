import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import type { EditCommand, Id, MediaDto, MediaLinkDto, PersonDetail, TextRegionDto } from '../api/api';
import { clipAt, clipsOf, clockMs, parseClock, personRecordings, stepClip, wholeSounds } from './clips';
import { pieceAt } from './TranscriptPanel.svelte';
import { playerKey } from './RecordingPlayer.svelte';
import { voiceSamples } from './VoiceSamples.svelte';
import ClipPlayer from './ClipPlayer.svelte';
import RecordingPlayer from './RecordingPlayer.svelte';
import TranscriptPanel from './TranscriptPanel.svelte';
import PersonRecordings from './PersonRecordings.svelte';
import { recordingView } from '../state/recording.svelte';

// Everything here is invented: a made-up interview on a made-up source.

const sound = (over: Partial<MediaDto> = {}): MediaDto => ({
  id: 'rec1',
  title: 'Interview, side A',
  mime: 'audio/mp4',
  originalName: 'side-a.m4a',
  date: null,
  description: null,
  resolved: true,
  width: null,
  height: null,
  sha256: null,
  transcription: [],
  rotation: 0,
  durationMs: 1_200_000,
  ...over,
});

const clipLink = (id: string, startMs: number, endMs: number, label: string | null = null, media = sound()): MediaLinkDto => ({
  media,
  primary: false,
  clip: { id, startMs, endMs, label },
});

describe('clip times', () => {
  it('clock_writes_minutes_and_hours_and_tenths_when_asked', () => {
    expect(clockMs(65_000)).toBe('1:05');
    expect(clockMs(3_725_000)).toBe('1:02:05');
    expect(clockMs(65_300, true)).toBe('1:05.3');
    expect(clockMs(65_000, true)).toBe('1:05');
    expect(clockMs(-5)).toBe('0:00');
  });

  it('parse_reads_clock_forms_and_seconds_and_refuses_the_rest', () => {
    expect(parseClock('1:05')).toBe(65_000);
    expect(parseClock(' 01:05.25 ')).toBe(65_250);
    expect(parseClock('1:02:05')).toBe(3_725_000);
    expect(parseClock('75')).toBe(75_000);
    expect(parseClock('2.5')).toBe(2_500);
    for (const bad of ['', '1:5', '1:60', '1:60:00', 'abc', '-3', '1:05:']) expect(parseClock(bad), bad).toBeNull();
  });

  it('round_trips_every_tenth', () => {
    for (const ms of [0, 100, 59_900, 61_200, 3_599_900, 3_600_000, 36_000_100]) expect(parseClock(clockMs(ms, true))).toBe(ms);
  });
});

describe('clips among media links', () => {
  const whole: MediaLinkDto = { media: sound({ id: 'v' }), primary: false };
  const photo: MediaLinkDto = { media: sound({ id: 'p', mime: 'image/jpeg', originalName: 'p.jpg' }), primary: true };
  const links = [photo, clipLink('c2', 90_000, 95_000), whole, clipLink('c1', 10_000, 20_000)];

  it('separates_clips_from_whole_recordings_and_pictures', () => {
    expect(clipsOf(links).map((l) => l.clip.id)).toEqual(['c1', 'c2']);
    expect(wholeSounds(links).map((l) => l.media.id)).toEqual(['v']);
    expect(voiceSamples(links).map((l) => l.media.id)).toEqual(['v']);
    expect(clipsOf(undefined)).toEqual([]);
  });

  it('finds_the_clip_heard_and_steps_between_clips', () => {
    const clips = [
      { id: 'a', startMs: 0, endMs: 10_000 },
      { id: 'b', startMs: 5_000, endMs: 8_000 },
      { id: 'c', startMs: 30_000, endMs: 40_000 },
    ];
    expect(clipAt(clips, 6_000)?.id).toBe('b');
    expect(clipAt(clips, 9_000)?.id).toBe('a');
    expect(clipAt(clips, 20_000)).toBeNull();
    const starts = clips.map((c) => c.startMs);
    expect(stepClip(starts, 6_000, 1)).toBe(30_000);
    expect(stepClip(starts, 30_500, -1)).toBe(5_000);
    expect(stepClip(starts, 40_000, 1)).toBeNull();
    expect(stepClip(starts, 0, -1)).toBeNull();
  });

  it('gathers_a_persons_recordings_from_every_record_and_cited_sources', () => {
    const other = sound({ id: 'rec2', title: 'Side B' });
    const d = {
      ref: { id: 'p1', name: 'Olena Koval' },
      media: [clipLink('cp', 50_000, 60_000)],
      notes: [{ id: 'n1', text: 'Story', media: [clipLink('cn', 10_000, 20_000)] }],
      events: [{ id: 'e1', kind: 'birth', notes: [], citations: [], media: [clipLink('ce', 1_000, 2_000)] }],
      citations: [{ id: 'ci', source: { id: 's1', title: 'Interview', media: [{ media: other, primary: false }] }, media: [] }],
      names: [],
    } as unknown as PersonDetail;
    const recs = personRecordings(d);
    expect(recs.map((r) => r.media.id)).toEqual(['rec1', 'rec2']);
    expect(recs[0].clips.map((c) => [c.link.clip.id, c.owner.kind])).toEqual([
      ['ce', 'event'],
      ['cn', 'note'],
      ['cp', 'person'],
    ]);
    expect(recs[1]).toMatchObject({ cited: true, clips: [] });
  });
});

describe('player keys', () => {
  const key = (k: string, target: HTMLElement, over: Partial<KeyboardEventInit> = {}) => {
    const e = new KeyboardEvent('keydown', { key: k, ...over });
    Object.defineProperty(e, 'target', { value: target });
    return playerKey(e);
  };
  it('answers_space_arrows_and_page_keys_but_not_while_typing', () => {
    const div = document.createElement('div');
    const text = document.createElement('input');
    const range = document.createElement('input');
    range.type = 'range';
    const button = document.createElement('button');
    expect(key(' ', div)).toBe('toggle');
    expect(key('ArrowLeft', div)).toBe('back');
    expect(key('ArrowRight', div)).toBe('forward');
    expect(key('PageUp', div)).toBe('prevClip');
    expect(key('PageDown', range)).toBe('nextClip');
    expect(key(' ', range)).toBe('toggle');
    expect(key('ArrowLeft', range)).toBeNull();
    expect(key(' ', button)).toBeNull();
    expect(key(' ', text)).toBeNull();
    expect(key('PageDown', text)).toBeNull();
    expect(key(' ', div, { ctrlKey: true })).toBeNull();
  });
});

describe('the transcript follows playback', () => {
  const pieces: TextRegionDto[] = [
    { id: 't1', area: null, rotation: 0, text: 'Where were you born?', startMs: 0, endMs: 2_500, speaker: 'Grandson' },
    { id: 't2', area: null, rotation: 0, text: 'In the village.', startMs: 2_500, endMs: 7_000, speaker: 'Olena' },
    { id: 't3', area: null, rotation: 0, text: 'Untimed aside.' },
  ];
  it('picks_the_piece_being_said', () => {
    expect(pieceAt(pieces, 1_000)).toBe('t1');
    expect(pieceAt(pieces, 2_500)).toBe('t2');
    expect(pieceAt(pieces, 9_000)).toBeNull();
  });

  it('marks_the_current_piece_seeks_on_a_click_and_saves_times_and_speakers', async () => {
    const api = new MockApi(fixtureTree(5));
    const apply = vi.spyOn(api, 'apply').mockResolvedValue({ label: 'Edit transcription', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } });
    const onSeek = vi.fn();
    const onSaved = vi.fn();
    renderWith(TranscriptPanel, { media: sound({ transcription: pieces }), now: 3_000, onSeek, onSaved }, { api });
    const current = document.querySelector('li[aria-current="true"]');
    expect(current?.textContent).toContain('In the village.');
    await userEvent.click(screen.getByRole('button', { name: 'Play from 0:02' }));
    expect(onSeek).toHaveBeenCalledWith(2_500);

    await userEvent.click(screen.getByRole('button', { name: 'Edit transcript' }));
    const speakers = screen.getAllByLabelText('Speaker');
    await userEvent.clear(speakers[1]);
    await userEvent.type(speakers[1], 'Olena Koval');
    await userEvent.click(screen.getByRole('button', { name: 'Save transcript' }));
    const cmd = apply.mock.calls.at(-1)?.[0] as Extract<EditCommand, { type: 'setMediaTranscription' }>;
    expect(cmd.regions[1]).toEqual({ id: 't2', text: 'In the village.', startMs: 2_500, endMs: 7_000, speaker: 'Olena Koval' });
    expect(cmd.regions[2]).toEqual({ id: 't3', text: 'Untimed aside.' });
    expect(cmd.known).toEqual(['t1', 't2', 't3']);
    expect(onSaved).toHaveBeenCalled();
  });
});

describe('a clip where it is linked', () => {
  let play: ReturnType<typeof vi.fn>;
  beforeEach(() => {
    play = vi.fn(function (this: HTMLMediaElement) {
      Object.defineProperty(this, 'paused', { value: false, configurable: true });
      this.dispatchEvent(new Event('play'));
      return Promise.resolve();
    });
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(play as never);
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(function (this: HTMLMediaElement) {
      Object.defineProperty(this, 'paused', { value: true, configurable: true });
      this.dispatchEvent(new Event('pause'));
    });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    recordingView.close();
  });

  it('plays_from_its_start_stops_at_its_end_and_opens_the_whole_recording', async () => {
    renderWith(ClipPlayer, { link: clipLink('c1', 65_000, 102_500, 'How the house was built') });
    expect(screen.getByText('1:05 to 1:42')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Play the clip How the house was built' }));
    const audio = document.querySelector('audio')!;
    expect(play).toHaveBeenCalledTimes(1);
    expect(audio.currentTime).toBe(65);
    expect(screen.getByRole('button', { name: 'Pause the clip How the house was built' })).toBeTruthy();
    audio.currentTime = 102.6;
    await fireEvent.timeUpdate(audio);
    expect(audio.currentTime).toBe(65);
    expect(screen.getByRole('button', { name: 'Play the clip How the house was built' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Open the whole recording Interview, side A' }));
    expect(recordingView.open).toEqual({ media: 'rec1', clip: 'c1' });
  });

  it('says_so_when_the_file_is_missing_and_keeps_the_clip', () => {
    renderWith(ClipPlayer, { link: clipLink('c1', 0, 1_000, null, sound({ resolved: false })) });
    expect(screen.getByTestId('clip-missing').textContent).toContain('missing');
    expect(document.querySelector('audio')).toBeNull();
    expect(screen.getByRole('button', { name: /Open the whole recording/ })).toBeTruthy();
  });

  it('pauses_any_other_player_when_it_starts', async () => {
    const other = document.createElement('audio');
    document.body.append(other);
    Object.defineProperty(other, 'paused', { value: false, configurable: true });
    renderWith(ClipPlayer, { link: clipLink('c1', 0, 1_000, 'Hello') });
    await userEvent.click(screen.getByRole('button', { name: 'Play the clip Hello' }));
    expect(other.paused).toBe(true);
    other.remove();
  });
});

/** A mock tree with an interview on a source and clips of it on a note and an event of the first person. */
async function interview() {
  const api = new MockApi(fixtureTree(5));
  const people = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 10);
  const pid = people.items[0].id;
  const source = (await api.apply({ type: 'saveSource', source: { title: 'Interview, 1987' } })).touched[0];
  const media = (await api.attachFiles({ kind: 'source', id: source }, ['C:/Mock/side-a.m4a'])).attached[0];
  await api.apply({ type: 'saveNote', target: { kind: 'person', id: pid }, note: { kind: 'story', text: 'The house.' } });
  const d = await api.getPerson(pid);
  const note = d.notes.at(-1)!;
  const event = d.events[0];
  await api.apply({ type: 'addAudioClip', target: { kind: 'note', id: note.id }, media: media.id, startMs: 65_000, endMs: 102_500, label: 'The house' });
  await api.apply({ type: 'addAudioClip', target: { kind: 'event', id: event.id }, media: media.id, startMs: 5_000, endMs: 9_000 });
  return { api, pid, source, media, note, event };
}

describe('clips in the mock tree', () => {
  it('lists_the_clips_and_source_of_a_recording_and_refuses_bad_clips', async () => {
    const { api, pid, source, media, note } = await interview();
    const rec = await api.getRecording(media.id);
    expect(rec.sources).toEqual([{ id: source, title: 'Interview, 1987' }]);
    expect(rec.clips.map((c) => [c.clip.startMs, c.target.kind, c.person?.id])).toEqual([
      [5_000, 'event', pid],
      [65_000, 'note', pid],
    ]);
    const add = (over: Partial<Extract<EditCommand, { type: 'addAudioClip' }>>) =>
      api.apply({ type: 'addAudioClip', target: { kind: 'note', id: note.id }, media: media.id, startMs: 0, endMs: 1_000, ...over });
    await expect(add({ startMs: 5_000, endMs: 5_000 })).rejects.toThrow('must end after it starts');
    await expect(add({ endMs: media.durationMs! + 1 })).rejects.toThrow('The recording lasts');
    await expect(add({ target: { kind: 'source', id: source } })).rejects.toThrow('A clip goes on');
    await expect(add({ startMs: 65_000, endMs: 102_500, label: 'The house' })).rejects.toThrow('already on this record');
  });

  it('moves_a_clip_deletes_it_and_undo_brings_it_back', async () => {
    const { api, pid, media, note } = await interview();
    const id = (await api.getRecording(media.id)).clips[1].clip.id;
    await api.apply({ type: 'updateAudioClip', target: { kind: 'note', id: note.id }, clip: id, startMs: 60_000, endMs: 70_000, label: 'Moved', moveTo: { kind: 'person', id: pid } });
    const moved = (await api.getRecording(media.id)).clips.find((c) => c.clip.id === id)!;
    expect(moved).toMatchObject({ target: { kind: 'person', id: pid }, clip: { startMs: 60_000, endMs: 70_000, label: 'Moved' } });
    expect((await api.getPerson(pid)).media.some((l) => l.clip?.id === id)).toBe(true);
    await api.apply({ type: 'deleteAudioClip', target: { kind: 'person', id: pid }, clip: id });
    expect((await api.getRecording(media.id)).clips).toHaveLength(1);
    await api.undo();
    expect((await api.getRecording(media.id)).clips).toHaveLength(2);
  });

  it('deleting_the_recording_takes_every_clip_of_it', async () => {
    const { api, pid, media } = await interview();
    await api.apply({ type: 'deleteMedia', id: media.id });
    const d = await api.getPerson(pid);
    expect([...d.notes.flatMap((n) => n.media), ...d.events.flatMap((e) => e.media)].some((l) => l.media.id === media.id)).toBe(false);
  });
});

describe('the full player', () => {
  afterEach(() => vi.restoreAllMocks());

  async function open(clip?: Id) {
    const t = await interview();
    const onclose = vi.fn();
    renderWith(RecordingPlayer, { media: t.media.id, clip, onclose }, { api: t.api });
    await screen.findByRole('heading', { name: 'Clips of recordings' });
    return { ...t, onclose };
  }

  it('shows_the_source_marks_and_every_clip_with_where_it_is', async () => {
    await open();
    expect(screen.getByRole('button', { name: 'Open the source Interview, 1987' })).toBeTruthy();
    expect(document.querySelectorAll('.marks .mark')).toHaveLength(2);
    const list = document.querySelector('ol.list') as HTMLElement;
    expect(within(list).getByText('The house')).toBeTruthy();
    expect(within(list).getAllByRole('button', { name: /^Edit the clip/ })).toHaveLength(2);
    expect(screen.getByRole('slider', { name: 'Position in the recording' })).toBeTruthy();
    expect(screen.getByRole('combobox', { name: 'Speed' })).toBeTruthy();
  });

  it('starts_at_the_clip_it_was_opened_for_and_pages_between_clips', async () => {
    const t = await interview();
    const id = (await t.api.getRecording(t.media.id)).clips[1].clip.id;
    renderWith(RecordingPlayer, { media: t.media.id, clip: id, onclose: vi.fn() }, { api: t.api });
    const slider = await screen.findByRole('slider', { name: 'Position in the recording' });
    expect((slider as HTMLInputElement).value).toBe('65000');
    await fireEvent.keyDown(slider, { key: 'PageUp' });
    expect((slider as HTMLInputElement).value).toBe('5000');
    await fireEvent.keyDown(slider, { key: 'PageDown' });
    expect((slider as HTMLInputElement).value).toBe('65000');
    await userEvent.click(screen.getByRole('button', { name: 'Forward 10 seconds' }));
    expect((slider as HTMLInputElement).value).toBe('75000');
  });

  it('deletes_a_clip_as_one_undoable_command', async () => {
    const t = await open();
    const apply = vi.spyOn(t.api, 'apply');
    await userEvent.click(screen.getByRole('button', { name: 'Delete the clip The house' }));
    expect(apply.mock.calls.at(-1)?.[0]).toMatchObject({ type: 'deleteAudioClip', target: { kind: 'note', id: t.note.id } });
  });

  it('makes_a_clip_from_marked_times_on_a_chosen_record', async () => {
    const t = await open();
    const apply = vi.spyOn(t.api, 'apply');
    await userEvent.click(screen.getByRole('button', { name: 'New clip' }));
    const editor = screen.getByTestId('clip-editor');
    const start = within(editor).getByLabelText('Start');
    const end = within(editor).getByLabelText('End');
    await userEvent.clear(start);
    await userEvent.type(start, '2:00');
    await userEvent.clear(end);
    await userEvent.type(end, '1:00');
    expect(within(editor).getByText('The end must come after the start.')).toBeTruthy();
    await userEvent.clear(end);
    await userEvent.type(end, '2:30');
    await userEvent.click(within(editor).getByRole('button', { name: 'End one second later' }));
    expect((end as HTMLInputElement).value).toBe('2:31');
    await userEvent.type(within(editor).getByLabelText(/^Label/), 'The well');
    const record = within(editor).getByRole('combobox', { name: 'Record' });
    await waitFor(() => expect((record as HTMLSelectElement).options.length).toBeGreaterThan(2));
    await userEvent.selectOptions(record, `event:${t.event.id}`);
    await userEvent.click(within(editor).getByRole('button', { name: 'Add clip' }));
    expect(apply.mock.calls.at(-1)?.[0]).toEqual({
      type: 'addAudioClip',
      target: { kind: 'event', id: t.event.id },
      media: t.media.id,
      startMs: 120_000,
      endMs: 151_000,
      label: 'The well',
    });
    await waitFor(() => expect(screen.queryByTestId('clip-editor')).toBeNull());
  });
});

describe("a person's Recordings section", () => {
  afterEach(() => recordingView.close());

  it('lists_each_recording_with_its_clips_here_and_opens_the_player', async () => {
    const t = await interview();
    const detail = await t.api.getPerson(t.pid);
    renderWith(PersonRecordings, { detail }, { api: t.api });
    const item = screen.getByTestId('person-recording');
    expect(item.textContent).toContain('2 clips on this page');
    expect(within(item).getAllByTestId('clip-player')).toHaveLength(2);
    expect(within(item).getByText(/^Note: The house\./)).toBeTruthy();
    await userEvent.click(within(item).getByRole('button', { name: /^Open side-a in the player$/ }));
    expect(recordingView.open?.media).toBe(t.media.id);
  });

  it('says_when_no_recording_tells_of_the_person', async () => {
    const api = new MockApi(fixtureTree(5));
    const people = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1);
    renderWith(PersonRecordings, { detail: await api.getPerson(people.items[0].id) }, { api });
    expect(screen.getByText(/No recording tells of this person yet/)).toBeTruthy();
  });
});
