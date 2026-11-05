import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import PersonCard from '../ui/PersonCard.svelte';
import { makeFakeContext } from '../../test-setup';
import { draw, type Theme } from '../chart/renderer';
import type { ChartNode, ChartWindow } from '../api/types';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';
import { setLanguage, t } from '../i18n';

beforeEach(() => setLanguage('en'));

describe('the unnamed-siblings marker', () => {
  it('shows_translated_label', () => {
    expect(t('person.siblingMarker')).toBe('<Some more Sisters/Brothers>');
    const { unmount } = render(PersonCard, { name: '', siblingMarker: true });
    expect(screen.getByText('<Some more Sisters/Brothers>')).toBeTruthy();
    // No page to open: nothing to click but, when offered, Delete.
    expect(screen.queryByRole('button')).toBeNull();
    unmount();

    setLanguage('uk');
    expect(t('person.siblingMarker')).toBe('<Ще сестри/брати>');
    render(PersonCard, { name: '', siblingMarker: true, ondelete: () => {} });
    expect(screen.getByText('<Ще сестри/брати>')).toBeTruthy();
    expect(screen.getAllByRole('button')).toHaveLength(1);
  });

  const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };

  it('chart_card_has_no_dates', () => {
    const marker: ChartNode = {
      id: 1,
      person: { id: 'm1', name: '', lifespan: '', sex: 'unknown', living: true, private: false, siblingMarker: true },
      x: 0,
      y: 0,
      w: 240,
      h: 56,
      gen: 1,
      ahnen: 3,
      dupOf: null,
      moreUp: true,
      moreDown: true,
      nav: { parent: null, child: null, prev: null, next: null },
      label: '',
      kinship: { kind: 'blood', up: 0, down: 0, half: false, married: false, sex: 'unknown', via: 'unknown' },
    };
    const win: ChartWindow = { nodes: [marker], edges: [] };
    const ctx = makeFakeContext();
    draw(ctx, win, { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, focus: null, theme });
    const texts = ctx.calls.filter((c) => c.op === 'fillText').map((c) => c.args[0]);
    // Only the label: no name, no dates, no ancestor number, no relationship.
    expect(texts).toEqual(['<Some more Sisters/Brothers>']);
    // Dashed outline, and no "add relative" handle.
    expect(ctx.calls.some((c) => c.op === 'setLineDash' && (c.args[0] as number[]).length > 0)).toBe(true);
    expect(ctx.calls.filter((c) => c.op === 'arc')).toHaveLength(0);
  });

  it('the_mock_keeps_it_out_of_lists_and_counts', async () => {
    const api = new MockApi(fixtureTree(30));
    const info = await api.openProject('C:/Genealogy/Test');
    const view = await api.getFamilyView(info.homePerson!);
    const fam = view.partnerships[0]?.familyId ?? view.parentSets[0]!.familyId;
    const before = (await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1)).total;
    const r = await api.apply({ type: 'addSiblingMarker', family: fam });
    expect(r.label).toBe('Add unnamed brothers and sisters');
    const f = await api.getFamily(fam);
    expect(f.children.some((c) => c.person.siblingMarker && c.person.name === '' && c.person.lifespan === '')).toBe(true);
    const after = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 1);
    expect(after.total).toBe(before);
    await expect(api.apply({ type: 'addSiblingMarker', family: fam })).rejects.toMatchObject({ code: 'conflict' });
  });
  it('the_mock_counts_refuses_and_reports_not_living', async () => {
    const api = new MockApi(fixtureTree(30));
    const info = await api.openProject('C:/Genealogy/Test');
    const view = await api.getFamilyView(info.homePerson!);
    const fam = view.partnerships[0]?.familyId ?? view.parentSets[0]!.familyId;
    const before = await api.chartOpenWhole();
    await api.apply({ type: 'addSiblingMarker', family: fam });
    const marker = (await api.getFamily(fam)).children.find((c) => c.person.siblingMarker)!.person;
    expect(marker.living).toBe(false);
    const after = await api.chartOpenWhole();
    expect(after.meta.nodeCount).toBe(before.meta.nodeCount);
    expect(after.blocks.map((x) => x.people)).toEqual(before.blocks.map((x) => x.people));
    await expect(api.getFamilyView(marker.id)).rejects.toMatchObject({ code: 'not_found', message: expect.stringContaining('unnamed brothers and sisters') });
    await expect(api.getPerson(marker.id)).rejects.toMatchObject({ code: 'not_found', message: expect.stringContaining('no details') });
    await expect(api.chartOpen({ kind: 'descendants', root: marker.id, generations: 3 })).rejects.toMatchObject({ code: 'not_found' });
  });
});
