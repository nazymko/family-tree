import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/svelte';
import OutlineMirror from './OutlineMirror.svelte';
import Host from './testing/Host.svelte';
import { makeApi, richestRoot } from './testing/helpers';

// The expand glyph is aria-hidden but part of the text content.
const lead = '^[\\s\\u25b8\\u25be]*';

async function setup(kind: 'pedigree' | 'descendants' = 'pedigree') {
  const { api, fx } = makeApi();
  const root = await richestRoot(api, fx, kind);
  const meta = await api.chartOpen({ kind, root, generations: 5 });
  const spy = vi.spyOn(api, 'chartOutline');
  const onPick = vi.fn();
  render(Host, { props: { api, view: OutlineMirror, props: { chartId: meta.chartId, onPick } } });
  await screen.findAllByRole('treeitem');
  return { api, root, spy, onPick };
}

describe('OutlineMirror', () => {
  it('tree_roles_and_aria_expanded', async () => {
    await setup();
    const tree = screen.getByRole('tree');
    const [first] = within(tree).getAllByRole('treeitem');
    expect(first).toHaveAttribute('aria-level', '1');
    expect(first).toHaveAttribute('aria-expanded', 'false');
    first.focus();
    await fireEvent.keyDown(first, { key: 'ArrowRight' });
    await waitFor(() => expect(first).toHaveAttribute('aria-expanded', 'true'));
    expect(within(first).getByRole('group')).toBeInTheDocument();
    await fireEvent.keyDown(first, { key: 'ArrowLeft' });
    expect(first).toHaveAttribute('aria-expanded', 'false');
  });

  it('lazy_loads_children_on_expand', async () => {
    const { spy } = await setup();
    expect(spy).toHaveBeenCalledTimes(1);
    const [first] = screen.getAllByRole('treeitem');
    await fireEvent.keyDown(first, { key: 'ArrowRight' });
    await waitFor(() => expect(first).toHaveAttribute('aria-expanded', 'true'));
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[1][1]).toBe(0);
    await fireEvent.keyDown(first, { key: 'ArrowLeft' });
    await fireEvent.keyDown(first, { key: 'ArrowRight' });
    await waitFor(() => expect(first).toHaveAttribute('aria-expanded', 'true'));
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('enter_picks_person', async () => {
    const { onPick, root } = await setup();
    const [first] = screen.getAllByRole('treeitem');
    await fireEvent.keyDown(first, { key: 'Enter' });
    expect(onPick).toHaveBeenCalledWith(root, expect.any(String));
  });

  it('ahnentafel_labels_in_pedigree', async () => {
    await setup('pedigree');
    const [first] = screen.getAllByRole('treeitem');
    expect(first.textContent).toMatch(new RegExp(`${lead}1 Starting person`));
    await fireEvent.keyDown(first, { key: 'ArrowRight' });
    await waitFor(() => expect(within(first).getAllByRole('treeitem').length).toBeGreaterThanOrEqual(2));
    const kids = within(first).getAllByRole('treeitem');
    expect(kids[0].textContent).toMatch(new RegExp(`${lead}2 Father`));
    expect(kids[1].textContent).toMatch(new RegExp(`${lead}3 Mother`));
  });

  it('arrow_down_moves_between_visible_items', async () => {
    await setup();
    const [first] = screen.getAllByRole('treeitem');
    first.focus();
    await fireEvent.keyDown(first, { key: 'ArrowRight' });
    await waitFor(() => expect(first).toHaveAttribute('aria-expanded', 'true'));
    await fireEvent.keyDown(first, { key: 'ArrowDown' });
    const second = screen.getAllByRole('treeitem')[1];
    await waitFor(() => expect(document.activeElement).toBe(second));
  });
});

describe('OutlineMirror selection', () => {
  it('aria_selected_follows_selected_prop_and_focus_reports_select', async () => {
    const { api, fx } = makeApi();
    const root = await richestRoot(api, fx, 'pedigree');
    const meta = await api.chartOpen({ kind: 'pedigree', root, generations: 5 });
    const onSelect = vi.fn();
    render(Host, { props: { api, view: OutlineMirror, props: { chartId: meta.chartId, onPick: vi.fn(), onSelect, selected: null } } });
    const items = await screen.findAllByRole('treeitem');
    expect(items.some((i) => i.getAttribute('aria-selected') === 'true')).toBe(false);
    items[0].focus();
    expect(onSelect).toHaveBeenCalled();
    expect(onSelect.mock.calls[0][0]).toEqual(expect.any(Number));
  });
});
