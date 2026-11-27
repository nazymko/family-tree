import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import Host from './testing/Host.svelte';
import { attachContext, fakeRaf, makeApi, richestRoot } from './testing/helpers';
import { setLanguage } from '../i18n';
import { nav } from '../state/nav.svelte';
import Around from '../views/Around.svelte';
import Descendants from '../views/Descendants.svelte';
import { BRANCHES_KEY, branchesPref } from './branchesPref.svelte';
import { BRANCH_WOOD } from './renderer';

beforeEach(() => {
  nav.reset();
  try {
    localStorage.removeItem(BRANCHES_KEY);
    localStorage.removeItem('gt.chartView');
  } catch {
    // ignore
  }
  branchesPref.reload();
});
afterEach(() => {
  vi.unstubAllGlobals();
  setLanguage('en');
});

async function mountAround() {
  const raf = fakeRaf();
  const { api, fx } = makeApi();
  const id = await richestRoot(api, fx, 'around');
  nav.go({ name: 'whole' });
  nav.go({ name: 'around', ids: [id] });
  render(Host, { props: { api, view: Around } });
  const canvas = (await screen.findByRole('application')) as HTMLCanvasElement;
  const ctx = attachContext(canvas);
  return { raf, ctx, canvas };
}

const wood = (ctx: ReturnType<typeof attachContext>) => ctx.calls.some((c) => c.op === 'set:strokeStyle' && c.args[0] === BRANCH_WOOD);

describe('the branches toggle on the chart built around people', () => {
  it('is_off_by_default_and_leaves_the_connector_lines', async () => {
    const s = await mountAround();
    const box = screen.getByRole('checkbox', { name: 'Branches' });
    expect(box).not.toBeChecked();
    expect(box.closest('.tool')).toHaveAttribute('title', 'Show families as branches');
    await waitFor(() => {
      s.raf.pump(2);
      expect(s.ctx.calls.some((c) => c.op === 'stroke')).toBe(true);
    });
    expect(wood(s.ctx)).toBe(false);
  });

  it('switches_the_branches_on_and_off_and_remembers_the_choice', async () => {
    const s = await mountAround();
    const box = screen.getByRole('checkbox', { name: 'Branches' });
    await fireEvent.click(box);
    expect(box).toBeChecked();
    expect(localStorage.getItem(BRANCHES_KEY)).toBe('1');
    await waitFor(() => {
      s.raf.pump(2);
      expect(wood(s.ctx)).toBe(true);
    });
    // A new session reads the stored choice.
    branchesPref.value = false;
    branchesPref.reload();
    expect(branchesPref.value).toBe(true);
    await fireEvent.click(box);
    expect(box).not.toBeChecked();
    expect(localStorage.getItem(BRANCHES_KEY)).toBe('0');
    s.ctx.calls.length = 0;
    await waitFor(() => {
      s.raf.pump(2);
      expect(s.ctx.calls.some((c) => c.op === 'stroke')).toBe(true);
    });
    expect(wood(s.ctx)).toBe(false);
  });

  it('is_not_offered_on_the_other_charts', async () => {
    fakeRaf();
    const { api, fx } = makeApi();
    const id = await richestRoot(api, fx, 'descendants');
    nav.go({ name: 'whole' });
    nav.focus(id);
    render(Host, { props: { api, view: Descendants } });
    await screen.findByRole('application');
    expect(screen.queryByRole('checkbox', { name: 'Branches' })).toBeNull();
  });

  it('has_its_words_in_ukrainian', async () => {
    setLanguage('uk');
    await mountAround();
    const box = screen.getByRole('checkbox', { name: 'Гілки' });
    expect(box.closest('.tool')).toHaveAttribute('title', 'Показати родини як гілки');
  });
});
