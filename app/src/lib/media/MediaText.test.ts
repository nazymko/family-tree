import { describe, expect, it, vi } from 'vitest';
import { fireEvent, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import MediaViewer from './MediaViewer.svelte';
import MediaGrid from './MediaGrid.svelte';
import RecordingPlayer from './RecordingPlayer.svelte';
import TranscriptPanel from './TranscriptPanel.svelte';
import { renderWith } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import type { MediaDto, MediaLinkDto, TextRegionDto } from '../api/api';

// Invented text on an invented letter and an invented interview; no real document or recording.
const PIECES: TextRegionDto[] = [
  { id: 'r1', area: { x: 0.1, y: 0.1, w: 0.5, h: 0.1 }, rotation: 0, text: 'Dear Marta,' },
  { id: 'r2', area: { x: 0.1, y: 0.3, w: 0.6, h: 0.2 }, rotation: 0, text: 'the orchard is in bloom.' },
];

const media = (id: string, over: Partial<MediaDto> = {}): MediaDto => ({
  id,
  title: `Letter ${id}`,
  mime: 'image/jpeg',
  originalName: `${id}.jpg`,
  date: null,
  description: null,
  resolved: true,
  width: 800,
  height: 400,
  sha256: null,
  transcription: PIECES,
  rotation: 0,
  ...over,
});

const box = (i: number) => screen.getByRole('button', { name: `Text piece ${i}` });

function viewer(over: Partial<MediaDto> = {}) {
  const api = new MockApi(fixtureTree(5));
  const items: MediaLinkDto[] = [{ media: media('m0', over), primary: false }];
  renderWith(MediaViewer, { items, index: 0, onClose: vi.fn() }, { api });
}

describe('text on a picture: the tip', () => {
  it('shows the text beside the pointer on hover, outside the clipped stage', async () => {
    const user = userEvent.setup();
    viewer();
    await user.hover(box(2));
    const tip = screen.getByRole('tooltip');
    expect(tip.textContent).toContain('the orchard is in bloom.');
    // Moved out of the stage (which clips what it holds) so it can be read at any zoom and pan.
    expect(screen.getByRole('group', { name: /^Preview/ }).contains(tip)).toBe(false);
    expect(getComputedStyle(tip).position).toBe('fixed');
    await user.unhover(box(2));
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('shows the tip on a tap with a finger and keeps it when the finger lifts', async () => {
    viewer();
    const b = box(1);
    // jsdom's pointer events carry no pointerType, so the finger is told to the handlers by hand.
    const touch = (type: string, bubbles = true) => {
      const e = new MouseEvent(type, { bubbles });
      Object.defineProperty(e, 'pointerType', { value: 'touch' });
      b.dispatchEvent(e);
    };
    touch('pointerenter', false);
    touch('pointerdown');
    touch('pointerup');
    await fireEvent.click(b);
    expect(screen.getByRole('tooltip').textContent).toContain('Dear Marta,');
    touch('pointerleave', false);
    expect(screen.getByRole('tooltip')).toBeInTheDocument();
    // A second tap on the same piece closes it.
    touch('pointerdown');
    await fireEvent.click(b);
    expect(screen.queryByRole('tooltip')).toBeNull();
  });

  it('draws the outlines only while "Text on image" is on, and the pieces still answer when it is off', async () => {
    const user = userEvent.setup();
    viewer();
    const layer = document.querySelector('.layer')!;
    const toggle = screen.getByRole('button', { name: 'Text on image' });
    expect(toggle).toHaveAttribute('aria-pressed', 'false');
    expect(layer.classList.contains('quiet')).toBe(true);
    await user.hover(box(1));
    expect(screen.getByRole('tooltip')).toBeInTheDocument();
    await user.click(toggle);
    expect(layer.classList.contains('quiet')).toBe(false);
  });

  it('offers "Mark text" at once on a picture, and the full text lists the pieces', async () => {
    const user = userEvent.setup();
    viewer();
    await user.click(screen.getByRole('button', { name: 'Mark text' }));
    expect(screen.getByText('Drag across the picture where the text is written.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Mark text' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: 'Edit text' })).toHaveAttribute('aria-pressed', 'true');
  });
});

describe('media tiles: the tip and the transcript marker', () => {
  it('says what the file is, with date, place, description and a transcript hint, and marks it', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(5));
    const withText = media('a', {
      title: 'Letter to Marta',
      description: 'Written in spring.\nSecond line.',
      date: { display: 'May 1950' } as MediaDto['date'],
      place: { full: 'Lviv, Ukraine' } as MediaDto['place'],
    });
    const plain = media('b', { title: 'Orchard', transcription: [] });
    const items: MediaLinkDto[] = [
      { media: withText, primary: false },
      { media: plain, primary: false },
    ];
    renderWith(MediaGrid, { items, onOpen: vi.fn() }, { api });
    expect(screen.getAllByTestId('has-transcript')).toHaveLength(1);
    await user.hover(screen.getByRole('button', { name: 'View Letter to Marta' }));
    const tip = await screen.findByRole('tooltip');
    for (const s of ['Letter to Marta', 'Scan of a document', 'May 1950', 'Lviv, Ukraine', 'Written in spring.', 'Has a transcript']) expect(tip.textContent).toContain(s);
    await user.unhover(screen.getByRole('button', { name: 'View Letter to Marta' }));
    await user.hover(screen.getByRole('button', { name: 'View Orchard' }));
    const tip2 = await screen.findByRole('tooltip');
    expect(tip2.textContent).toContain('Photo');
    expect(tip2.textContent).not.toContain('Has a transcript');
  });

  it('a tap still opens the item', async () => {
    const user = userEvent.setup();
    const api = new MockApi(fixtureTree(5));
    const onOpen = vi.fn();
    renderWith(MediaGrid, { items: [{ media: media('a'), primary: false }], onOpen }, { api });
    await user.click(screen.getByRole('button', { name: /^View/ }));
    expect(onOpen).toHaveBeenCalledWith(0);
  });
});

/** Whether an element scrolls by itself (jsdom keeps the `overflow` shorthand apart from its longhands). */
const scrolls = (el: Element) => {
  const cs = getComputedStyle(el);
  return /auto|scroll/.test(cs.overflowY || cs.overflow);
};

describe('the recording player: one scrollbar', () => {
  async function open() {
    const api = new MockApi(fixtureTree(5));
    const source = (await api.apply({ type: 'saveSource', source: { title: 'Interview, 1987' } })).touched[0];
    const rec = (await api.attachFiles({ kind: 'source', id: source }, ['C:/Mock/side-a.m4a'])).attached[0];
    const regions = Array.from({ length: 60 }, (_, i) => ({ text: `Line number ${i + 1} of the talk.`, startMs: i * 5000, endMs: i * 5000 + 4000, speaker: i % 2 ? 'Ivan' : 'Olena' }));
    await api.apply({ type: 'setMediaTranscription', id: rec.id, regions, known: [] });
    renderWith(RecordingPlayer, { media: rec.id, onclose: vi.fn() }, { api });
    const list = await screen.findByTestId('transcript-list');
    return { api, rec, list };
  }

  it('fills the dialog height and lets only the transcript list scroll', async () => {
    const { list } = await open();
    const dialog = screen.getByRole('dialog');
    expect(dialog.classList.contains('fill')).toBe(true);
    expect(getComputedStyle(dialog).height).toContain('100dvh');
    expect(scrolls(list)).toBe(true);
    // Every container between the list and the page: none of them scrolls, so there is one scrollbar, not two.
    const chain: Element[] = [];
    for (let el = list.parentElement; el && el !== document.body; el = el.parentElement) chain.push(el);
    const offenders = chain.filter((el) => scrolls(el) && !el.classList.contains('main'));
    expect(offenders.map((e) => e.className)).toEqual([]);
    // The player's own column is not an ancestor of the list: the two columns scroll apart, and the list is alone in its.
    expect(list.closest('[data-testid="recording-main"]')).toBeNull();
    expect(within(screen.getByTestId('recording-player')).getAllByRole('heading', { name: 'Transcript' })).toHaveLength(1);
  });

  it('follows the line being said, and waits after the reader scrolled by hand', async () => {
    const regions: TextRegionDto[] = Array.from({ length: 60 }, (_, i) => ({ id: `p${i}`, area: null, rotation: 0, text: `Line ${i}`, startMs: i * 5000, endMs: i * 5000 + 4000 }));
    const m = media('rec', { mime: 'audio/mp4', transcription: regions, width: null, height: null });
    const api = new MockApi(fixtureTree(5));
    const view = renderWith(TranscriptPanel, { media: m, now: 0, onSeek: vi.fn(), onSaved: vi.fn() }, { api });
    const list = screen.getByTestId('transcript-list');
    // jsdom has no layout: each row gets a 40 px slot in a 200 px list.
    Object.defineProperty(list, 'clientHeight', { value: 200, configurable: true });
    Object.defineProperty(list, 'scrollHeight', { value: 2400, configurable: true });
    list.getBoundingClientRect = () => ({ top: 0, bottom: 200, left: 0, right: 100, width: 100, height: 200, x: 0, y: 0, toJSON: () => ({}) });
    list.querySelectorAll<HTMLElement>('li').forEach((row, i) => {
      row.getBoundingClientRect = () => {
        const top = i * 40 - list.scrollTop;
        return { top, bottom: top + 40, left: 0, right: 100, width: 100, height: 40, x: 0, y: top, toJSON: () => ({}) };
      };
    });
    const at = (ms: number) => view.rerender({ props: { props: { media: m, now: ms, onSeek: vi.fn(), onSaved: vi.fn() } } });
    await at(100_000); // line 20
    await waitFor(() => expect(list.scrollTop).toBeGreaterThan(0));
    const followed = list.scrollTop;
    // The reader scrolls by hand: the list stays where they put it while the recording plays on.
    await fireEvent.wheel(list);
    list.scrollTop = 0;
    await at(200_000);
    await new Promise((r) => setTimeout(r, 30));
    expect(list.scrollTop).toBe(0);
    expect(followed).toBeGreaterThan(0);
  });
});
