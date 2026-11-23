// Invented data for the places view: tests and screenshots. No real names.
import type { MockApi } from '../api/mock/MockApi';
import type { PlaceEventDto } from '../api/types';

const OTHERS = ['Bondar', 'Melnyk', 'Shevchuk', 'Tkach', 'Savchenko', 'Kravets'];
const GIVEN = ['Danylo', 'Hanna', 'Petro', 'Olena', 'Stepan', 'Maria'];

/** A village with two relatives born in it and `n` records about other people, 1820 to 1899. */
export async function seedPlaceDemo(api: MockApi, n = 60) {
  const born = (given: string, surname: string, year: string) =>
    api.apply({ type: 'createPerson', draft: { given, surnames: [surname], sex: 'male', birthDateRaw: year, birthPlaceText: 'Zelenivka, Testova oblast' } });
  const a = (await born('Ivan', 'Koval', '1860')).focus!;
  const b = (await born('Andrii', 'Koval', '1884')).focus!;
  const place = (await api.suggestPlaces('Zelenivka', 5)).find((p) => p.name === 'Zelenivka')!;
  const kinds = ['birth', 'marriage', 'death', 'confession', 'revision', 'census'];
  const records: (PlaceEventDto & { place: string })[] = [];
  for (let i = 0; i < n; i++) {
    const year = 1820 + ((i * 7) % 80);
    const kind = kinds[i % kinds.length];
    const surname = OTHERS[i % OTHERS.length];
    records.push({
      key: `r:${i}`,
      source: 'record',
      ours: false,
      kind,
      kindDetail: '',
      date: String(year),
      year,
      place: place.id,
      people: [{ name: `${GIVEN[i % GIVEN.length]} ${surname}`, asWritten: '', role: kind === 'birth' ? 'child' : 'head', age: kind === 'death' ? '40' : '', estate: '', note: '', person: null, living: false }],
      book: 'BK-1',
      page: String(10 + (i % 30)),
      recId: `bk1:${i}`,
      note: '',
      unsure: i % 9 === 0,
    });
  }
  api.placeRecords = records;
  api.placeImportSample = {
    files: 2,
    rows: 540,
    badRows: 1,
    groups: [
      { book: 'BK-1', village: 'Zelenivka', records: 120, rows: 300, yearFrom: 1820, yearTo: 1899, kinds: [{ kind: 'birth', count: 60 }, { kind: 'death', count: 60 }] },
      { book: 'BK-2', village: 'Hirka', records: 80, rows: 240, yearFrom: 1850, yearTo: 1870, kinds: [{ kind: 'marriage', count: 80 }] },
    ],
  };
  return { place, relatives: [a, b] };
}
