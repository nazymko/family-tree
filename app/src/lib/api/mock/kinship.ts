import type { Id, Kinship, KinshipKind, ParentRelId, SexKind } from '../types';
import type { FamilyRec } from './model';

// TypeScript port of crates/gt-session/src/kinship.rs: how one person is related to another, as data.
// Keep the two in step (the Rust tests have the table of expected answers).

const MAX_UP = 8;
const MAX_ANCESTORS = 2000;

/** Ancestor -> [distance in generations, family it was reached through ('' for the person itself)]. */
type Anc = Map<Id, [number, string]>;
type Blood = { up: number; down: number; half: boolean };

const countsAsParent = (r: ParentRelId) => r === 'birth' || r === 'adopted' || r === 'unknown';

export interface KinData {
  families(): Iterable<FamilyRec>;
  sex(id: Id): SexKind;
}

/** A finder for one reference person; answers are remembered. */
export function kinshipFinder(data: KinData, from: Id): (to: Id) => Kinship | null {
  const fams = [...data.families()];
  const childIn = new Map<Id, FamilyRec[]>();
  const partnerIn = new Map<Id, FamilyRec[]>();
  for (const f of fams) {
    for (const c of f.children) childIn.set(c.person, [...(childIn.get(c.person) ?? []), f]);
    for (const p of [f.partner1, f.partner2]) if (p) partnerIn.set(p, [...(partnerIn.get(p) ?? []), f]);
  }

  const parents = (k: Id): [Id, string][] => {
    const out: [Id, string][] = [];
    for (const f of childIn.get(k) ?? []) {
      const c = f.children.find((x) => x.person === k);
      if (!c) continue;
      if (f.partner1 && countsAsParent(c.rel[0])) out.push([f.partner1, f.id]);
      if (f.partner2 && countsAsParent(c.rel[1])) out.push([f.partner2, f.id]);
    }
    return out;
  };

  const partners = (k: Id): [Id, boolean][] => {
    const out: [Id, boolean][] = [];
    for (const f of partnerIn.get(k) ?? []) {
      const other = f.partner1 === k ? f.partner2 : f.partner1;
      if (!other) continue;
      const married = f.kind === 'married' || f.kind === 'civil_union';
      const hit = out.find((e) => e[0] === other);
      if (hit) hit[1] ||= married;
      else out.push([other, married]);
    }
    return out;
  };

  const ancestors = (start: Id): Anc => {
    const map: Anc = new Map([[start, [0, '']]]);
    let frontier = [start];
    for (let d = 1; d <= MAX_UP; d++) {
      const next: Id[] = [];
      for (const k of frontier)
        for (const [p, fam] of parents(k))
          if (!map.has(p)) {
            map.set(p, [d, fam]);
            next.push(p);
          }
      if (next.length === 0 || map.size >= MAX_ANCESTORS) break;
      frontier = next;
    }
    return map;
  };

  const blood = (a: Anc, b: Anc): Blood | null => {
    let best: [number, number, number, Id] | null = null; // sum, imbalance, up, ancestor
    for (const [x, [d1]] of a) {
      const e = b.get(x);
      if (!e) continue;
      const cand: [number, number, number, Id] = [d1 + e[0], Math.abs(d1 - e[0]), d1, x];
      if (!best || cand[0] < best[0] || (cand[0] === best[0] && (cand[1] < best[1] || (cand[1] === best[1] && cand[3] < best[3])))) best = cand;
    }
    if (!best) return null;
    const [sum, , up, anc] = best;
    const down = sum - up;
    let half = false;
    if (up > 0 && down > 0) {
      let same = 0;
      for (const [x, [d1]] of a) if (d1 === up && b.get(x)?.[0] === down) same++;
      if (same === 1) half = a.get(anc)?.[1] !== b.get(anc)?.[1];
    }
    return { up, down, half };
  };

  const make = (kind: KinshipKind, b: Blood | null, sex: SexKind, via: SexKind): Kinship => ({
    kind,
    up: b?.up ?? 0,
    down: b?.down ?? 0,
    half: b?.half ?? false,
    married: false,
    sex,
    via,
  });

  const fromAnc = ancestors(from);
  const fromPartners = partners(from).map(([p, m]) => [p, m, ancestors(p)] as const);
  const memo = new Map<Id, Kinship | null>();

  const compute = (to: Id): Kinship | null => {
    const sex = data.sex(to);
    if (to === from) return make('self_person', null, sex, 'unknown');
    const toAnc = ancestors(to);
    let best: { key: [number, number]; k: Kinship } | null = null;
    const offer = (cls: number, cost: number, k: Kinship) => {
      if (!best || cls < best.key[0] || (cls === best.key[0] && cost < best.key[1])) best = { key: [cls, cost], k };
    };
    const b = blood(fromAnc, toAnc);
    if (b) offer(b.up + b.down === 1 ? 0 : 2, b.up + b.down, make('blood', b, sex, 'unknown'));
    const direct = fromPartners.find((p) => p[0] === to);
    if (direct) offer(1, 0, { ...make('partner', null, sex, 'unknown'), married: direct[1] });
    for (const [q, married] of partners(to)) {
      if (q === from) continue;
      const bq = blood(fromAnc, ancestors(q));
      if (bq) offer(3, (bq.up + bq.down) * 2, { ...make('partner_of_relative', bq, sex, data.sex(q)), married });
    }
    for (const [p, married, pAnc] of fromPartners) {
      if (p === to) continue;
      const bp = blood(pAnc, toAnc);
      if (bp) offer(3, (bp.up + bp.down) * 2 + 1, { ...make('relative_of_partner', bp, sex, data.sex(p)), married });
    }
    return best ? (best as { k: Kinship }).k : null;
  };

  return (to) => {
    if (!memo.has(to)) memo.set(to, compute(to));
    return memo.get(to) ?? null;
  };
}
