import { describe, expect, it } from 'vitest';
import type { ChartNode } from '../types';
import { MockApi } from './MockApi';
import { fixtureTree } from './fixtures';

const BIG = { x: -1e6, y: -1e6, w: 2e6, h: 2e6 };

function overlaps(nodes: ChartNode[]): number {
  let n = 0;
  for (let i = 0; i < nodes.length; i++)
    for (let j = i + 1; j < nodes.length; j++) {
      const a = nodes[i];
      const b = nodes[j];
      if (a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h) n++;
    }
  return n;
}

describe('mock chart around a person with side branches opened', () => {
  const fx = fixtureTree(500);
  const api = new MockApi(fx);

  async function windowOf(req: Parameters<MockApi['chartOpen']>[0]) {
    const m = await api.chartOpen(req);
    return { m, nodes: (await api.chartWindow(m.chartId, BIG, 1)).nodes };
  }

  /** A root with an ancestor whose brothers and sisters can be opened. */
  async function rootWithOpenable() {
    for (const p of fx.persons.slice(0, 300)) {
      const { nodes } = await windowOf({ kind: 'around', root: p.id, generations: 255, kin: 'line' });
      const anc = nodes.find((n) => n.canOpenSiblings && n.person?.id !== p.id);
      if (anc?.person) return { root: p.id, anc: anc.person.id, line: nodes };
    }
    throw new Error('the fixture has nobody with an ancestor who has brothers or sisters');
  }

  it('empty_openings_are_the_same_chart_as_none', async () => {
    const { root } = await rootWithOpenable();
    for (const kin of ['line', 'siblings', 'all'] as const) {
      const a = await windowOf({ kind: 'around', root, generations: 255, kin });
      const b = await windowOf({ kind: 'around', root, generations: 255, kin, open: [] });
      expect(b.nodes.map((n) => [n.person?.id, n.x, n.y])).toEqual(a.nodes.map((n) => [n.person?.id, n.x, n.y]));
    }
  });

  it('opening_an_ancestors_siblings_adds_people_marks_the_card_and_nothing_overlaps', async () => {
    const { root, anc, line } = await rootWithOpenable();
    const kids = await windowOf({ kind: 'around', root, generations: 255, kin: 'line', open: [{ person: anc, siblings: 'children' }] });
    const all = await windowOf({ kind: 'around', root, generations: 255, kin: 'line', open: [{ person: anc, siblings: 'all' }] });
    expect(kids.nodes.length).toBeGreaterThan(line.length);
    expect(all.nodes.length).toBeGreaterThanOrEqual(kids.nodes.length);
    expect(overlaps(kids.nodes)).toBe(0);
    expect(overlaps(all.nodes)).toBe(0);
    expect(kids.nodes.find((n) => n.person?.id === anc)?.opened).toBe(true);
    expect(line.every((n) => !n.opened)).toBe(true);
  });

  it('partners_get_no_controls', async () => {
    const { root } = await rootWithOpenable();
    const { nodes } = await windowOf({ kind: 'around', root, generations: 255, kin: 'line' });
    for (const n of nodes.filter((x) => (x.relation ?? '').includes('partner') || /husband|wife/.test(x.relation ?? ''))) {
      expect(n.canOpenSiblings, `${n.person?.name}`).toBeUndefined();
      expect(n.canOpenChildren).toBeUndefined();
    }
  });
});
