import { describe, expect, it, vi, beforeEach } from 'vitest';
import { screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import CitationEditor from './CitationEditor.svelte';
import CitationList from './CitationList.svelte';
import { renderWith, ALL_PERSONS } from '../testing/renderWith';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { notices } from '../ui/notices.svelte';
import type { OwnerRef } from '../api/api';

beforeEach(() => notices.dismiss());

async function prepare(withLast: boolean) {
  const api = new MockApi(fixtureTree(80));
  const id = (await api.listPersons(ALL_PERSONS, 0, 5)).items[0].id;
  const target: OwnerRef = { kind: 'person', id };
  let cit;
  if (withLast) {
    const src = (await api.suggestSources('', 1))[0];
    const r = await api.apply({ type: 'addCitation', target, citation: { source: src.id, page: 'p. 7' } });
    cit = (await api.getPerson(id)).citations.find((c) => c.id === r.touched[1])!;
  }
  const spy = vi.spyOn(api, 'apply');
  return { api, target, spy, cit };
}

async function mountEditor(withLast = false) {
  const p = await prepare(withLast);
  const onDone = vi.fn();
  const onCancel = vi.fn();
  renderWith(CitationEditor, { target: p.target, onDone, onCancel }, { api: p.api });
  return { ...p, onDone, onCancel };
}

async function createSource(user: ReturnType<typeof userEvent.setup>, title: string) {
  await user.type(screen.getByRole('combobox', { name: 'Source' }), title);
  await user.click(await screen.findByRole('option', { name: `Create source "${title}"` }));
}

describe('CitationEditor', () => {
  it('reuse_last_adds_in_one_command', async () => {
    const user = userEvent.setup();
    const { spy, target, onDone } = await mountEditor(true);
    const btn = screen.getByRole('button', { name: 'Use the last citation' });
    await waitFor(() => expect(btn).not.toHaveAttribute('aria-disabled', 'true'));
    spy.mockClear();
    await user.click(btn);
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith({ type: 'addCitation', target, citation: { reuseLast: true } }, { silent: true });
    expect(notices.current?.message).toBe('Citation added');
  });

  it('reuse_last_disabled_when_none', async () => {
    const user = userEvent.setup();
    const { spy } = await mountEditor();
    const btn = screen.getByRole('button', { name: 'Use the last citation' });
    expect(btn).toHaveAttribute('aria-disabled', 'true');
    expect(screen.getByText('No citation yet')).toBeInTheDocument();
    await user.click(btn);
    expect(spy).not.toHaveBeenCalled();
  });

  it('new_source_inline_creates_with_title', async () => {
    const user = userEvent.setup();
    const { spy, target } = await mountEditor();
    await createSource(user, '1881 Census');
    await user.click(screen.getByRole('button', { name: 'Add citation' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(spy.mock.calls[0][0]).toMatchObject({ type: 'addCitation', target, citation: { source: { title: '1881 Census' } } });
  });

  it('quality_labels_map_to_numbers', async () => {
    const user = userEvent.setup();
    const { spy } = await mountEditor();
    await createSource(user, 'Parish register');
    await user.selectOptions(screen.getByRole('combobox', { name: 'Quality' }), 'Secondary evidence');
    await user.click(screen.getByRole('button', { name: 'Add citation' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const cmd = spy.mock.calls[0][0] as { citation: { quality?: number } };
    expect(cmd.citation.quality).toBe(2);
  });

  it('evidence_group_collapsed_by_default_and_tristate_payload', async () => {
    const user = userEvent.setup();
    const { spy } = await mountEditor();
    const details = screen.getByText('Evidence analysis').closest('details')!;
    expect(details.open).toBe(false);
    await createSource(user, 'Deed');
    await user.selectOptions(screen.getByLabelText('Original source or derivative'), 'Derivative');
    await user.click(screen.getByRole('button', { name: 'Add citation' }));
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const ev = (spy.mock.calls[0][0] as { citation: { evidence?: Record<string, boolean> } }).citation.evidence!;
    expect(ev.original).toBe(false);
    expect(ev.primaryInfo).toBeUndefined();
    expect('direct' in ev).toBe(false);
  });

  it('mod_shift_c_shortcut_inside_dialog', async () => {
    const user = userEvent.setup();
    const { spy, onDone } = await mountEditor(true);
    await waitFor(() => expect(screen.getByRole('button', { name: 'Use the last citation' })).not.toHaveAttribute('aria-disabled', 'true'));
    spy.mockClear();
    await user.keyboard('{Control>}{Shift>}c{/Shift}{/Control}');
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][0]).toMatchObject({ citation: { reuseLast: true } });
  });

  it('clone_sends_cloneOf_and_focuses_page_field', async () => {
    const user = userEvent.setup();
    const { api, target, spy, cit } = await prepare(true);
    const onChanged = vi.fn();
    renderWith(CitationList, { target, citations: [cit!], onChanged }, { api });
    spy.mockClear();
    await user.click(screen.getByRole('button', { name: /^Clone citation/ }));
    await waitFor(() => expect(spy).toHaveBeenCalledWith({ type: 'addCitation', target, citation: { cloneOf: cit!.id } }));
    expect(onChanged).toHaveBeenCalled();
    await screen.findByRole('dialog', { name: 'Edit citation' });
    await waitFor(() => expect(screen.getByRole('textbox', { name: 'Page / entry' })).toHaveFocus());
    expect(screen.getByRole('textbox', { name: 'Page / entry' })).toHaveValue('p. 7');
  });
});
