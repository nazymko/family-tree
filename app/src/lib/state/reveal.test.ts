import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { REVEAL_CLASS, revealRecord } from './reveal';

const KINDS = ['person', 'family', 'event', 'name', 'note', 'media', 'citation', 'source'];

let scroll: ReturnType<typeof vi.fn>;

beforeEach(() => {
  vi.useFakeTimers();
  scroll = vi.fn();
  HTMLElement.prototype.scrollIntoView = scroll as unknown as typeof HTMLElement.prototype.scrollIntoView;
  vi.stubGlobal('matchMedia', (q: string) => ({ matches: false, media: q }));
});
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
  document.body.innerHTML = '';
});

describe('revealRecord', () => {
  it.each(KINDS)('brings a %s into view, marks it and moves focus to it', (kind) => {
    document.body.innerHTML = `<main><h1>Page</h1><ul><li data-${kind}-id="x1">Row</li></ul></main>`;
    const el = document.querySelector<HTMLElement>(`[data-${kind}-id="x1"]`)!;
    revealRecord(kind, 'x1');
    vi.advanceTimersByTime(100);
    expect(el.classList.contains(REVEAL_CLASS)).toBe(true);
    expect(scroll).toHaveBeenCalledWith({ block: 'center', behavior: 'smooth' });
    expect(document.activeElement).toBe(el);
    vi.advanceTimersByTime(2600);
    expect(el.classList.contains(REVEAL_CLASS)).toBe(false);
  });

  it('ends the mark when the glow animation ends, before the timer', () => {
    document.body.innerHTML = '<table><tbody><tr data-citation-id="c1"><td>x</td></tr></tbody></table>';
    const el = document.querySelector<HTMLElement>('tr')!;
    revealRecord('citation', 'c1');
    vi.advanceTimersByTime(100);
    expect(el.classList.contains(REVEAL_CLASS)).toBe(true);
    el.dispatchEvent(new Event('animationend'));
    expect(el.classList.contains(REVEAL_CLASS)).toBe(false);
  });

  it('does not touch another record with the same id of another kind', () => {
    document.body.innerHTML = '<p data-note-id="x1">note</p><p data-event-id="x1">event</p>';
    revealRecord('event', 'x1');
    vi.advanceTimersByTime(100);
    expect(document.querySelector('[data-note-id]')!.classList.contains(REVEAL_CLASS)).toBe(false);
    expect(document.querySelector('[data-event-id]')!.classList.contains(REVEAL_CLASS)).toBe(true);
  });

  it('opens a closed section around the record first', () => {
    document.body.innerHTML = '<details><summary>History</summary><div><p data-event-id="e1">Birth</p></div></details>';
    revealRecord('event', 'e1');
    vi.advanceTimersByTime(100);
    expect(document.querySelector('details')!.open).toBe(true);
  });

  it('waits for a page that is still loading', () => {
    revealRecord('media', 'm1');
    vi.advanceTimersByTime(1500);
    document.body.innerHTML = '<figure data-media-id="m1">Photo</figure>';
    vi.advanceTimersByTime(300);
    expect(document.querySelector('figure')!.classList.contains(REVEAL_CLASS)).toBe(true);
  });

  it('gives up quietly when the record never appears', () => {
    revealRecord('citation', 'nope');
    expect(() => vi.advanceTimersByTime(20000)).not.toThrow();
    expect(vi.getTimerCount()).toBe(0);
    expect(scroll).not.toHaveBeenCalled();
  });

  it('ignores a kind it does not know', () => {
    document.body.innerHTML = '<p data-place-id="x">place</p>';
    revealRecord('place', 'x');
    expect(vi.getTimerCount()).toBe(0);
  });

  it('scrolls without animation when the person asks for reduced motion', () => {
    vi.stubGlobal('matchMedia', (q: string) => ({ matches: q.includes('reduce'), media: q }));
    document.body.innerHTML = '<p data-name-id="n1">Name</p>';
    revealRecord('name', 'n1');
    vi.advanceTimersByTime(100);
    expect(scroll).toHaveBeenCalledWith({ block: 'center', behavior: 'auto' });
  });

  it('never moves focus into a field: it goes to the heading of the section', () => {
    document.body.innerHTML = '<section><h2>Names</h2><input data-name-id="n1" /></section>';
    revealRecord('name', 'n1');
    vi.advanceTimersByTime(100);
    expect(document.activeElement).toBe(document.querySelector('h2'));
  });

  it('leaves no tabindex behind once focus moves on', () => {
    document.body.innerHTML = '<p data-person-id="p1">Name</p><button id="b">b</button>';
    revealRecord('person', 'p1');
    vi.advanceTimersByTime(100);
    const el = document.querySelector<HTMLElement>('[data-person-id]')!;
    expect(el.getAttribute('tabindex')).toBe('-1');
    document.getElementById('b')!.focus();
    expect(el.hasAttribute('tabindex')).toBe(false);
  });
});
