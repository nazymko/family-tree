import type { EventDto, PersonRef } from '../api/types';

/**
 * How a couple ended, read from the couple's own events: a divorce (or annulment), a separation, or the death of one
 * of them ("widowed"). The person who left or died and the one who then raised the children alone are participants
 * of that event, so nothing beyond ordinary events is stored and a GEDCOM export keeps it.
 */
export type EndReason = 'divorce' | 'separation' | 'death';

export const END_KIND: Record<EndReason, string> = {
  divorce: 'divorce',
  separation: 'separation',
  death: 'custom:widowed',
};
export const ROLE_LEFT = 'left_family';
export const ROLE_ALONE = 'raised_alone';

export interface CoupleEnd {
  event: EventDto;
  reason: EndReason;
  /** The one who left (or died), when recorded. */
  left: PersonRef | null;
  /** The one who raised the children alone afterwards, when recorded. */
  alone: PersonRef | null;
  /** "1935", or the date as written when it has no year. */
  when: string;
}

function reasonOf(kind: string): EndReason | null {
  if (kind === 'divorce' || kind === 'annulment' || kind === 'divorce_filed') return 'divorce';
  if (kind === 'separation') return 'separation';
  if (kind === END_KIND.death) return 'death';
  return null;
}

/** The couple's end, if one of its events records it (a divorce wins over a filing, the latest record over older ones). */
export function coupleEnd(events: EventDto[] | undefined): CoupleEnd | null {
  let best: CoupleEnd | null = null;
  for (const event of events ?? []) {
    const reason = reasonOf(event.kind);
    if (!reason || event.negative) continue;
    const who = (role: string) => event.participants.find((p) => p.role === role)?.person ?? null;
    const shown = event.date?.display ?? '';
    const when = /\b(\d{4})\b/.exec(shown)?.[1] ?? shown;
    const found: CoupleEnd = {
      event,
      reason,
      left: who(ROLE_LEFT),
      alone: who(ROLE_ALONE),
      when,
    };
    if (!best || (best.event.kind === 'divorce_filed' && event.kind !== 'divorce_filed') || (found.left && !best.left)) best = found;
  }
  return best;
}
