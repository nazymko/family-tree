import type { ExportOptions, EventDto, Id, PrivacyPreview } from '../types';
import type { Db, Fixture, PersonRec } from './model';
import { MockApi } from './MockApi';

/*
 * A mock of gt_core::privacy::filter's SUMMARY (crates/gt-core/src/privacy.rs + living.rs), so the export
 * dialog shows realistic numbers on MockApi. The UI never computes privacy; this is test/dev data only.
 *
 * Rules mirrored:
 *  - Living is fail-safe: a person is dead only when something proves it (a non-negated death, burial,
 *    cremation or probate event, a birth/christening/baptism whose latest possible day is more than
 *    max_age years ago, a child born more than max_age - 12 years ago proves the parents dead, a partner
 *    born more than max_age + 70 years ago proves the children dead). Missing or open-ended dates prove
 *    nothing, so an undated person is living. An explicit override wins.
 *  - Mode Exclude/Placeholder hide every living person; IncludeAll hides nobody.
 *    hiddenPersons = |hidden|; placeholderPersons = |hidden| in Placeholder mode, else 0.
 *  - Private records are removed unless includePrivate, whatever the living mode.
 *  - removedRecords excludes persons removed because they are living; it counts private persons, private
 *    events of kept persons, and events of families that have a hidden partner (their dates would leak).
 *  - mentionWarnings: a kept note whose text contains "given surname" of a hidden person (case-insensitive).
 *  - groupMedia: kept media that is also linked from a hidden person.
 * Year-level arithmetic approximates the Rust day-level cut-offs. exportedPersons counts persons written
 * under their own name (placeholders are counted separately).
 */

const REFERENCE_YEAR = 2026;
const DEATHLIKE = new Set(['death', 'burial', 'cremation', 'probate']);
const BIRTHLIKE = new Set(['birth', 'christening', 'baptism']);

const yearOfDay = (day: number) => new Date(day * 86_400_000).getUTCFullYear();

export function livingSet(db: Db, maxAge: number, refYear = REFERENCE_YEAR): Set<Id> {
  const dead = new Set<Id>();
  const birthBound = new Map<Id, number>();
  for (const p of db.persons.values()) {
    for (const e of p.events) {
      if (e.negative) continue;
      if (DEATHLIKE.has(e.kind)) dead.add(p.id);
      if (BIRTHLIKE.has(e.kind) && e.date?.sortMax != null) {
        const y = yearOfDay(e.date.sortMax);
        birthBound.set(p.id, Math.max(birthBound.get(p.id) ?? -Infinity, y));
      }
    }
  }
  const bornBefore = (id: Id, cutYear: number) => (birthBound.get(id) ?? Infinity) < cutYear;
  const proven = new Set<Id>();
  for (const f of db.families.values()) {
    const partners = [f.partner1, f.partner2].filter((x): x is Id => !!x);
    if (f.children.some((c) => bornBefore(c.person, refYear - (maxAge - 12)))) partners.forEach((x) => proven.add(x));
    if (partners.some((x) => bornBefore(x, refYear - (maxAge + 70)))) f.children.forEach((c) => proven.add(c.person));
  }
  const living = new Set<Id>();
  for (const p of db.persons.values()) {
    let isLiving: boolean;
    if (p.livingOverride) isLiving = p.livingOverride === 'living';
    else isLiving = !(dead.has(p.id) || bornBefore(p.id, refYear - maxAge) || proven.has(p.id));
    if (isLiving) living.add(p.id);
  }
  return living;
}

const fullName = (p: PersonRec): string[] =>
  p.names
    .filter((n) => n.given && n.surnames.length)
    .map((n) => `${n.given} ${n.surnames.join(' ')}`.toLowerCase());

const displayName = (p: PersonRec) => p.names.find((n) => n.preferred)?.display ?? p.names[0]?.display ?? 'Unnamed';

export type PrivacyCounts = Pick<
  PrivacyPreview,
  'exportedPersons' | 'placeholderPersons' | 'hiddenPersons' | 'removedRecords' | 'mentionWarnings' | 'groupMedia'
>;

export function computePrivacyCounts(db: Db, opts: ExportOptions, maxAge: number, refYear = REFERENCE_YEAR): PrivacyCounts {
  const living = livingSet(db, maxAge, refYear);
  const hidden = opts.living === 'all' ? new Set<Id>() : living;
  const placeholder = opts.living === 'placeholder';
  const dropPrivate = !opts.includePrivate;
  const persons = [...db.persons.values()];

  let removedRecords = 0;
  const kept: PersonRec[] = [];
  for (const p of persons) {
    const privateDropped = dropPrivate && p.private;
    const livingRemoved = hidden.has(p.id) && !placeholder;
    if (privateDropped && !livingRemoved) removedRecords++;
    if (!privateDropped && !hidden.has(p.id)) kept.push(p);
  }
  const keptEvents = (p: PersonRec): EventDto[] => p.events.filter((e) => !(dropPrivate && e.private));
  for (const p of kept) removedRecords += p.events.length - keptEvents(p).length;
  for (const f of db.families.values()) {
    const hiddenPartner = [f.partner1, f.partner2].some((x) => x && hidden.has(x));
    if (hiddenPartner) removedRecords += f.events.length;
  }

  const hiddenPersons = [...hidden].map((id) => db.persons.get(id)).filter((p): p is PersonRec => !!p);
  const hiddenNames = hiddenPersons.map((p) => ({ p, names: fullName(p) }));
  const mentionWarnings: PrivacyPreview['mentionWarnings'] = [];
  // Every surviving note, as Rust scans the filtered tree: kept persons' notes, kept events' notes, and notes of
  // families (and their events) that have no hidden partner. Rust drops private notes with their records; the mock
  // has no private flag on notes, so a note on a dropped record is simply not reached.
  const notes = kept.flatMap((p) => [...p.notes, ...keptEvents(p).flatMap((e) => e.notes)]);
  for (const f of db.families.values()) {
    if ([f.partner1, f.partner2].some((x) => x && hidden.has(x))) continue;
    notes.push(...f.notes, ...f.events.filter((e) => !(dropPrivate && e.private)).flatMap((e) => e.notes));
  }
  {
    for (const n of notes) {
      const low = n.text.toLowerCase();
      for (const h of hiddenNames) {
        const hit = h.names.map((nm) => low.indexOf(nm)).find((i) => i >= 0);
        if (hit === undefined) continue;
        const from = Math.max(0, hit - 30);
        mentionWarnings.push({
          noteExcerpt: `${from > 0 ? '…' : ''}${n.text.slice(from, hit + 50).trim()}${hit + 50 < n.text.length ? '…' : ''}`,
          hiddenPerson: displayName(h.p),
        });
      }
    }
  }

  const keptMedia = new Map<Id, { title: string; originalName: string | null }>();
  for (const p of kept) {
    for (const l of [...p.media, ...keptEvents(p).flatMap((e) => e.media)]) {
      keptMedia.set(l.media.id, { title: l.media.title ?? l.media.originalName ?? 'Untitled file', originalName: l.media.originalName });
    }
  }
  const groupMedia: PrivacyPreview['groupMedia'] = [];
  const seen = new Set<Id>();
  for (const h of hiddenPersons) {
    for (const l of h.media) {
      const k = keptMedia.get(l.media.id);
      if (k && !seen.has(l.media.id)) {
        seen.add(l.media.id);
        groupMedia.push(k);
      }
    }
  }

  return {
    exportedPersons: kept.length,
    placeholderPersons: placeholder ? hidden.size : 0,
    hiddenPersons: hidden.size,
    removedRecords,
    mentionWarnings,
    groupMedia,
  };
}

export const LOSSLESS_551 =
  "Roles, 'did not happen' records, shared notes and external IDs are written as extension tags; other programs may drop them.";

/** MockApi whose export preview follows the Rust privacy filter's counting rules. */
export class PrivacyMockApi extends MockApi {
  #defaultVersion: '5.5.1' | '7.0';

  constructor(fixture: Fixture, opts: { defaultVersion?: '5.5.1' | '7.0' } = {}) {
    super(fixture);
    this.#defaultVersion = opts.defaultVersion ?? '7.0';
  }

  override async previewExport(opts: ExportOptions): Promise<PrivacyPreview> {
    const base = await super.previewExport(opts); // registers the token exportGedcom checks
    const inner = this as unknown as { db: Db; settings: { livingMaxAge: number } };
    return {
      ...base,
      ...computePrivacyCounts(inner.db, opts, inner.settings.livingMaxAge),
      defaultVersion: this.#defaultVersion,
      losslessNotes: opts.version === '5.5.1' ? [LOSSLESS_551] : [],
    };
  }
}

