import { beforeEach, describe, expect, it } from 'vitest';
import { makeFakeContext } from '../../test-setup';
import { setLanguage } from '../i18n';
import { draw, type Theme } from '../chart/renderer';
import type { ChartNode, ChartWindow, PersonRef } from './types';
import { localizeApi, localizeNames } from './localize';
import { MockApi } from './mock/MockApi';
import { fixtureTree } from './mock/fixtures';

const ref = (name: string): PersonRef => ({ id: 'p1', name, lifespan: '', sex: 'male', living: false, private: false, siblingMarker: false });
const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };

beforeEach(() => setLanguage('en'));

describe('the name of a person without one', () => {
  it('reads_in_the_active_language_and_follows_a_language_switch', () => {
    const shown = localizeNames({ person: ref('Unknown') });
    expect(shown.person.name).toBe('Unknown');
    setLanguage('uk');
    expect(shown.person.name).toBe('Ім’я невідоме');
    setLanguage('en');
    expect(shown.person.name).toBe('Unknown');
  });

  it('leaves_real_names_places_name_parts_and_the_input_alone', () => {
    setLanguage('uk');
    const input = { person: ref('Unknown'), place: { id: 'x', name: 'Unknown', full: 'Unknown', kind: null }, other: ref('Ivan Koval') };
    const out = localizeNames(input);
    expect(out.place).toBe(input.place);
    expect(out.other).toBe(input.other);
    const name = { id: 'n', display: 'Unknown', given: 'Unknown', surnames: [] };
    expect(localizeNames(name)).toBe(name);
  });

  it('covers_people_rows_agents_connections_warnings_and_person_search_hits', () => {
    setLanguage('uk');
    const word = 'Ім’я невідоме';
    const row = { id: 'p', name: 'Unknown', surname: '', given: '', birth: '', death: '', lifespan: '', siblingMarker: false };
    expect(localizeNames(row).name).toBe(word);
    expect(localizeNames({ kind: 'agent', id: 'a', name: 'Unknown' }).name).toBe(word);
    expect(localizeNames({ id: 'c', name: 'Unknown', created: 1 }).name).toBe(word);
    expect(localizeNames({ noteExcerpt: 'x', hiddenPerson: 'Unknown' }).hiddenPerson).toBe(word);
    expect(localizeNames([{ id: 'a', name: 'Unknown' }], 'changeLogAgents')[0].name).toBe(word);
    expect(localizeNames([{ id: 'a', name: 'Unknown' }], 'listPlaces')[0].name).toBe('Unknown');
    const hit = { kind: 'person', id: 'p', label: '', snippet: null, person: ref('Unknown') };
    expect(localizeNames(hit).label).toBe(word);
    const named = { kind: 'person', id: 'p', label: '', snippet: null, person: ref('Olena Koval') };
    expect(localizeNames(named).label).toBe('Olena Koval');
  });

  it('lifespans_dates_and_summaries_use_native_words_and_english_is_unchanged', () => {
    const shown = localizeNames({
      ref: { ...ref('Ivan Petrovych Koval'), lifespan: 'c. 1791 – bef. 1857' },
      cards: [{ ...ref('Olena Koval'), lifespan: 'b. 1970' }, { ...ref('Olena Koval'), lifespan: 'c. 44 BCE – ?' }],
      date: { raw: 'abt 1901', display: 'about 3 Apr 1901', status: 'ok', sortMin: 1, sortMax: 2 },
      union: { kind: 'married', summary: 'm. 1 Jun 1878, Hamburg', events: [] },
      focusSummary: 'b. 1 Jan 1950, Lviv; d. 2001',
    });
    expect(shown.ref.lifespan).toBe('c. 1791 – bef. 1857');
    expect(shown.cards[0].lifespan).toBe('b. 1970');
    expect(shown.date.display).toBe('about 3 Apr 1901');
    expect(shown.union.summary).toBe('m. 1 Jun 1878, Hamburg');
    expect(shown.focusSummary).toBe('b. 1 Jan 1950, Lviv; d. 2001');
    setLanguage('uk');
    expect(shown.ref.lifespan).toBe('бл. 1791 – до 1857');
    expect(shown.cards[0].lifespan).toBe('нар. 1970');
    expect(shown.cards[1].lifespan).toBe('бл. 44 до н. е. – ?');
    expect(shown.date.raw).toBe('abt 1901');
    expect(shown.date.display).toBe('близько 3 квіт 1901');
    expect(shown.union.summary).toBe('одр. 1 черв 1878, Hamburg');
    expect(shown.focusSummary).toBe('нар. 1 січ 1950, Lviv; пом. 2001');
  });

  it('is_drawn_on_a_chart_in_the_active_language_even_after_a_switch', () => {
    const nodes: ChartNode[] = [
      {
        id: 0, person: ref('Unknown'), x: 0, y: 0, w: 176, h: 56, gen: 0, ahnen: 1, dupOf: null, moreUp: false, moreDown: false,
        nav: { parent: null, child: null, prev: null, next: null }, label: '',
      },
    ];
    const win = localizeNames<ChartWindow>({ nodes, edges: [] });
    const texts = () => {
      const ctx = makeFakeContext();
      draw(ctx, win, { x: 0, y: 0, zoom: 1 }, { w: 800, h: 600, dpr: 1 }, { selected: null, focus: null, theme });
      return ctx.calls.filter((c) => c.op === 'fillText').map((c) => String(c.args[0]));
    };
    expect(texts().join(' ')).toContain('Unknown');
    setLanguage('uk');
    const uk = texts().join(' ');
    expect(uk).toContain('Ім’я');
    expect(uk).not.toContain('Unknown');
  });
});

describe('the wrapped Api', () => {
  it('words_a_nameless_person_in_a_search_and_the_people_list_and_keeps_the_stored_word', async () => {
    const raw = new MockApi(fixtureTree(30));
    await raw.openProject('C:/Genealogy/Test');
    const made = await raw.apply({
      type: 'createPerson',
      draft: { sex: 'unknown', surnames: [] } as never,
    });
    const id = (made as { focus: string }).focus;
    const api = localizeApi(raw);
    setLanguage('uk');
    expect((await api.getPerson(id)).ref.name).toBe('Ім’я невідоме');
    const rows = await api.listPersons({ sort: 'surname', desc: false, text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 500);
    expect(rows.items.find((r) => r.id === id)?.name).toBe('Ім’я невідоме');
    // The raw Api, which is what MCP agents and export read, keeps the literal word.
    expect((await raw.getPerson(id)).ref.name).toBe('Unknown');
    expect(localizeApi(api)).toBe(api);
  });
});
