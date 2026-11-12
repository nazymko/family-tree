import { describe, expect, it, vi } from 'vitest';
import { screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import type { EditCommand, MediaDto, TextRegionDto } from '../api/api';
import TranscriptPanel from './TranscriptPanel.svelte';
import { foldQuery, highlightParts, pieceShown, speakerCounts, withSpeakers } from './transcriptTools';

// Everything here is invented: made-up speakers and made-up talk.

const piece = (id: string, startMs: number | null, text: string, speaker?: string): TextRegionDto =>
  ({ id, area: null, rotation: 0, text, ...(startMs != null ? { startMs, endMs: startMs + 1_000 } : {}), ...(speaker ? { speaker } : {}) }) as TextRegionDto;

const pieces: TextRegionDto[] = [
  piece('a', 0, 'Where were you born?', 'Ivan Test'),
  piece('b', 2_000, "In the village, near the mill. It's a long story.", 'Maria Sample [?]'),
  piece('c', 4_000, 'Ми жили на Заході, у ПЕРШОМУ домі.', 'Maria Sample'),
  piece('d', 6_000, 'Хто там був?', 'Ivan Test'),
  piece('e', 8_000, 'Nobody. Only the dog.', 'Maria Sample [?]'),
  piece('f', 10_000, 'Someone spoke, unclear.'),
];

const sound = (transcription: TextRegionDto[]): MediaDto =>
  ({
    id: 'rec1',
    title: 'Talk',
    mime: 'audio/mp4',
    originalName: 'a.m4a',
    date: null,
    description: null,
    resolved: true,
    width: null,
    height: null,
    sha256: null,
    transcription,
    rotation: 0,
    durationMs: 100_000,
  }) as MediaDto;

describe('transcript search logic', () => {
  it('matches_case_insensitively_in_ukrainian_and_treats_apostrophes_alike', () => {
    expect(pieceShown(pieces[2], new Set(), foldQuery('першому'))).toBe(true);
    expect(pieceShown(pieces[2], new Set(), foldQuery('ЗАХОДІ'))).toBe(true);
    expect(pieceShown(pieces[1], new Set(), foldQuery('It’s'))).toBe(true);
    expect(pieceShown(pieces[1], new Set(), foldQuery('itʼs'))).toBe(true);
    expect(pieceShown(pieces[1], new Set(), foldQuery('zzz'))).toBe(false);
  });

  it('matches_the_speaker_name_too_and_combines_with_the_speaker_choice', () => {
    const q = foldQuery('sample');
    expect(pieces.filter((p) => pieceShown(p, new Set(), q)).map((p) => p.id)).toEqual(['b', 'c', 'e']);
    expect(pieces.filter((p) => pieceShown(p, new Set(['Maria Sample [?]']), q)).map((p) => p.id)).toEqual(['b', 'e']);
    expect(pieces.filter((p) => pieceShown(p, new Set(['']), '')).map((p) => p.id)).toEqual(['f']);
  });

  it('cuts_the_text_into_marked_parts_by_the_original_letters', () => {
    expect(highlightParts('Ми жили на Заході', foldQuery('ЗАХ'))).toEqual([
      { text: 'Ми жили на ', hit: false },
      { text: 'Зах', hit: true },
      { text: 'оді', hit: false },
    ]);
    expect(highlightParts("It's", foldQuery('’s'))).toEqual([
      { text: 'It', hit: false },
      { text: "'s", hit: true },
    ]);
    expect(highlightParts('plain', '')).toEqual([{ text: 'plain', hit: false }]);
  });

  it('counts_speakers_with_no_speaker_last', () => {
    expect(speakerCounts(pieces)).toEqual([
      { speaker: 'Ivan Test', count: 2 },
      { speaker: 'Maria Sample [?]', count: 2 },
      { speaker: 'Maria Sample', count: 1 },
      { speaker: '', count: 1 },
    ]);
  });

  it('rewrites_only_speakers_and_keeps_ids_times_text_and_order', () => {
    const { regions, known } = withSpeakers(pieces, (p) => (p.speaker === 'Ivan Test' ? 'Ivan T.' : p.speaker ?? undefined));
    expect(known).toEqual(['a', 'b', 'c', 'd', 'e', 'f']);
    expect(regions[0]).toEqual({ id: 'a', text: 'Where were you born?', startMs: 0, endMs: 1_000, speaker: 'Ivan T.' });
    expect(regions[5]).toEqual({ id: 'f', text: 'Someone spoke, unclear.', startMs: 10_000, endMs: 11_000 });
    expect(regions[2]).toMatchObject({ id: 'c', speaker: 'Maria Sample' });
  });
});

describe('the transcript panel tools', () => {
  function mount() {
    const api = new MockApi(fixtureTree(5));
    const apply = vi.spyOn(api, 'apply').mockResolvedValue({ label: 'Edit transcription', focus: null, touched: [], undo: { canUndo: true, canRedo: false, undoLabel: null, redoLabel: null } });
    const onSeek = vi.fn();
    const onSaved = vi.fn();
    renderWith(TranscriptPanel, { media: sound(pieces), now: 2_500, onSeek, onSaved }, { api });
    const cmd = () => apply.mock.calls.at(-1)![0] as Extract<EditCommand, { type: 'setMediaTranscription' }>;
    return { apply, onSeek, onSaved, cmd };
  }
  const rows = () => Array.from(document.querySelectorAll('li[data-piece]'));
  const search = () => screen.getByLabelText('Search the transcript');

  it('searches_highlights_counts_clears_and_still_seeks', async () => {
    const { onSeek } = mount();
    expect(screen.getByText('6 of 6')).toBeTruthy();
    await userEvent.type(search(), 'THE');
    expect(rows().map((r) => r.getAttribute('data-piece'))).toEqual(['b', 'e']);
    expect(screen.getByText('2 of 6')).toBeTruthy();
    expect(Array.from(document.querySelectorAll('mark')).map((m) => m.textContent)).toEqual(['the', 'the', 'the']);
    await userEvent.click(within(rows()[1] as HTMLElement).getByRole('button', { name: 'Play from 0:08' }));
    expect(onSeek).toHaveBeenCalledWith(8_000);

    await userEvent.clear(search());
    await userEvent.type(search(), 'it’s');
    expect(rows().map((r) => r.getAttribute('data-piece'))).toEqual(['b']);

    await userEvent.type(search(), '{Escape}');
    expect((search() as HTMLInputElement).value).toBe('');
    expect(rows()).toHaveLength(6);

    await userEvent.type(search(), 'nothing like this');
    expect(screen.getByText('No piece matches.')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Clear the search' }));
    expect(rows()).toHaveLength(6);
    expect(document.activeElement).toBe(search());
  });

  it('never_puts_user_text_in_as_html', async () => {
    const api = new MockApi(fixtureTree(5));
    renderWith(TranscriptPanel, { media: sound([piece('x', 0, '<img src=x onerror=alert(1)> end', 'Bob')]), now: 0, onSeek: vi.fn(), onSaved: vi.fn() }, { api });
    await userEvent.type(search(), 'img');
    expect(document.querySelector('li img')).toBeNull();
    expect(document.querySelector('mark')?.textContent).toBe('img');
  });

  it('keeps_the_playing_piece_marked_only_while_it_is_shown', async () => {
    mount();
    expect(document.querySelector('li[aria-current="true"]')?.getAttribute('data-piece')).toBe('b');
    await userEvent.type(search(), 'dog');
    expect(document.querySelector('li[aria-current="true"]')).toBeNull();
    expect(rows()).toHaveLength(1);
  });

  it('filters_by_speaker_alone_and_with_search', async () => {
    mount();
    const group = screen.getByRole('group', { name: 'Show only' });
    await userEvent.click(within(group).getByRole('button', { name: /^Ivan Test \(2\)$/ }));
    expect(rows().map((r) => r.getAttribute('data-piece'))).toEqual(['a', 'd']);
    expect(screen.getByText('2 of 6')).toBeTruthy();
    await userEvent.type(search(), 'хто');
    expect(rows().map((r) => r.getAttribute('data-piece'))).toEqual(['d']);
    await userEvent.clear(search());
    await userEvent.click(within(group).getByRole('button', { name: /^No speaker \(1\)$/ }));
    expect(rows().map((r) => r.getAttribute('data-piece'))).toEqual(['a', 'd', 'f']);
  });

  it('renames_a_speaker_everywhere_merging_two_labels_in_one_save', async () => {
    const { apply, cmd, onSaved } = mount();
    const rename = screen.getByRole('button', { name: 'Rename a speaker…' });
    await userEvent.click(rename);
    const dialog = screen.getByRole('dialog');
    await userEvent.selectOptions(within(dialog).getByLabelText('Speaker to rename'), 'Maria Sample [?]');
    await userEvent.type(within(dialog).getByLabelText('New name'), 'Maria Sample');
    expect(within(dialog).getByText('2 pieces will change')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Rename' }));
    expect(apply).toHaveBeenCalledTimes(1);
    const c = cmd();
    expect(c.known).toEqual(['a', 'b', 'c', 'd', 'e', 'f']);
    expect(c.regions.map((r) => [r.id, r.speaker, r.startMs, r.text])).toEqual(pieces.map((p) => [p.id, p.id === 'b' || p.id === 'e' ? 'Maria Sample' : p.speaker, p.startMs, p.text]));
    expect(onSaved).toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(document.activeElement).toBe(rename);
  });

  it('rename_can_be_limited_to_the_pieces_shown_and_can_remove_a_speaker', async () => {
    const { cmd } = mount();
    await userEvent.type(search(), 'dog');
    await userEvent.click(screen.getByRole('button', { name: 'Rename a speaker…' }));
    const dialog = screen.getByRole('dialog');
    await userEvent.selectOptions(within(dialog).getByLabelText('Speaker to rename'), 'Maria Sample [?]');
    expect(within(dialog).getByText('2 pieces will change')).toBeTruthy();
    await userEvent.click(within(dialog).getByLabelText('Only the pieces shown'));
    expect(within(dialog).getByText('1 piece will change')).toBeTruthy();
    expect(within(dialog).getByText('The speaker will be removed from 1 piece.')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Remove the speaker' }));
    const c = cmd();
    expect(c.regions.find((r) => r.id === 'e')).toEqual({ id: 'e', text: 'Nobody. Only the dog.', startMs: 8_000, endMs: 9_000 });
    expect(c.regions.find((r) => r.id === 'b')?.speaker).toBe('Maria Sample [?]');
  });

  it('changes_the_speaker_of_one_piece_with_one_save_and_returns_focus', async () => {
    const { apply, cmd } = mount();
    await userEvent.click(within(rows()[3] as HTMLElement).getByRole('button', { name: 'Change the speaker: Ivan Test' }));
    const input = screen.getByLabelText('Speaker');
    expect(document.activeElement).toBe(input);
    await userEvent.clear(input);
    await userEvent.type(input, 'Ivan Other{Enter}');
    expect(apply).toHaveBeenCalledTimes(1);
    const c = cmd();
    expect(c.regions.map((r) => r.speaker)).toEqual(['Ivan Test', 'Maria Sample [?]', 'Maria Sample', 'Ivan Other', 'Maria Sample [?]', undefined]);
    expect(c.regions[3]).toEqual({ id: 'd', text: 'Хто там був?', startMs: 6_000, endMs: 7_000, speaker: 'Ivan Other' });
    await vi.waitFor(() => expect(document.activeElement).toBe(within(rows()[3] as HTMLElement).getByRole('button', { name: /Change the speaker/ })));
  });

  it('adds_a_speaker_to_a_piece_without_one_and_escape_cancels_without_saving', async () => {
    const { apply, cmd } = mount();
    const add = within(rows()[5] as HTMLElement).getByRole('button', { name: 'Add speaker' });
    await userEvent.click(add);
    await userEvent.type(screen.getByLabelText('Speaker'), 'Guest{Escape}');
    expect(apply).not.toHaveBeenCalled();
    await vi.waitFor(() => expect(document.activeElement).toBe(within(rows()[5] as HTMLElement).getByRole('button', { name: 'Add speaker' })));
    await userEvent.click(document.activeElement as HTMLElement);
    await userEvent.type(screen.getByLabelText('Speaker'), 'Guest{Enter}');
    expect(apply).toHaveBeenCalledTimes(1);
    expect(cmd().regions[5]).toEqual({ id: 'f', text: 'Someone spoke, unclear.', startMs: 10_000, endMs: 11_000, speaker: 'Guest' });
  });

  it('draws_edit_rows_a_page_at_a_time', async () => {
    const many = Array.from({ length: 120 }, (_, i) => piece(`p${i}`, i * 1_000, `Line ${i}`, 'Ivan Test'));
    const api = new MockApi(fixtureTree(5));
    renderWith(TranscriptPanel, { media: sound(many), now: 0, onSeek: vi.fn(), onSaved: vi.fn() }, { api });
    await userEvent.click(screen.getByRole('button', { name: 'Edit transcript' }));
    expect(document.querySelectorAll('textarea')).toHaveLength(50);
    await userEvent.click(screen.getByRole('button', { name: 'Show 70 more pieces' }));
    expect(document.querySelectorAll('textarea')).toHaveLength(100);
  });
});
