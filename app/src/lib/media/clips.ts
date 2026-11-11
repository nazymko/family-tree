import type { ClipDto, Id, MediaDto, MediaLinkDto, OwnerRef, PersonDetail } from '../api/api';
import { isSound } from './sound';

/**
 * 65000 -> "1:05", 3725000 -> "1:02:05"; with `precise`, tenths of a second when there are any ("1:05.3").
 * The same clock the backend writes in its messages and the agent reads.
 */
export function clockMs(ms: number, precise = false): string {
  const total = Math.max(0, Math.floor(ms));
  const s = Math.floor(total / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  const tenth = Math.floor((total % 1000) / 100);
  const frac = precise && tenth > 0 ? `.${tenth}` : '';
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}${frac}` : `${m}:${ss}${frac}`;
}

/**
 * "1:05", "01:05.25", "1:02:05" or a whole number of seconds ("75") to milliseconds; null when it is none of them.
 * Seconds and minutes after the first part are below 60.
 */
export function parseClock(text: string): number | null {
  const s = text.trim();
  if (/^\d+(\.\d{1,3})?$/.test(s)) return Math.round(Number(s) * 1000);
  const m = /^(?:(\d+):)?(\d{1,2}):(\d{2})(?:\.(\d{1,3}))?$/.exec(s);
  if (!m) return null;
  const [, h, mm, ss, frac] = m;
  if (Number(ss) > 59 || (h !== undefined && Number(mm) > 59)) return null;
  const ms = frac ? Number(frac.padEnd(3, '0')) : 0;
  return ((Number(h ?? 0) * 60 + Number(mm)) * 60 + Number(ss)) * 1000 + ms;
}

/** A clip's link: a stretch of a recording linked to a record. */
export type ClipLink = MediaLinkDto & { clip: ClipDto };

export const isClip = (l: MediaLinkDto): l is ClipLink => !!l.clip;

/** The clips among a record's media links, in the order they are said. */
export function clipsOf(links: readonly MediaLinkDto[] | null | undefined): ClipLink[] {
  return (links ?? []).filter(isClip).sort((a, b) => a.clip.startMs - b.clip.startMs);
}

/** Whole recordings among a record's media links (a voice sample on a note, a recording on its source). */
export function wholeSounds(links: readonly MediaLinkDto[] | null | undefined): MediaLinkDto[] {
  return (links ?? []).filter((l) => !l.clip && isSound(l.media));
}

/** The clip being heard at `ms`, if any (the latest-starting one when clips overlap). */
export function clipAt<T extends { startMs: number; endMs: number }>(clips: readonly T[], ms: number): T | null {
  let hit: T | null = null;
  for (const c of clips) if (c.startMs <= ms && ms < c.endMs && (!hit || c.startMs >= hit.startMs)) hit = c;
  return hit;
}

/** The start of the next clip after `ms`, or of the previous one (a clip just started counts as current, not previous). */
export function stepClip(starts: readonly number[], ms: number, dir: 1 | -1): number | null {
  const sorted = [...starts].sort((a, b) => a - b);
  if (dir === 1) return sorted.find((s) => s > ms + 250) ?? null;
  const before = sorted.filter((s) => s < ms - 1500);
  return before.length ? before[before.length - 1] : null;
}

/** One place in a person's records that a clip is linked to. */
export interface ClipPlace {
  link: ClipLink;
  owner: OwnerRef;
}

/** A recording heard on a person's page: the recording, and the clips of it linked to the person's records. */
export interface PersonRecording {
  media: MediaDto;
  clips: ClipPlace[];
  /** True when the recording is on a source the person's citations name. */
  cited: boolean;
}

/**
 * Every recording a person's page touches: clips on the person, their notes, events and citations (and the events'
 * notes and citations), and recordings on the sources they cite. Grouped by recording, in the order first met.
 */
export function personRecordings(d: PersonDetail): PersonRecording[] {
  const byId = new Map<Id, PersonRecording>();
  const take = (links: readonly MediaLinkDto[] | null | undefined, owner: OwnerRef) => {
    for (const link of clipsOf(links)) {
      const rec = byId.get(link.media.id) ?? { media: link.media, clips: [], cited: false };
      rec.clips.push({ link, owner });
      byId.set(link.media.id, rec);
    }
  };
  const cites = (cs: PersonDetail['citations']) => {
    for (const c of cs) {
      take(c.media, { kind: 'citation', id: c.id });
      for (const l of wholeSounds(c.source?.media)) {
        const rec = byId.get(l.media.id) ?? { media: l.media, clips: [], cited: true };
        rec.cited = true;
        byId.set(l.media.id, rec);
      }
    }
  };
  take(d.media, { kind: 'person', id: d.ref.id });
  for (const n of d.notes) take(n.media, { kind: 'note', id: n.id });
  for (const e of d.events) {
    take(e.media, { kind: 'event', id: e.id });
    for (const n of e.notes) take(n.media, { kind: 'note', id: n.id });
    cites(e.citations);
  }
  cites(d.citations);
  for (const n of d.names) cites(n.citations);
  for (const r of byId.values()) r.clips.sort((a, b) => a.link.clip.startMs - b.link.clip.startMs);
  return [...byId.values()];
}
