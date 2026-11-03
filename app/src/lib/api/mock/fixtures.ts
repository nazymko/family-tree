import type { CitationDto, Id, MediaDto, PlaceOption, SexKind, SourceDto } from '../types';
import { IdGen, makeEvent, makeName, makeNote, mulberry32 } from './build';
import type { ChildLinkRec, FamilyRec, Fixture, PersonRec } from './model';

const GIVEN_M = ['Johann', 'Friedrich', 'Wilhelm', 'Heinrich', 'Karl', 'August', 'Ludwig', 'Hermann', 'Jürgen', 'Józef', 'François', 'Anders', 'Thomas', 'Georg', 'Otto'];
const GIVEN_F = ['Anna', 'Maria', 'Elisabeth', 'Katharina', 'Margarethe', 'Sophie', 'Charlotte', 'Luise', 'Åsa', 'Zoë', 'Henriette', 'Clara', 'Johanna', 'Emma', 'Frieda'];
const SURNAMES = ['Müller', 'Müller', 'Schmidt', 'Schneider', 'Fischer', 'Weber', 'Meyer', 'Wagner', 'Becker', 'Hoffmann', 'Schäfer', 'Koch', 'Bauer', 'Richter', 'Klein', 'Wolf', 'Neumann', 'Schwarz', 'Zimmermann', 'Braun', 'Krüger', 'Kowalski', 'Nowak', 'Łukasiewicz', 'Dvořák', 'Nielsen', 'Lindqvist', 'Dubois'];

const CYR_M = ['Иван', 'Пётр', 'Алексей', 'Николай', 'Сергей', 'Дмитрий'];
const CYR_F = ['Анна', 'Мария', 'Елена', 'Ольга', 'Наталья', 'Татьяна'];
const CYR_S = ['Иванов', 'Петров', 'Смирнов', 'Кузнецов', 'Попов', 'Соколов'];
const HEB_M = ['דוד', 'משה', 'יוסף', 'אברהם', 'יעקב'];
const HEB_F = ['שרה', 'רחל', 'רבקה', 'לאה', 'מרים'];
const HEB_S = ['כהן', 'לוי', 'מזרחי', 'פרידמן', 'שפירא'];
const CJK_M = ['伟', '强', '磊', '军', '建国'];
const CJK_F = ['芳', '娜', '敏', '静', '秀英'];
const CJK_S = ['王', '李', '张', '刘', '陈'];

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

const PLACES = [
  'Hamburg, Germany', 'Berlin, Brandenburg, Germany', 'Munich, Bavaria, Germany', 'Cologne, Rhineland, Germany',
  'Leipzig, Saxony, Germany', 'Dresden, Saxony, Germany', 'Bremen, Germany', 'Lübeck, Schleswig-Holstein, Germany',
  'Vienna, Austria', 'Prague, Bohemia', 'Kraków, Poland', 'Gdańsk, Poland', 'Warsaw, Poland', 'Copenhagen, Denmark',
  'Stockholm, Sweden', 'Gothenburg, Sweden', 'Oslo, Norway', 'Riga, Latvia', 'Moscow, Russia', 'Saint Petersburg, Russia',
  'Odessa, Ukraine', 'Kyiv, Ukraine', 'Vilnius, Lithuania', 'Zürich, Switzerland', 'Basel, Switzerland', 'Paris, France',
  'Strasbourg, Alsace, France', 'Amsterdam, Netherlands', 'Antwerp, Belgium', 'London, England', 'Liverpool, England',
  'New York, New York, USA', 'Chicago, Illinois, USA', 'Milwaukee, Wisconsin, USA', 'Cincinnati, Ohio, USA',
  'Buenos Aires, Argentina', 'Haifa, Israel', 'Jerusalem, Israel', 'Shanghai, China', 'Beijing, China',
];

const SOURCE_TITLES = [
  'Parish register of St Michael, Hamburg', 'Parish register of St Mary, Lübeck', 'Census 1880', 'Census 1900', 'Census 1910',
  'Civil registry, Berlin Mitte', 'Civil registry, Munich', 'Emigration list, Bremerhaven', 'Passenger manifest, Ellis Island',
  'Church book, Kraków', 'Jewish community register, Prague', 'Family Bible of the Schmidt line', 'Letters to Aunt Frieda',
  'Hamburg address book 1890', 'Military roll, 2nd Infantry', 'Probate record, Cologne', 'Tax roll, Leipzig 1865',
  'Naturalization index, Chicago', 'Newspaper notice, Berliner Tageblatt', 'Cemetery records, Ohlsdorf',
  'School register, Dresden', 'Guild register, Bremen', 'Oral history, interview with Clara', 'Marriage banns, Vienna',
  'Population register, Stockholm', 'Land register, Gdańsk', 'Orthodox metrical book, Riga', 'Household examination roll, Oslo',
  'Directory of Milwaukee 1905', 'Memorial book, Haifa',
];

const NOTE_TEXTS = [
  'Worked as a carpenter until the family moved to the city. Letters mention a long illness in the winter of the second year.',
  'The family story says the house stood near the river and flooded twice. Not confirmed by any record yet.',
  'Name spelled differently in each record; the church book gives the older form.',
  'Emigrated with two children and followed by her sister a year later.',
  'Served as witness at the baptism of a cousin. The godparents are listed in the register.',
];

function pick<T>(rng: () => number, list: readonly T[]): T {
  return list[Math.floor(rng() * list.length)];
}

/**
 * Builds a plausible multi-generation tree of exactly `n` people from a seeded generator, plus
 * the hand-written cases the views must survive: a person with 12 partners, a parent cycle
 * (A is a parent of B is a parent of A) and an undated person. Deterministic for (n, seed).
 */
export function fixtureTree(n: number, seed = 1): Fixture {
  const rng = mulberry32(seed);
  const ids = new IdGen(rng);

  const places: PlaceOption[] = [];
  const placeByFull = new Map<string, PlaceOption>();
  const ensurePlace = (full: string): PlaceOption => {
    const found = placeByFull.get(full.toLowerCase());
    if (found) return found;
    const parts = full.split(',').map((s) => s.trim());
    const p: PlaceOption = { id: ids.next(), name: parts[0], full, kind: null };
    placeByFull.set(full.toLowerCase(), p);
    places.push(p);
    return p;
  };
  for (const full of PLACES) {
    const parts = full.split(',').map((s) => s.trim());
    for (let i = parts.length - 1; i >= 0; i--) ensurePlace(parts.slice(i).join(', '));
  }

  const sources: SourceDto[] = SOURCE_TITLES.map((title) => ({
    id: ids.next(),
    title,
    author: null,
    publication: null,
    abbreviation: null,
    text: null,
    citationCount: 0,
  }));

  const makeCitation = (): CitationDto => {
    const src = pick(rng, sources);
    return {
      id: ids.next(),
      source: src,
      page: rng() < 0.6 ? `p. ${1 + Math.floor(rng() * 400)}` : null,
      date: null,
      text: null,
      quality: pick(rng, [0, 1, 2, 3, null] as const),
      evidence: { original: null, primaryInfo: null, direct: null },
    };
  };

  const persons: PersonRec[] = [];
  const families: FamilyRec[] = [];
  const birthYear = new Map<Id, number>();
  const byId = new Map<Id, PersonRec>();
  const media: MediaDto[] = [];
  let tick = 0;

  interface MakeOpts {
    sex: SexKind;
    year: number;
    surname?: string;
    undated?: boolean;
    given?: string;
    forceSurname?: string;
  }

  const mkPerson = (o: MakeOpts): PersonRec => {
    const id = ids.next();
    const r = rng();
    let given: string;
    let surname: string;
    if (o.given && o.forceSurname) {
      given = o.given;
      surname = o.forceSurname;
    } else if (r < 0.03) {
      const m = o.sex === 'female' ? pick(rng, CYR_F) : pick(rng, CYR_M);
      given = m;
      const s = pick(rng, CYR_S);
      surname = o.sex === 'female' ? `${s}а` : s;
    } else if (r < 0.055) {
      given = o.sex === 'female' ? pick(rng, HEB_F) : pick(rng, HEB_M);
      surname = pick(rng, HEB_S);
    } else if (r < 0.085) {
      given = o.sex === 'female' ? pick(rng, CJK_F) : pick(rng, CJK_M);
      surname = pick(rng, CJK_S);
    } else {
      given = o.sex === 'female' ? pick(rng, GIVEN_F) : pick(rng, GIVEN_M);
      surname = o.surname ?? pick(rng, SURNAMES);
    }

    const events = [];
    if (!o.undated) {
      const placeA = rng() < 0.85 ? ensurePlace(pick(rng, PLACES)) : null;
      let raw: string | null;
      const d = rng();
      if (rng() < 0.05) raw = null;
      else if (d < 0.6) raw = `${1 + Math.floor(rng() * 28)} ${pick(rng, MONTHS)} ${o.year}`;
      else if (d < 0.75) raw = `abt ${o.year}`;
      else if (d < 0.9) raw = String(o.year);
      else raw = `${pick(rng, MONTHS)} ${o.year}`;
      const birth = makeEvent(ids.next(), 'birth', { dateRaw: raw, place: placeA, preferred: true });
      if (rng() < 0.75) birth.citations.push(makeCitation());
      events.push(birth);

      const age = 35 + Math.floor(rng() * 55);
      const deathYear = o.year + age;
      const dies = o.year < 1940 ? deathYear <= 2024 : rng() < 0.12 && deathYear <= 2024;
      if (dies) {
        const death = makeEvent(ids.next(), 'death', {
          dateRaw: rng() < 0.7 ? `${1 + Math.floor(rng() * 28)} ${pick(rng, MONTHS)} ${deathYear}` : String(deathYear),
          place: rng() < 0.7 ? ensurePlace(pick(rng, PLACES)) : null,
          preferred: true,
        });
        if (rng() < 0.4) death.citations.push(makeCitation());
        events.push(death);
      }
      if (rng() < 0.25) {
        events.push(
          makeEvent(ids.next(), 'occupation', {
            description: pick(rng, ['Carpenter', 'Teacher', 'Baker', 'Seamstress', 'Clerk', 'Farmer', 'Midwife', 'Sailor']),
          }),
        );
      }
    }

    const rec: PersonRec = {
      id,
      sex: o.sex,
      sexText: null,
      livingOverride: !o.undated && o.year < 1940 && !events.some((e) => e.kind === 'death') ? 'deceased' : null,
      private: rng() < 0.025,
      names: [
        makeName(ids.next(), { kind: 'birth', preferred: true, private: false, given, surnames: [surname] }),
      ],
      events,
      notes: rng() < 0.1 ? [makeNote(ids.next(), 'general', pick(rng, NOTE_TEXTS))] : [],
      media: [],
      citations: [],
      changed: 1_700_000_000_000 + tick++ * 60_000,
    };
    if (o.sex === 'female' && o.year < 1990 && rng() < 0.3) {
      rec.names.push(
        makeName(ids.next(), { kind: 'married', preferred: false, private: false, given, surnames: [pick(rng, SURNAMES)] }),
      );
    }
    if (rng() < 0.06) {
      const mid = ids.next();
      media.push({
        id: mid,
        title: `Photograph of ${given}`,
        mime: 'image/jpeg',
        originalName: `photo-${media.length + 1}.jpg`,
        date: null,
        description: null,
        resolved: true,
        width: 1200,
        height: 900,
        sha256: null,
      });
      rec.media.push({ media: media[media.length - 1], primary: true });
    }
    persons.push(rec);
    byId.set(id, rec);
    birthYear.set(id, o.year);
    return rec;
  };

  const mkFamily = (p1: PersonRec | null, p2: PersonRec | null, year: number): FamilyRec => {
    const married = rng() < 0.85;
    const fam: FamilyRec = {
      id: ids.next(),
      partner1: p1?.id ?? null,
      partner2: p2?.id ?? null,
      kind: married ? 'married' : 'unmarried',
      events: [],
      children: [],
      notes: [],
      citations: [],
    };
    if (married && p1 && p2 && rng() < 0.8) {
      fam.events.push(
        makeEvent(ids.next(), 'marriage', {
          dateRaw: `${1 + Math.floor(rng() * 28)} ${pick(rng, MONTHS)} ${year}`,
          place: rng() < 0.8 ? ensurePlace(pick(rng, PLACES)) : null,
        }),
      );
    }
    families.push(fam);
    return fam;
  };

  // Reserve room for the hand-written cases when the tree is large enough to hold them.
  const withSpecials = n >= 60;
  const specialCount = withSpecials ? 13 + 2 + 1 : 0;
  const target = n - specialCount;

  const queue: FamilyRec[] = [];
  const seedCouple = () => {
    const year = 1775 + Math.floor(rng() * 50);
    const m = mkPerson({ sex: 'male', year });
    if (persons.length >= target) return;
    const f = mkPerson({ sex: 'female', year: year + Math.floor(rng() * 5) - 2 });
    queue.push(mkFamily(m, f, year + 24));
  };

  const childCounts = [0, 1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 5, 5, 6, 6];
  while (persons.length < target) {
    const fam = queue.shift();
    if (!fam) {
      seedCouple();
      continue;
    }
    const father = byId.get(fam.partner1!)!;
    const mother = byId.get(fam.partner2!)!;
    const fy = birthYear.get(mother.id)!;
    const kids = pick(rng, childCounts);
    for (let k = 0; k < kids && persons.length < target; k++) {
      const year = fy + 21 + k * 2 + Math.floor(rng() * 3);
      if (year > 2022) break;
      const sex: SexKind = rng() < 0.5 ? 'male' : 'female';
      const fSurname = father.names[0].surnames[0];
      const child = mkPerson({ sex, year, surname: /[Ѐ-ӿ֐-׿㐀-鿿]/.test(fSurname) ? undefined : fSurname });
      const link: ChildLinkRec = { person: child.id, rel: ['birth', 'birth'], preferred: true };
      fam.children.push(link);
      if (year < 1995 && rng() < 0.7 && persons.length < target) {
        const spouse = mkPerson({ sex: sex === 'male' ? 'female' : 'male', year: year + Math.floor(rng() * 7) - 3 });
        const cf = sex === 'male' ? mkFamily(child, spouse, year + 25) : mkFamily(spouse, child, year + 25);
        queue.push(cf);
      }
    }
  }

  const specials: Fixture['specials'] = { manyPartners: null, cycleA: null, cycleB: null, undated: null };
  if (withSpecials) {
    const henry = mkPerson({ sex: 'male', year: 1801, given: 'Heinrich', forceSurname: 'Vielfrau' });
    specials.manyPartners = henry.id;
    for (let i = 1; i <= 12; i++) {
      const wife = mkPerson({ sex: 'female', year: 1805 + i, given: `Partnerin ${i}`, forceSurname: 'Vielfrau' });
      mkFamily(henry, wife, 1825 + i);
    }
    const a = mkPerson({ sex: 'male', year: 1830, given: 'Cyril', forceSurname: 'Cycle' });
    const b = mkPerson({ sex: 'female', year: 1855, given: 'Cora', forceSurname: 'Cycle' });
    specials.cycleA = a.id;
    specials.cycleB = b.id;
    const fab = mkFamily(a, null, 1854);
    fab.children.push({ person: b.id, rel: ['birth', 'birth'], preferred: true });
    const fba = mkFamily(b, null, 1829);
    fba.children.push({ person: a.id, rel: ['birth', 'birth'], preferred: true });
    const u = mkPerson({ sex: 'unknown', year: 1900, given: 'Uma', forceSurname: 'Undated', undated: true });
    specials.undated = u.id;
  }

  // Recount source usage and choose a home person with children and a partner.
  const counts = new Map<Id, number>();
  const countCites = (cs: CitationDto[]) => cs.forEach((c) => c.source && counts.set(c.source.id, (counts.get(c.source.id) ?? 0) + 1));
  for (const p of persons) {
    countCites(p.citations);
    p.events.forEach((e) => countCites(e.citations));
    p.names.forEach((nm) => countCites(nm.citations));
  }
  for (const s of sources) s.citationCount = counts.get(s.id) ?? 0;

  const firstFamily = families.find((f) => f.children.length > 0 && f.partner1 && f.partner2) ?? families[0];
  const home = firstFamily?.children[0]?.person ?? persons[0].id;

  return {
    name: 'Mock family',
    persons,
    families,
    places,
    sources,
    media,
    homePerson: home,
    specials,
  };
}
