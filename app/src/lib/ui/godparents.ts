import type { EventDto, GodparentDto, SexKind } from '../api/types';
import { t } from '../i18n';

/** The kinds of event that have godparents. */
export const takesGodparents = (kind: string): boolean => kind === 'baptism' || kind === 'christening';

/** "godfather", "godmother"; a godparent of no stated role reads by the person's sex when known. */
export function godparentRoleText(role: string, sex?: SexKind | null): string {
  if (role === 'godfather' || (role === 'godparent' && sex === 'male')) return t('godparentRole.godfather');
  if (role === 'godmother' || (role === 'godparent' && sex === 'female')) return t('godparentRole.godmother');
  return t('godparentRole.godparent');
}

/** A written name up to its first comma: "Ivan Koval, a peasant of ..." shows as "Ivan Koval" where room is short. */
export function shortGodparentName(name: string | null | undefined): string {
  const text = (name ?? '').trim();
  const cut = text.indexOf(',');
  return cut > 0 ? text.slice(0, cut).trim() : text;
}

/** One godparent in full, for a tooltip: name, role and note. */
export function godparentFull(g: GodparentDto): string {
  const who = g.person?.name ?? g.name ?? '';
  const role = godparentRoleText(g.role, g.person?.sex);
  return [`${who} (${role})`, g.note?.trim()].filter(Boolean).join(': ');
}

/** The baptism shown on a person's card: a baptism, else a christening. */
export function baptismOf(events: EventDto[] | undefined): EventDto | undefined {
  return events?.find((e) => e.kind === 'baptism') ?? events?.find((e) => e.kind === 'christening');
}
