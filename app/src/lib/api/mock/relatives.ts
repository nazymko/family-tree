import type { ChartEdge, Id, OutlineItem } from '../types';
import { M, MAX_NODES, bounds, dupEdge, newNode, strip, type N, type RustLayoutSource } from './layout';
import type { Layout } from './mockLayout';
import { OUTLINE_GROUP_BASE } from '../../chart/outline';
import { relationLabel, type PartnerOf, type Rel } from './relation';

// TypeScript port of crates/gt-layout/src/relatives.rs: an hourglass of blocks (one per ancestor), each with a row
// group (the person and their siblings, with partners and a small tree of children) hung under the blocks of the
// parents. Every block owns its own horizontal interval, so boxes never overlap.

export interface RelLayout extends Layout {
  rels: Rel[];
}

/** Outline rows from here up are group headings, not chart nodes (same value as the Rust side). */
export const GROUP_BASE = OUTLINE_GROUP_BASE;

interface Unit {
  person: number;
  partners: number;
  row: number;
  cousin: boolean;
  attach: number | null;
  children: number[];
  hang: [boolean, boolean];
  sw: number;
  ucx: number;
  left: number;
}

interface Block {
  row: number;
  anchor: number;
  group: number[];
  slots: [number | null, number | null];
  ups: number[];
  upOff: number[];
  w: number;
  acx: number;
  goff: number;
  left: number;
}

interface Sib {
  key: Id;
  hang: [boolean, boolean];
  half: boolean;
}

type Tree = { t: 'focus'; limit: number } | { t: 'niece' } | { t: 'aunt' };
const limitOf = (t: Tree): number => (t.t === 'focus' ? t.limit : t.t === 'niece' || t.t === 'aunt' ? 1 : 0);

function siblingsOf(src: RustLayoutSource, x: Id): Sib[] {
  const ps = src.parents(x);
  const out: Sib[] = [];
  for (let slot = 0; slot < 2; slot++) {
    const p = ps[slot];
    if (p === null) continue;
    const other = ps[1 - slot];
    for (const u of src.unions(p)) {
      // With one parent unknown nothing says the other partner is a different person: do not claim "half".
      const full = other === null || u.partner === other;
      for (const k of u.children) {
        if (ps[0] === k || ps[1] === k) continue;
        const s = out.find((o) => o.key === k);
        if (s) {
          s.hang[slot] = true;
          s.half = s.half && !full;
        } else {
          const hang: [boolean, boolean] = [false, false];
          hang[slot] = true;
          out.push({ key: k, hang, half: !full });
        }
      }
    }
  }
  if (!out.some((s) => s.key === x)) out.push({ key: x, hang: [true, true], half: false });
  const b = (k: Id) => src.birth?.(k) ?? Infinity;
  return out.sort((a, c) => (b(a.key) === b(c.key) ? 0 : b(a.key) < b(c.key) ? -1 : 1));
}

export function layoutRelatives(root: Id, generationsIn: number, aunts: boolean, src: RustLayoutSource): RelLayout {
  const generations = Math.max(1, generationsIn);
  const nodes: N[] = [];
  const rels: Rel[] = [];
  const visited = new Map<Id, number>();
  const units: Unit[] = [];
  const pending: { at: number | null; children: Id[] }[][] = [];
  const blocks: Block[] = [];
  let truncated = false;

  const addNode = (key: Id, row: number, rel: Rel): number => {
    const id = nodes.length;
    const n = newNode(id, key, src, row);
    const first = visited.get(key);
    if (first !== undefined) n.dupOf = first;
    else visited.set(key, id);
    nodes.push(n);
    rels.push(rel);
    return id;
  };

  const makeUnit = (key: Id, row: number, rel: Rel, partnerOf: PartnerOf | null, attach: number | null): number | null => {
    if (nodes.length >= MAX_NODES) {
      truncated = true;
      return null;
    }
    const person = addNode(key, row, rel);
    let partners = 0;
    const pend: { at: number | null; children: Id[] }[] = [];
    if (nodes[person].dupOf === null) {
      for (const u of src.unions(key)) {
        let at: number | null = null;
        if (u.partner !== null && partnerOf !== null) {
          if (nodes.length >= MAX_NODES) truncated = true;
          else {
            at = addNode(u.partner, row, { t: 'partner', of: partnerOf });
            partners++;
          }
        }
        pend.push({ at, children: u.children });
      }
    }
    pending.push(pend);
    units.push({ person, partners, row, cousin: rel.t === 'cousin', attach, children: [], hang: [true, true], sw: 0, ucx: 0, left: 0 });
    return units.length - 1;
  };

  const expand = (rootUnit: number, tree: Tree): void => {
    const limit = limitOf(tree);
    const queue: [number, number][] = [[rootUnit, 0]];
    for (let qi = 0; qi < queue.length; qi++) {
      const [u, depth] = queue[qi];
      const unions = pending[u];
      pending[u] = [];
      const person = units[u].person;
      if (unions.every((x) => x.children.length === 0)) continue;
      if (depth >= limit) {
        nodes[person].moreDown = true;
        continue;
      }
      const row = units[u].row + 1;
      const step = depth + 1;
      const rel: Rel = tree.t === 'focus' ? { t: 'descendant', n: step } : tree.t === 'aunt' ? { t: 'cousin' } : { t: 'nibling' };
      const partnerOf: PartnerOf | null = tree.t === 'focus' ? step : null;
      const kidNodes: number[] = [];
      outer: for (const { at, children } of unions) {
        for (const key of children) {
          const cu = makeUnit(key, row, rel, partnerOf, at);
          if (cu === null) {
            nodes[person].moreDown = true;
            break outer;
          }
          units[u].children.push(cu);
          kidNodes.push(units[cu].person);
          queue.push([cu, depth + 1]);
        }
      }
      nodes[person].nav.child = kidNodes[0] ?? null;
      kidNodes.forEach((k) => (nodes[k].nav.parent = person));
    }
  };

  const makeBlock = (key: Id, row: number, side: number | null): number | null => {
    if (nodes.length >= MAX_NODES) {
      truncated = true;
      return null;
    }
    const dup = visited.has(key);
    const focus = row === 0;
    const withSibs = !dup && (focus || (row === -1 && aunts));
    const sibs: Sib[] = withSibs ? siblingsOf(src, key) : [{ key, hang: [true, true], half: false }];
    // The parent stands next to the other parent: last on the left side, first on the right.
    const at = sibs.findIndex((s) => s.key === key);
    if (at >= 0 && side !== null) sibs.splice(side === 1 ? 0 : sibs.length, 0, ...sibs.splice(at, 1));
    const anchor = focus
      ? makeUnit(key, row, { t: 'focus' }, 'focus', null)
      : makeUnit(key, row, { t: 'ancestor', n: -row }, null, null);
    if (anchor === null) return null;
    if (focus) expand(anchor, { t: 'focus', limit: generations });
    const group: number[] = [];
    for (const s of sibs) {
      let u: number | null = anchor;
      if (s.key !== key) {
        u = makeUnit(s.key, row, focus ? { t: 'sibling', half: s.half } : { t: 'aunt', half: s.half }, focus ? 'sibling' : 'aunt', null);
        if (u === null) continue;
        expand(u, focus ? { t: 'niece' } : { t: 'aunt' });
      }
      units[u].hang = s.hang;
      group.push(u);
    }
    blocks.push({ row, anchor, group, slots: [null, null], ups: [], upOff: [], w: 0, acx: 0, goff: 0, left: 0 });
    return blocks.length - 1;
  };

  makeBlock(root, 0, null);
  const top = -generations;
  for (let bi = 0; bi < blocks.length; bi++) {
    const row = blocks[bi].row;
    const xnode = units[blocks[bi].anchor].person;
    if (nodes[xnode].dupOf !== null) continue;
    const ps = src.parents(nodes[xnode].person!.id);
    if (ps[0] === null && ps[1] === null) continue;
    if (row <= top) {
      nodes[xnode].moreUp = true;
      continue;
    }
    for (let slot = 0; slot < 2; slot++) {
      const pk = ps[slot];
      if (pk === null) continue;
      const nb = makeBlock(pk, row - 1, slot);
      if (nb === null) nodes[xnode].moreUp = true;
      else {
        blocks[bi].slots[slot] = nb;
        blocks[bi].ups.push(nb);
      }
    }
  }

  // Widths, bottom-up: children and ancestor blocks are always created after their owners.
  const gap = M.gapX;
  const uw = (u: Unit) => M.nodeW * (1 + u.partners) + M.coupleGap * u.partners;
  for (let i = units.length - 1; i >= 0; i--) {
    const u = units[i];
    const w = uw(u);
    const ch = u.children;
    if (ch.length === 0) {
      u.sw = w;
      u.ucx = w / 2;
      continue;
    }
    const cw = ch.reduce((s, c) => s + units[c].sw, 0) + gap * (ch.length - 1);
    const sw = Math.max(w, cw);
    const start = (sw - cw) / 2;
    const first = ch[0];
    const last = ch[ch.length - 1];
    const lastOff = start + cw - units[last].sw;
    const mid = (start + units[first].ucx + lastOff + units[last].ucx) / 2;
    u.sw = sw;
    u.ucx = Math.min(sw - w / 2, Math.max(w / 2, mid));
  }
  for (let i = blocks.length - 1; i >= 0; i--) {
    const b = blocks[i];
    const upW = b.ups.reduce((s, u) => s + blocks[u].w, 0) + gap * Math.max(0, b.ups.length - 1);
    const gw = b.group.reduce((s, u) => s + units[u].sw, 0) + gap * Math.max(0, b.group.length - 1);
    const w = Math.max(upW, gw);
    let x = (w - upW) / 2;
    const offs: number[] = [];
    const centres: number[] = [];
    for (const u of b.ups) {
      offs.push(x);
      centres.push(x + blocks[u].acx);
      x += blocks[u].w + gap;
    }
    const target = centres.length ? centres.reduce((s, c) => s + c, 0) / centres.length : w / 2;
    const goff = Math.min(Math.max(w - gw, 0), Math.max(0, target - gw / 2));
    let gx = goff;
    let acx = 0;
    for (const u of b.group) {
      if (u === b.anchor) acx = gx + units[u].ucx;
      gx += units[u].sw + gap;
    }
    b.w = w;
    b.upOff = offs;
    b.goff = goff;
    b.acx = acx;
  }

  // Absolute positions, top-down.
  blocks[0].left = 0;
  for (const b of blocks) {
    b.ups.forEach((u, k) => (blocks[u].left = b.left + b.upOff[k]));
    let gx = b.left + b.goff;
    for (const u of b.group) {
      units[u].left = gx;
      gx += units[u].sw + gap;
    }
  }
  const stepY = M.nodeH + M.gapY * 4;
  const stepP = M.nodeW + M.coupleGap;
  const hasCousins = units.some((u) => u.cousin);
  for (const u of units) {
    const w = uw(u);
    const x0 = u.left + u.ucx - w / 2;
    for (let k = 0; k <= u.partners; k++) {
      const n = nodes[u.person + k];
      n.x = x0 + k * stepP;
      // Cousins get a row of their own between the parents' row and the centred person's row.
      n.y = u.cousin ? 0 : hasCousins && u.row >= 0 ? (u.row + 1) * stepY : u.row * stepY;
    }
    const ch = u.children;
    if (ch.length) {
      const cw = ch.reduce((s, c) => s + units[c].sw, 0) + gap * (ch.length - 1);
      let cx = u.left + (u.sw - cw) / 2;
      for (const c of ch) {
        units[c].left = cx;
        cx += units[c].sw + gap;
      }
    }
  }
  const minX = Math.min(...nodes.map((n) => n.x));
  const minY = Math.min(...nodes.map((n) => n.y));
  for (const n of nodes) {
    n.x -= minX;
    n.y -= minY;
  }

  const edges: ChartEdge[] = [];
  for (const u of units) {
    for (let k = 1; k <= u.partners; k++) {
      const to = u.person + k;
      const a = nodes[to - 1];
      const b = nodes[to];
      edges.push({ from: to - 1, to, kind: 'partner', pts: [a.x + a.w, a.y + a.h / 2, b.x, b.y + b.h / 2] });
    }
    for (const c of u.children) {
      const cu = units[c];
      let sx: number;
      let sy: number;
      if (cu.attach !== null) {
        const a = nodes[cu.attach - 1];
        const b = nodes[cu.attach];
        sx = (a.x + a.w + b.x) / 2;
        sy = b.y + b.h / 2;
      } else {
        const a = nodes[u.person];
        sx = a.x + a.w / 2;
        sy = a.y + a.h;
      }
      const t = nodes[cu.person];
      const mid = nodes[u.person].y + nodes[u.person].h + M.gapY * 2;
      edges.push({ from: u.person, to: cu.person, kind: 'parent', pts: [sx, sy, sx, mid, t.x + t.w / 2, mid, t.x + t.w / 2, t.y] });
    }
  }
  for (const blk of blocks) {
    for (const r of blk.group) {
      const t = nodes[units[r].person];
      const hung = [0, 1].filter((s) => units[r].hang[s] && blk.slots[s] !== null);
      let k = 2;
      if (hung.length === 1 && hung[0] === 0 && blk.slots[1] !== null) k = 1.2;
      else if (hung.length === 1 && hung[0] === 1 && blk.slots[0] !== null) k = 2.8;
      for (const s of hung) {
        const pn = units[blocks[blk.slots[s]!].anchor].person;
        const a = nodes[pn];
        const mid = a.y + a.h + M.gapY * k;
        const ax = a.x + a.w / 2;
        const tx = t.x + t.w / 2;
        edges.push({ from: pn, to: units[r].person, kind: 'parent', pts: [ax, a.y + a.h, ax, mid, tx, mid, tx, t.y] });
      }
    }
  }
  nodes.forEach((n, v) => {
    if (n.dupOf !== null) edges.push(dupEdge(nodes, v, n.dupOf));
  });

  // Keyboard links: up to a parent, down to a child, left and right along the row.
  for (const blk of blocks) {
    const firstUp = (hang: [boolean, boolean]): number | null => {
      for (let s = 0; s < 2; s++) if (hang[s] && blk.slots[s] !== null) return units[blocks[blk.slots[s]!].anchor].person;
      return null;
    };
    for (const r of blk.group) nodes[units[r].person].nav.parent = firstUp(units[r].hang);
    const anchor = units[blk.anchor].person;
    for (const ub of blk.ups) {
      const up = units[blocks[ub].anchor].person;
      if (nodes[up].nav.child === null) nodes[up].nav.child = anchor;
    }
  }
  for (const u of units) {
    for (let k = 1; k <= u.partners; k++) {
      nodes[u.person + k].nav.parent = nodes[u.person].nav.parent;
      nodes[u.person + k].nav.child = nodes[u.person].nav.child;
    }
  }
  for (const n of nodes) {
    n.nav.prev = null;
    n.nav.next = null;
  }
  const order = nodes.map((_, i) => i).sort((a, b) => nodes[a].y - nodes[b].y || nodes[a].x - nodes[b].x || a - b);
  for (let i = 0; i + 1 < order.length; i++) {
    const a = nodes[order[i]];
    const b = nodes[order[i + 1]];
    if (a.y === b.y) {
      a.nav.next = b.id;
      b.nav.prev = a.id;
    }
  }

  for (const n of nodes) {
    const rel = rels[n.id];
    n.relation = relationLabel(rel, n.person!.sex);
    const role = n.dupOf !== null ? `${n.relation}, appears earlier` : n.relation;
    n.label = `${n.person!.name}${n.person!.lifespan ? `, ${n.person!.lifespan}` : ''}, ${role}`;
  }
  return {
    nodes: nodes.map(strip),
    edges,
    bounds: bounds(nodes),
    truncated,
    kids: nodes.map(() => []),
    labels: nodes.map((n) => n.label),
    rels,
  };
}

// ---------------------------------------------------------------- outline

const GROUPS = ['Parents', 'Siblings', 'Partners', 'Children', 'Aunts and uncles'] as const;

function groupOf(l: RelLayout, id: number, g: number): number[] {
  const into = (key: 'from' | 'to') =>
    l.edges.filter((e) => e.kind === 'parent' && e[key === 'from' ? 'to' : 'from'] === id).map((e) => e[key]);
  const onRow = (pred: (r: Rel) => boolean): number[] =>
    id !== 0
      ? []
      : l.nodes
          .filter((n) => pred(l.rels[n.id]) && n.dupOf === null)
          .sort((a, b) => a.x - b.x)
          .map((n) => n.id);
  switch (g) {
    case 0:
      return into('from');
    case 1:
      return onRow((r) => r.t === 'sibling');
    case 2: {
      const out: number[] = [];
      for (const e of l.edges) if (e.kind === 'partner' && (e.from === id || out.includes(e.from))) out.push(e.to);
      return out;
    }
    case 3:
      return into('to');
    default:
      return onRow((r) => r.t === 'aunt');
  }
}

const cap = (s: string) => (s ? s.charAt(0).toUpperCase() + s.slice(1) : s);

/** Where each node is first met reading the outline top to bottom: `owner:group`. Later sightings are references. */
function firstPlaces(l: RelLayout): Map<number, string> {
  const seen = new Map<number, string>([[0, 'root']]);
  const walk = (id: number): void => {
    // Claim everyone listed under this person first, then go into the newly claimed people.
    const fresh: number[] = [];
    GROUPS.forEach((_, g) => {
      for (const m of groupOf(l, id, g)) {
        if (seen.has(m)) continue;
        seen.set(m, `${id}:${g}`);
        fresh.push(m);
      }
    });
    fresh.forEach(walk);
  };
  walk(0);
  return seen;
}

export function relativesOutline(l: RelLayout, parent: number | null, limit: number): OutlineItem[] {
  const places = firstPlaces(l);
  const item = (id: number, place: string): OutlineItem => {
    const n = l.nodes[id];
    const first = places.get(id) === place;
    const label = cap(n.relation ?? '') + (n.dupOf !== null ? ' (appears earlier)' : '') + (first ? '' : ' (also shown above)');
    // Only a person with someone not listed elsewhere can be opened.
    const hasChildren = first && GROUPS.some((_, g) => groupOf(l, id, g).some((m) => places.get(m) === `${id}:${g}`));
    const out: OutlineItem = { nodeId: id, person: n.person, label, hasChildren };
    if (id !== 0 && n.relation) out.relation = n.relation;
    return out;
  };
  if (parent === null) return [item(0, 'root')];
  if (parent < GROUP_BASE) {
    return GROUPS.map((title, g) => ({ g, title }))
      .filter(({ g }) => groupOf(l, parent, g).length > 0)
      .map(({ g, title }) => ({ nodeId: GROUP_BASE + parent * 8 + g, person: null, label: title, hasChildren: true }));
  }
  const rest = parent - GROUP_BASE;
  const owner = Math.floor(rest / 8);
  const g = rest % 8;
  return groupOf(l, owner, g)
    .slice(0, limit)
    .map((m) => item(m, `${owner}:${g}`));
}
