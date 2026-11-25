import type { Id, MediaDto, MediaLinkDto } from '../api/api';

/**
 * Versions of one page: the same page kept in several files (a new scan and an older small copy). The backend sends the
 * whole group on every member, primary first; a file with no other versions has none.
 */

/** A shown link: one per version group. `linked` names the members this record actually links to. */
export type ShownLink = MediaLinkDto & { linked?: Id[] };

export const pixels = (m: Pick<MediaDto, 'width' | 'height'>): number => (m.width ?? 0) * (m.height ?? 0);

/** Every version of a file, primary first; empty when it has no other versions. */
export const versionsOf = (m: MediaDto): MediaDto[] => (m.versions && m.versions.length > 1 ? m.versions : []);

/** How many files the page is kept in: 1 for a file with no other versions. */
export const versionCount = (m: MediaDto): number => Math.max(1, versionsOf(m).length);

/**
 * One link per version group, in the order the groups first appear. The tile shows the group's primary even when this
 * record links only to another version of it; the link counts as primary when any link of the group is.
 */
export function collapseVersions(links: readonly MediaLinkDto[]): ShownLink[] {
  const out: ShownLink[] = [];
  const at = new Map<Id, number>();
  for (const l of links) {
    const vs = versionsOf(l.media);
    if (vs.length === 0 || l.clip) {
      out.push(l);
      continue;
    }
    const head = vs[0];
    const seen = at.get(head.id);
    if (seen !== undefined) {
      const was = out[seen];
      out[seen] = { ...was, primary: was.primary || l.primary, linked: [...(was.linked ?? []), l.media.id] };
      continue;
    }
    at.set(head.id, out.length);
    out.push({ ...l, media: { ...head, versions: vs }, linked: [l.media.id] });
  }
  return out;
}

/** The file to show first: the most pixels; on a tie, the one named first. */
export function bestPrimary<T extends Pick<MediaDto, 'width' | 'height'>>(files: readonly T[]): T | undefined {
  let best: T | undefined;
  for (const f of files) if (!best || pixels(f) > pixels(best)) best = f;
  return best;
}

/** "4392×3496", or null when the size is not known. */
export const sizeText = (m: Pick<MediaDto, 'width' | 'height'>): string | null => (m.width && m.height ? `${m.width}×${m.height}` : null);

/** The member of a shown link this record links to, preferring `viewed`: what Remove and primary photo act on. */
export function linkedMember(link: ShownLink, viewed: Id): Id {
  const linked = link.linked ?? [link.media.id];
  return linked.includes(viewed) ? viewed : (linked[0] ?? viewed);
}

/** The edit that groups `picked` (and its group) with the group of `current`: the largest file becomes the primary. */
export function groupCommand(current: MediaDto, picked: MediaDto): { type: 'groupMediaVersions'; primary: Id; others: Id[] } | null {
  const files = new Map<Id, MediaDto>();
  for (const m of [current, ...versionsOf(current), picked, ...versionsOf(picked)]) if (!files.has(m.id)) files.set(m.id, m);
  // The primary of the group the page is opened from stays first when it is as large as any.
  const order = [...files.values()];
  const head = versionsOf(current)[0] ?? current;
  const best = bestPrimary([files.get(head.id) ?? head, ...order.filter((m) => m.id !== head.id)]);
  if (!best || files.size < 2) return null;
  return { type: 'groupMediaVersions', primary: best.id, others: order.filter((m) => m.id !== best.id).map((m) => m.id) };
}
