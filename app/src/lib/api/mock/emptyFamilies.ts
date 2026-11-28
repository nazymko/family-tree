import type { EmptyFamilyDto, EmptyFamilyKind, Id, PersonRef } from '../types';
import type { Db, FamilyRec, PersonRec } from './model';
import { MockApiError } from './model';

/** The kind of a family that is a candidate for clean-up, or null for a real family. */
export function kindOf(f: FamilyRec): EmptyFamilyKind | null {
  const partners = (f.partner1 ? 1 : 0) + (f.partner2 ? 1 : 0);
  if (partners === 0) return f.children.length > 0 ? 'parentless' : 'empty';
  if (partners === 1 && f.children.length === 0) return 'singlePartner';
  return null;
}

const members = (f: FamilyRec): Id[] => [f.partner1, f.partner2, ...f.children.map((c) => c.person)].filter((x): x is Id => !!x);

/** How many other families the child is a child in that have at least one partner. */
function otherParentFamilies(db: Db, family: Id, child: Id): number {
  let n = 0;
  for (const f of db.families.values()) {
    if (f.id !== family && (f.partner1 || f.partner2) && f.children.some((c) => c.person === child)) n++;
  }
  return n;
}

function blockers(db: Db, f: FamilyRec, name: (id: Id) => string): { code: string; name: string | null; text: string }[] {
  const out: { code: string; name: string | null; text: string }[] = [];
  const partners = (f.partner1 ? 1 : 0) + (f.partner2 ? 1 : 0);
  if (partners === 2) out.push({ code: 'two_partners', name: null, text: 'The family has two partners: it is a real couple, not an empty family.' });
  if (partners === 1 && f.children.length > 0) out.push({ code: 'parent_with_children', name: null, text: 'The family has one partner and children: it is a real single-parent family.' });
  if (f.events.length > 0) out.push({ code: 'has_events', name: null, text: 'The family has events (a marriage or the like). Move them to a person first, or leave the family.' });
  if (partners === 0) {
    for (const c of f.children) {
      if (otherParentFamilies(db, f.id, c.person) === 0) {
        out.push({ code: 'child_would_be_orphaned', name: name(c.person), text: `${name(c.person)} would be left with no parent family: removing the link would cut them off from the family tree.` });
      }
    }
  }
  const holds = f.notes.length + f.citations.length + (f.media?.length ?? 0) > 0;
  if (holds && members(f).length === 0) out.push({ code: 'needs_notes_to', name: null, text: 'The family holds notes, citations or pictures and has no partner or child to move them to.' });
  return out;
}

export function listEmptyFamilies(db: Db, ref: (p: PersonRec) => PersonRef): EmptyFamilyDto[] {
  const rank: Record<EmptyFamilyKind, number> = { parentless: 0, singlePartner: 1, empty: 2 };
  const nameOf = (id: Id) => {
    const p = db.persons.get(id);
    return p ? ref(p).name : id;
  };
  const rows: EmptyFamilyDto[] = [];
  for (const f of db.families.values()) {
    const kind = kindOf(f);
    if (!kind) continue;
    const why = blockers(db, f, nameOf);
    const holds = f.notes.length + f.citations.length + (f.media?.length ?? 0) > 0;
    rows.push({
      id: f.id,
      kind,
      partners: [f.partner1, f.partner2].flatMap((id) => {
        const p = id ? db.persons.get(id) : undefined;
        return p ? [ref(p)] : [];
      }),
      children: f.children.flatMap((c) => {
        const p = db.persons.get(c.person);
        if (!p) return [];
        const n = otherParentFamilies(db, f.id, c.person);
        return [{ person: ref(p), otherParentFamilies: n, hasOtherFamily: n > 0 }];
      }),
      notes: f.notes.length,
      citations: f.citations.length,
      media: f.media?.length ?? 0,
      events: f.events.map((e) => ({ id: e.id, kind: e.kind, date: e.date?.display ?? null })),
      origin: null,
      removable: why.length === 0,
      blockers: why.map((w) => w.text),
      blockerCodes: why.map((w) => ({ code: w.code, name: w.name })),
      needsNotesTo: holds,
      suggestedNotesTo: members(f)[0] ?? null,
    });
  }
  return rows.sort((a, b) => rank[a.kind] - rank[b.kind] || a.id.localeCompare(b.id));
}

/** Removes the family from `db` (a copy the caller can throw away), moving what it holds to `notesTo`. Returns the touched ids. */
export function removeEmptyFamily(db: Db, id: Id, notesTo: Id | null, name: (id: Id) => string): Id[] {
  const f = db.families.get(id);
  if (!f) throw new MockApiError('not_found', `No family with id "${id}" in this project.`);
  if (!kindOf(f)) throw new MockApiError('invalid', 'This is a real family, not an empty one.');
  const why = blockers(db, f, name);
  if (why.length > 0) throw new MockApiError('invalid', why.map((w) => w.text).join(' '));
  const holds = f.notes.length + f.citations.length + (f.media?.length ?? 0) > 0;
  const touched: Id[] = [id, ...f.children.map((c) => c.person)];
  if (notesTo && !members(f).includes(notesTo)) throw new MockApiError('invalid', 'notes_to must be a partner or a child of the family being removed.');
  if (holds && !notesTo) throw new MockApiError('invalid', 'The family holds notes, citations or pictures. Give a partner or a child who takes them over.');
  if (holds && notesTo) {
    const p = db.persons.get(notesTo);
    if (p) {
      p.notes.push(...f.notes);
      p.citations.push(...f.citations);
      p.media.push(...(f.media ?? []));
      touched.push(notesTo);
    }
  }
  db.families.delete(id);
  return touched;
}
