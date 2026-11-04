import { describe, expect, it } from 'vitest';
import {
  acceptAll,
  acceptCard,
  activeCards,
  addCard,
  addLink,
  clickBox,
  buildFamilies,
  counts,
  newDraft,
  parseDraft,
  rejectAll,
  removeCard,
  toCommand,
  type TraceDraft,
} from './model';

const box = { x: 0.1, y: 0.1, w: 0.1, h: 0.05 };

function people(d: TraceDraft, ...names: string[]): string[] {
  return names.map((n) => {
    const c = addCard(d, box)!;
    c.given = n;
    c.surname = 'Koval';
    return c.id;
  });
}

describe('trace model', () => {
  it('marks_a_person_inside_the_picture', () => {
    const d = newDraft();
    const c = addCard(d, { x: 0.95, y: -1, w: 0.3, h: 0.04 })!;
    expect(c.x + c.w).toBeLessThanOrEqual(1);
    expect(c.y).toBe(0);
    expect(d.cards).toHaveLength(1);
  });

  it('groups_two_partners_and_their_children_into_one_family', () => {
    const d = newDraft();
    const [ivan, olena, petro, maria] = people(d, 'Ivan', 'Olena', 'Petro', 'Maria');
    addLink(d, 'partner', ivan, olena);
    addLink(d, 'parent', ivan, petro);
    addLink(d, 'parent', olena, petro);
    addLink(d, 'child', maria, ivan); // Maria is a child of Ivan only
    const { families, problems } = buildFamilies(d);
    expect(problems).toEqual([]);
    expect(families).toHaveLength(1);
    expect(families[0]).toEqual({ partner1: ivan, partner2: olena, children: [petro, maria] });
  });

  it('a_couple_without_children_is_still_a_family', () => {
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    addLink(d, 'partner', a, b);
    expect(buildFamilies(d).families).toEqual([{ partner1: a, partner2: b, children: [] }]);
  });

  it('a_single_parent_with_several_partners_gets_a_family_of_their_own', () => {
    const d = newDraft();
    const [a, b, c, kid] = people(d, 'Ivan', 'Olena', 'Hanna', 'Petro');
    addLink(d, 'partner', a, b);
    addLink(d, 'partner', a, c);
    addLink(d, 'parent', a, kid);
    const fams = buildFamilies(d).families;
    expect(fams).toHaveLength(3);
    expect(fams.find((f) => f.children.length === 1)).toEqual({ partner1: a, children: [kid] });
  });

  it('refuses_nonsense_links', () => {
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    expect(addLink(d, 'partner', a, a).ok).toBe(false);
    expect(addLink(d, 'parent', a, b).ok).toBe(true);
    expect(addLink(d, 'parent', a, b).ok).toBe(false);
    expect(addLink(d, 'child', a, b).ok).toBe(false); // the other way round
    expect(addLink(d, 'partner', a, 'nope').ok).toBe(false);
    expect(d.links).toHaveLength(1);
  });

  it('three_parents_and_unnamed_cards_block_adding', () => {
    const d = newDraft();
    const [a, b, c, kid] = people(d, 'A', 'B', 'C', 'D');
    for (const p of [a, b, c]) addLink(d, 'parent', p, kid);
    addCard(d, box);
    const { problems } = buildFamilies(d);
    expect(problems).toHaveLength(2);
    expect(problems.join(' ')).toMatch(/more than two parents/);
    expect(problems.join(' ')).toMatch(/no name/);
  });

  it('removing_a_card_removes_its_links', () => {
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    addLink(d, 'partner', a, b);
    removeCard(d, a);
    expect(d.links).toEqual([]);
  });

  it('builds_the_command_with_matched_people_and_the_picture_flag', () => {
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    d.cards[0].patronymic = 'Petrovych';
    d.cards[0].birth = '1901';
    d.cards[1].existing = { id: 'x-1', name: 'Olena Koval' };
    addLink(d, 'partner', a, b);
    d.keepPicture = false;
    const cmd = toCommand(d);
    expect(cmd.attachPicture).toBe(false);
    expect(cmd.persons).toEqual([
      { key: a, given: 'Ivan', surname: 'Koval', patronymic: 'Petrovych', sex: 'unknown', birthDateRaw: '1901' },
      { key: b, existing: 'x-1', sex: 'unknown' },
    ]);
    expect(cmd.families).toEqual([{ partner1: a, partner2: b, children: [] }]);
    expect(counts(d)).toMatchObject({ newPeople: 1, matched: 1, families: 1, links: 1 });
  });

  it('suggestions_are_left_out_until_accepted', () => {
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    addLink(d, 'partner', a, b);
    d.cards[1].suggested = true;
    d.links[0].suggested = true;
    expect(activeCards(d)).toHaveLength(1);
    expect(toCommand(d).persons).toHaveLength(1);
    expect(toCommand(d).families).toEqual([]);
    expect(counts(d).suggestions).toBe(2);
    acceptCard(d, b);
    expect(d.cards[1].suggested).toBe(false);
    expect(d.links[0].suggested).toBe(false); // its other end was already accepted
    expect(toCommand(d).families).toHaveLength(1);
  });

  it('accept_all_and_reject_all', () => {
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    addLink(d, 'partner', a, b);
    d.cards[1].suggested = true;
    d.links[0].suggested = true;
    const copy = structuredClone(d);
    rejectAll(copy);
    expect(copy.cards).toHaveLength(1);
    expect(copy.links).toEqual([]);
    acceptAll(d);
    expect(counts(d).suggestions).toBe(0);
  });

  it('reads_a_saved_draft_and_skips_what_is_broken', () => {
    expect(parseDraft(null)).toBeNull();
    expect(parseDraft('not json')).toBeNull();
    expect(parseDraft('[]')?.cards).toEqual([]);
    const d = newDraft();
    const [a, b] = people(d, 'Ivan', 'Olena');
    addLink(d, 'partner', a, b);
    const round = parseDraft(JSON.stringify(d))!;
    expect(round.cards).toHaveLength(2);
    expect(round.links).toHaveLength(1);
    expect(round.nextId).toBe(d.nextId);
    const broken = parseDraft(
      JSON.stringify({
        cards: [{ id: 'c1', x: 'a', given: 5, sex: 'robot' }, null, { id: 'c1' }, { id: 'c2', given: 'Ok' }],
        links: [{ id: 'l1', kind: 'parent', from: 'c1', to: 'zz' }, { id: 'l2', kind: 'sibling', from: 'c1', to: 'c2' }, { id: 'l3', kind: 'parent', from: 'c1', to: 'c2' }],
      }),
    )!;
    expect(broken.cards.map((c) => c.id)).toEqual(['c1', 'c2']);
    expect(broken.cards[0].sex).toBe('unknown');
    expect(broken.cards[0].given).toBe('');
    expect(broken.cards[0].x).toBeGreaterThanOrEqual(0);
    expect(broken.links.map((l) => l.id)).toEqual(['l3']);
    expect(broken.nextId).toBeGreaterThan(3);
  });

  it('asks_which_family_when_the_one_parent_is_in_several_and_passes_the_choice', () => {
    const d = newDraft();
    const [p, k] = people(d, 'Ivan', 'Petro');
    const parent = d.cards.find((c) => c.id === p)!;
    parent.existing = { id: 'x1', name: 'Ivan Koval', families: [{ id: 'f1', label: 'With A' }, { id: 'f2', label: 'With B' }] };
    addLink(d, 'parent', p, k);
    expect(buildFamilies(d).problems.join(' ')).toMatch(/more than one family/);
    parent.existing.familyId = 'f2';
    const built = buildFamilies(d);
    expect(built.problems).toEqual([]);
    expect(built.families[0]).toMatchObject({ partner1: p, existingFamily: 'f2', children: [k] });
    // With only one family there is nothing to ask.
    parent.existing = { id: 'x1', name: 'Ivan Koval', families: [{ id: 'f1', label: 'With A' }] };
    expect(buildFamilies(d).problems).toEqual([]);
  });
});

describe('clickBox', () => {
  it('is_about_18_by_7_percent_within_pixel_limits', () => {
    const mid = clickBox(1000, 800);
    expect(mid.w).toBeCloseTo(0.18, 3);
    expect(mid.h).toBeCloseTo(0.07, 3);
    const big = clickBox(6000, 4000);
    expect(big.w * 6000).toBe(360);
    expect(big.h * 4000).toBe(120);
    const small = clickBox(200, 150);
    expect(small.w * 200).toBe(60);
    expect(small.h * 150).toBe(24);
  });
});
