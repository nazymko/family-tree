import type { ChartEdge, ChartNode, Id, PersonRef, WholeBlockDto } from '../types';

// A small stand-in for crates/gt-layout/src/whole.rs so the mock serves a whole-tree chart with the same
// shape: generations as rows, partners side by side, one connector per child, components on shelves (largest
// first) under a heading, and people with no family link in a last block. It is simpler than the Rust one:
// rows are ordered by a depth-first walk and parents are centred over their children with a plain push, no
// least-squares solve and no bar lanes.

const NODE_W = 176;
const NODE_H = 56;
const GAP_X = 40;
const COUPLE_GAP = 8;
const ROW_GAP = 48;
const HEADING_H = 28;
const HEADING_GAP = 12;
const BLOCK_GAP = 96;
const MIN_BLOCK_W = 240;

export interface WholeSource {
  persons: { id: Id; surname: string; birth: number | null }[];
  families: { partners: (Id | null)[]; children: Id[] }[];
  ref(id: Id): PersonRef;
}

export interface WholeResult {
  nodes: ChartNode[];
  edges: ChartEdge[];
  bounds: { w: number; h: number };
  blocks: WholeBlockDto[];
  parents: number[][];
  children: number[][];
  nodeOf: Map<Id, number>;
}

interface Fam {
  partners: number[];
  kids: number[];
}

interface Placed {
  person: number;
  x: number;
  y: number;
  row: number;
}

interface CompOut {
  people: Placed[];
  edges: { from: number; to: number; kind: 'parent' | 'partner'; pts: number[] }[];
  w: number;
  h: number;
}

function layoutComponent(members: number[], fams: Fam[], partnerIn: number[][], childIn: number[][], around?: number): CompOut {
  const level = new Map<number, number>();
  const famLevel = new Map<number, number>();
  const queue: [boolean, number][] = [[false, members[0]]];
  level.set(members[0], 0);
  for (let head = 0; head < queue.length; head++) {
    const [isFam, id] = queue[head];
    if (isFam) {
      const lv = famLevel.get(id)!;
      for (const p of fams[id].partners)
        if (!level.has(p)) {
          level.set(p, lv);
          queue.push([false, p]);
        }
      for (const c of fams[id].kids)
        if (!level.has(c)) {
          level.set(c, lv + 1);
          queue.push([false, c]);
        }
    } else {
      const g = level.get(id)!;
      for (const f of partnerIn[id])
        if (!famLevel.has(f)) {
          famLevel.set(f, g);
          queue.push([true, f]);
        }
      for (const f of childIn[id])
        if (!famLevel.has(f)) {
          famLevel.set(f, g - 1);
          queue.push([true, f]);
        }
    }
  }
  const low = Math.min(...members.map((p) => level.get(p)!));
  const row = new Map(members.map((p) => [p, level.get(p)! - low]));
  const rows = Math.max(...row.values()) + 1;

  // Units: same-row partners side by side.
  const unitOf = new Map<number, number>();
  const units: { members: number[]; row: number; x: number; w: number }[] = [];
  for (const s of members) {
    if (unitOf.has(s)) continue;
    const order: number[] = [];
    const stack = [s];
    while (stack.length) {
      const p = stack.pop()!;
      if (unitOf.has(p)) continue;
      unitOf.set(p, units.length);
      order.push(p);
      for (const f of partnerIn[p])
        for (const q of fams[f].partners) if (q !== p && row.get(q) === row.get(p) && !unitOf.has(q)) stack.push(q);
    }
    units.push({ members: order, row: row.get(s)!, x: 0, w: order.length * NODE_W + (order.length - 1) * COUPLE_GAP });
  }
  const kidsOf = units.map((u) => {
    const out: number[] = [];
    for (const p of u.members)
      for (const f of partnerIn[p])
        for (const c of fams[f].kids) {
          const cu = unitOf.get(c)!;
          if (childIn[c][0] === f && units[cu] !== u && !out.includes(cu)) out.push(cu);
        }
    return out;
  });
  const parentsOf = units.map((u) => {
    const out: number[] = [];
    for (const p of u.members) {
      const f = childIn[p][0];
      if (f === undefined) continue;
      for (const q of fams[f].partners) {
        const pu = unitOf.get(q)!;
        if (units[pu] !== u && !out.includes(pu)) out.push(pu);
      }
    }
    return out;
  });
  const disc = new Array<number>(units.length).fill(-1);
  let counter = 0;
  for (let s = 0; s < units.length; s++) {
    const stack = [s];
    while (stack.length) {
      const u = stack.pop()!;
      if (disc[u] >= 0) continue;
      disc[u] = counter++;
      stack.push(...[...parentsOf[u], ...kidsOf[u]].reverse());
    }
  }
  const byRow: number[][] = Array.from({ length: rows }, () => []);
  units.forEach((u, i) => byRow[u.row].push(i));
  for (const r of byRow) r.sort((a, b) => disc[a] - disc[b]);
  if (around !== undefined && unitOf.has(around)) {
    // Arranged around one person: in every row the units closest to them (by parents / children links) go in the
    // middle and the farther ones alternate outwards, so close relatives end up near each other.
    const dist = new Array<number>(units.length).fill(Infinity);
    const start = unitOf.get(around)!;
    dist[start] = 0;
    const queue = [start];
    for (let head = 0; head < queue.length; head++) {
      const u = queue[head];
      for (const v of [...parentsOf[u], ...kidsOf[u]])
        if (dist[v] === Infinity) {
          dist[v] = dist[u] + 1;
          queue.push(v);
        }
    }
    for (const r of byRow) {
      const sorted = [...r].sort((a, b) => dist[a] - dist[b] || disc[a] - disc[b]);
      const left: number[] = [];
      const right: number[] = [];
      sorted.forEach((u, i) => (i === 0 ? right.push(u) : i % 2 === 1 ? left.push(u) : right.push(u)));
      r.splice(0, r.length, ...left.reverse(), ...right);
    }
  }
  const pack = (r: number[], wanted: (u: number) => number | null) => {
    let right = -Infinity;
    for (const u of r) {
      const want = wanted(u);
      const x = Math.max(want ?? units[u].x, right + GAP_X);
      units[u].x = x;
      right = x + units[u].w;
    }
  };
  for (const r of byRow) pack(r, () => null);
  const centre = (u: number) => units[u].x + units[u].w / 2;
  const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;
  for (let pass = 0; pass < 3; pass++) {
    for (let r = 1; r < rows; r++) pack(byRow[r], (u) => (parentsOf[u].length ? mean(parentsOf[u].map(centre)) - units[u].w / 2 : null));
    for (let r = rows - 2; r >= 0; r--) pack(byRow[r], (u) => (kidsOf[u].length ? mean(kidsOf[u].map(centre)) - units[u].w / 2 : null));
  }
  const minX = Math.min(...units.map((u) => u.x));
  const px = new Map<number, number>();
  const pos = new Map<number, number>();
  for (const u of units)
    u.members.forEach((p, k) => {
      px.set(p, u.x - minX + k * (NODE_W + COUPLE_GAP));
      pos.set(p, k);
    });
  const rowY = (r: number) => r * (NODE_H + ROW_GAP);
  const out: CompOut = { people: [], edges: [], w: 0, h: 0 };
  for (const p of members) {
    const r = row.get(p)!;
    out.people.push({ person: p, x: px.get(p)!, y: rowY(r), row: r });
    out.w = Math.max(out.w, px.get(p)! + NODE_W);
    out.h = Math.max(out.h, rowY(r) + NODE_H);
  }
  const cx = (p: number) => px.get(p)! + NODE_W / 2;
  const seen = new Set<string>();
  for (const f of fams) {
    if (f.partners.length < 2 || !members.includes(f.partners[0])) continue;
    const [p, q] = f.partners;
    const key = `${Math.min(p, q)}:${Math.max(p, q)}`;
    if (seen.has(key)) continue;
    seen.add(key);
    const [a, b] = px.get(p)! <= px.get(q)! ? [p, q] : [q, p];
    const adjacent = unitOf.get(a) === unitOf.get(b) && Math.abs(pos.get(a)! - pos.get(b)!) === 1 && row.get(a) === row.get(b);
    const ya = rowY(row.get(a)!);
    const pts = adjacent
      ? [px.get(a)! + NODE_W, ya + NODE_H / 2, px.get(b)!, ya + NODE_H / 2]
      : [cx(a), ya + NODE_H, cx(a), ya + NODE_H + 8, cx(b), ya + NODE_H + 8, cx(b), rowY(row.get(b)!) + NODE_H];
    out.edges.push({ from: a, to: b, kind: 'partner', pts });
  }
  for (const f of fams) {
    if (!f.partners.length || !f.kids.length || !members.includes(f.partners[0])) continue;
    const p = f.partners[0];
    const q = f.partners[1];
    const arow = q === undefined ? row.get(p)! : Math.max(row.get(p)!, row.get(q)!);
    const adjacent =
      q !== undefined && unitOf.get(p) === unitOf.get(q) && Math.abs(pos.get(p)! - pos.get(q)!) === 1 && row.get(p) === row.get(q);
    const ax = q === undefined ? cx(p) : adjacent ? (Math.min(px.get(p)!, px.get(q)!) + NODE_W + Math.max(px.get(p)!, px.get(q)!)) / 2 : (cx(p) + cx(q)) / 2;
    const ay = rowY(arow) + (adjacent ? NODE_H / 2 : NODE_H + (q === undefined ? 0 : 8));
    const bus = rowY(arow) + NODE_H + 20;
    for (const k of f.kids) {
      const kr = row.get(k)!;
      const end = kr > arow ? rowY(kr) : rowY(kr) + NODE_H;
      out.edges.push({ from: p, to: k, kind: 'parent', pts: [ax, ay, ax, bus, cx(k), bus, cx(k), end] });
    }
  }
  return out;
}

export function layoutWhole(src: WholeSource, reversed = false, aroundId?: Id): WholeResult {
  const index = new Map(src.persons.map((p, i) => [p.id, i]));
  const n = src.persons.length;
  const fams: Fam[] = [];
  for (const f of src.families) {
    const partners: number[] = [];
    for (const id of f.partners) {
      const i = id === null ? undefined : index.get(id);
      if (i !== undefined && !partners.includes(i)) partners.push(i);
    }
    const kids: number[] = [];
    for (const id of f.children) {
      const i = index.get(id);
      if (i !== undefined && !partners.includes(i) && !kids.includes(i)) kids.push(i);
    }
    kids.sort((a, b) => (src.persons[a].birth ?? Infinity) - (src.persons[b].birth ?? Infinity));
    if (partners.length + kids.length >= 2) fams.push({ partners, kids });
  }
  const partnerIn: number[][] = Array.from({ length: n }, () => []);
  const childIn: number[][] = Array.from({ length: n }, () => []);
  const dsu = Array.from({ length: n }, (_, i) => i);
  const find = (x: number): number => {
    while (dsu[x] !== x) x = dsu[x] = dsu[dsu[x]];
    return x;
  };
  fams.forEach((f, i) => {
    const all = [...f.partners, ...f.kids];
    f.partners.forEach((p) => partnerIn[p].push(i));
    f.kids.forEach((c) => childIn[c].push(i));
    for (const p of all.slice(1)) {
      const a = find(all[0]);
      const b = find(p);
      if (a !== b) dsu[Math.max(a, b)] = Math.min(a, b);
    }
  });
  const comps = new Map<number, number[]>();
  const loose: number[] = [];
  for (let p = 0; p < n; p++) {
    if (!partnerIn[p].length && !childIn[p].length) loose.push(p);
    else {
      const r = find(p);
      if (!comps.has(r)) comps.set(r, []);
      comps.get(r)!.push(p);
    }
  }
  const around = aroundId === undefined ? undefined : index.get(aroundId);
  const placed = [...comps.values()]
    .map((members) => {
      const out = layoutComponent(members, fams, partnerIn, childIn, around);
      const count = new Map<string, number>();
      for (const p of members) {
        const s = src.persons[p].surname;
        if (s) count.set(s, (count.get(s) ?? 0) + 1);
      }
      let surname = '';
      let best = 0;
      for (const [s, c] of count) if (c > best || (c === best && s < surname)) [surname, best] = [s, c];
      return { members, out, surname, w: Math.max(out.w, MIN_BLOCK_W), first: around !== undefined && members.includes(around) };
    })
    .sort((a, b) => Number(b.first) - Number(a.first) || b.members.length - a.members.length);

  const area =
    placed.reduce((s, p) => s + (p.w + BLOCK_GAP) * (p.out.h + HEADING_H + HEADING_GAP + BLOCK_GAP), 0) +
    loose.length * (NODE_W + GAP_X) * (NODE_H + 12);
  const shelfW = Math.max(MIN_BLOCK_W, ...placed.map((p) => p.w), Math.sqrt(area) * 1.6);

  const blocks: WholeBlockDto[] = [];
  const nodes: ChartNode[] = [];
  const nodeOf = new Map<Id, number>();
  const personNode = new Map<number, number>();
  const rawEdges: { from: number; to: number; kind: 'parent' | 'partner'; pts: number[] }[] = [];
  const addNode = (person: number, x: number, y: number, row: number) => {
    const id = nodes.length;
    const pid = src.persons[person].id;
    nodes.push({
      id,
      person: src.ref(pid),
      x,
      y,
      w: NODE_W,
      h: NODE_H,
      gen: row,
      ahnen: null,
      dupOf: null,
      moreUp: false,
      moreDown: false,
      nav: { parent: null, child: null, prev: null, next: null },
      label: '',
    });
    nodeOf.set(pid, id);
    personNode.set(person, id);
  };
  let x = 0;
  let y = 0;
  let shelfH = 0;
  for (const p of placed) {
    const h = p.out.h + HEADING_H + HEADING_GAP;
    if (x > 0 && x + p.w > shelfW) {
      y += shelfH + BLOCK_GAP;
      x = 0;
      shelfH = 0;
    }
    const title = p.surname ? `${p.surname} family (${p.members.length} people)` : `Family (${p.members.length} people)`;
    blocks.push({ title, surname: p.surname, people: p.members.length, unconnected: false, rect: { x, y, w: p.w, h: HEADING_H }, bodyH: p.out.h });
    const oy = y + HEADING_H + HEADING_GAP;
    // Reversed: each family is flipped inside its own block, so its heading stays above its people.
    const flipY = (v: number) => (reversed ? p.out.h - v : v);
    for (const q of [...p.out.people].sort((a, b) => a.y - b.y || a.x - b.x || a.person - b.person)) addNode(q.person, x + q.x, oy + (reversed ? p.out.h - q.y - NODE_H : q.y), q.row);
    for (const e of p.out.edges) rawEdges.push({ ...e, pts: e.pts.map((v, i) => (i % 2 === 0 ? v + x : oy + flipY(v))) });
    x += p.w + BLOCK_GAP;
    shelfH = Math.max(shelfH, h);
  }
  if (loose.length) {
    if (x > 0) y += shelfH + BLOCK_GAP;
    const cols = Math.max(1, Math.min(loose.length, Math.floor((shelfW + GAP_X) / (NODE_W + GAP_X))));
    blocks.push({
      title: `Unconnected people (${loose.length})`,
      surname: '',
      people: loose.length,
      unconnected: true,
      rect: { x: 0, y, w: Math.max(MIN_BLOCK_W, cols * (NODE_W + GAP_X) - GAP_X), h: HEADING_H },
      bodyH: Math.ceil(loose.length / cols) * (NODE_H + 12) - 12,
    });
    const oy = y + HEADING_H + HEADING_GAP;
    loose.forEach((person, i) => addNode(person, (i % cols) * (NODE_W + GAP_X), oy + Math.floor(i / cols) * (NODE_H + 12), 0));
  }

  const edges: ChartEdge[] = rawEdges.map((e) => ({ from: personNode.get(e.from)!, to: personNode.get(e.to)!, kind: e.kind, pts: e.pts }));
  const parents: number[][] = nodes.map(() => []);
  const children: number[][] = nodes.map(() => []);
  for (const f of fams)
    for (const c of f.kids)
      for (const p of f.partners) {
        const [pn, cn] = [personNode.get(p)!, personNode.get(c)!];
        if (!parents[cn].includes(pn)) {
          parents[cn].push(pn);
          children[pn].push(cn);
        }
      }
  for (const [i, f] of fams.entries()) {
    if (!f.partners.length) continue;
    const kids = f.kids.filter((c) => childIn[c][0] === i).map((c) => personNode.get(c)!);
    for (const p of f.partners) {
      const nav = nodes[personNode.get(p)!].nav;
      if (nav.child === null) nav.child = kids[0] ?? null;
    }
    kids.forEach((k, j) => {
      nodes[k].nav = { parent: personNode.get(f.partners[0])!, child: nodes[k].nav.child, prev: kids[j - 1] ?? null, next: kids[j + 1] ?? null };
    });
  }
  const bounds = { w: Math.max(0, ...nodes.map((q) => q.x + q.w), ...blocks.map((b) => b.rect.x + b.rect.w)), h: Math.max(0, ...nodes.map((q) => q.y + q.h)) };
  return { nodes, edges, bounds, blocks, parents, children, nodeOf };
}

/** The node, its ancestors and its descendants, ascending. */
export function lineageOf(w: WholeResult, node: number): number[] {
  const seen = new Set([node]);
  for (const adj of [w.parents, w.children]) {
    const stack = [node];
    while (stack.length) {
      for (const next of adj[stack.pop()!] ?? [])
        if (!seen.has(next)) {
          seen.add(next);
          stack.push(next);
        }
    }
  }
  return [...seen].sort((a, b) => a - b);
}
