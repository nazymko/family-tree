// What a row of the events list names first, by kind: the principal people, not everyone in the record.
import type { PlaceEventDto, PlaceEventPersonDto } from '../api/types';
import { plural, t } from '../i18n';

/** A piece of a row's summary: a person (a link when a relative) or plain text between people. */
export type Seg = { person: PlaceEventPersonDto } | { text: string };

type Role = 'child' | 'father' | 'mother' | 'groom' | 'bride' | 'deceased' | 'head' | 'member' | 'other';

// Roles come from the index files (English or Ukrainian) or from the tree's own participant roles.
const ROLES: [Role, RegExp][] = [
  ['bride', /^(bride|наречена|молода)/],
  ['groom', /^(groom|bridegroom|наречений|молодий)/],
  ['child', /^(child|baby|newborn|infant|дитина|немовля|новонароджен|народжен)/],
  ['father', /^(father|батько|отець)/],
  ['mother', /^(mother|мати|матір)/],
  ['deceased', /^(deceased|decedent|died|dead|померл|покійн|небіжч|principal)/],
  ['head', /^(head|householder|голова|господар|глава)/],
  ['member', /^(member|wife|husband|son|daughter|дружина|чоловік|син|донька|член)/],
];

export function roleOf(role: string): Role {
  const r = role.trim().toLowerCase();
  return ROLES.find(([, re]) => re.test(r))?.[0] ?? 'other';
}

const named = (p: PlaceEventPersonDto) => !!(p.name || p.asWritten);

/** The first named person whose role is `want` (each person is used once). */
function pick(people: PlaceEventPersonDto[], used: Set<PlaceEventPersonDto>, want: Role): PlaceEventPersonDto | undefined {
  const p = people.find((x) => !used.has(x) && named(x) && roleOf(x.role) === want);
  if (p) used.add(p);
  return p;
}

export function summarize(e: PlaceEventDto): Seg[] {
  const people = e.people.filter(named);
  const used = new Set<PlaceEventPersonDto>();
  const out: Seg[] = [];
  const add = (...s: Seg[]) => out.push(...s);
  const join = (a: PlaceEventPersonDto | undefined, sep: string, b: PlaceEventPersonDto | undefined) => {
    if (a) add({ person: a });
    if (a && b) add({ text: sep });
    if (b) add({ person: b });
  };
  switch (e.kind) {
    case 'birth': {
      const child = pick(people, used, 'child');
      const father = pick(people, used, 'father');
      const mother = pick(people, used, 'mother');
      if (child) add({ person: child });
      if (child && (father || mother)) add({ text: ' · ' });
      join(father, ' & ', mother);
      break;
    }
    case 'marriage': {
      join(pick(people, used, 'groom'), ' × ', pick(people, used, 'bride'));
      break;
    }
    case 'death': {
      const d = pick(people, used, 'deceased');
      if (d) {
        add({ person: d });
        if (d.age) add({ text: `, ${t('places.age', { age: d.age })}` });
      }
      break;
    }
    case 'confession':
    case 'revision':
    case 'census': {
      const head = pick(people, used, 'head');
      if (head) {
        add({ person: head });
        const more = people.length - 1;
        if (more > 0) add({ text: ` ${plural('places.members', more)}` });
      }
      break;
    }
    default:
      break;
  }
  if (out.length > 0) return out;
  // No recognised roles: the first person (or the first two for a marriage-like pair), then how many more.
  const first = e.kind === 'marriage' ? people.slice(0, 2) : people.slice(0, 1);
  first.forEach((p, i) => add(...(i > 0 ? [{ text: ' × ' } as Seg] : []), { person: p }));
  if (people.length > first.length) add({ text: ` +${people.length - first.length}` });
  return out;
}
