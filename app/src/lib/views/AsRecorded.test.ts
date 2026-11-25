import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import PersonEditor from './PersonEditor.svelte';
import { newApi, renderWithApi } from '../editors/testing/helpers';
import type { MockApi } from '../api/mock/MockApi';
import type { Id, NameDto } from '../api/types';
import { settings } from '../state/settings.svelte';
import { setLanguage } from '../i18n';
import { asRecordedLines } from './asRecorded';
import { modernVariants } from '../api/mock/orthography';

// Invented people only. A name as a 19th-century metric book writes it, next to the modern main name.
let api: MockApi;
let id: Id;

async function addRecordedName(cite: boolean): Promise<Id> {
  const r = await api.apply({
    type: 'saveName',
    person: id,
    name: { kind: 'as_recorded', preferred: false, private: false, given: 'Ѳеодоръ', patronymic: 'Петровъ', surnames: ['Вигаданенко'], dateRaw: '1885' },
  });
  const nameId = r.touched[r.touched.length - 1];
  if (cite) {
    await api.apply({
      type: 'addCitation',
      target: { kind: 'name', id: nameId },
      citation: { source: { title: 'Метрична книга вигаданої церкви' }, page: 'арк. 12 зв., № 34', dateRaw: '12 Mar 1885' },
    });
  }
  return nameId;
}

beforeEach(async () => {
  api = newApi();
  const r = await api.apply({ type: 'createPerson', draft: { given: 'Федір', surnames: ['Вигаданенко'], patronymic: 'Петрович', sex: 'male' } });
  id = r.focus!;
  await settings.load(api);
});

afterEach(() => setLanguage('en'));

describe('a name as the record writes it', () => {
  it('shows_under_the_main_name_with_the_source_and_year', async () => {
    await addRecordedName(true);
    renderWithApi(api, PersonEditor, { id });
    const line = await waitFor(() => {
      const el = document.querySelector('[data-as-recorded]');
      if (!el) throw new Error('no line yet');
      return el as HTMLElement;
    });
    expect((await screen.findAllByRole('heading', { level: 1 }))[0].textContent).toContain('Федір');
    expect(line.textContent).toContain('As recorded:');
    expect(line.textContent).toContain('Ѳеодоръ Петровъ Вигаданенко');
    expect(line.textContent).toContain('Метрична книга вигаданої церкви, 1885');
    const button = within(line).getByRole('button');
    expect(button.getAttribute('title')).toContain('арк. 12 зв., № 34');
    // The names section lists the record it comes from, where more can be added.
    const sources = document.querySelector('[data-name-sources]') as HTMLElement;
    expect(sources.textContent).toContain('Метрична книга вигаданої церкви, арк. 12 зв., № 34');
    // A click opens the name, with its kind.
    await userEvent.click(button);
    const dialog = await screen.findByRole('dialog');
    expect((within(dialog).getByLabelText('Given name') as HTMLInputElement).value).toBe('Ѳеодоръ');
    expect(dialog.textContent).toContain('As recorded in the source');
  });

  it('says_no_source_is_cited_yet_in_ukrainian_too', async () => {
    setLanguage('uk');
    await addRecordedName(false);
    renderWithApi(api, PersonEditor, { id });
    const line = await waitFor(() => {
      const el = document.querySelector('[data-as-recorded]');
      if (!el) throw new Error('no line yet');
      return el as HTMLElement;
    });
    expect(line.textContent).toContain('Як записано:');
    expect(line.textContent).toContain('джерело ще не вказано');
  });

  it('the_name_editor_offers_the_kind', async () => {
    renderWithApi(api, PersonEditor, { id });
    await screen.findAllByRole('heading', { level: 1 });
    await userEvent.click(screen.getAllByRole('button', { name: 'Add name' })[0]);
    const dialog = await screen.findByRole('dialog');
    const kind = within(dialog).getByLabelText('Kind of name') as HTMLSelectElement;
    expect([...kind.options].map((o) => o.value)).toContain('as_recorded');
  });
});

describe('search by the recorded spelling', () => {
  it('finds_the_person_by_todays_letters_and_by_the_old_ones', async () => {
    await addRecordedName(false);
    for (const q of ['Феодор', 'феодор петров', 'Ѳеодоръ', 'Вигаданенко']) {
      const hits = await api.search(q, 10);
      expect(hits.some((h) => h.kind === 'person' && h.id === id), q).toBe(true);
    }
    const rows = await api.listPersons({ sort: 'surname', desc: false, text: 'Феодор Петров', living: 'any', noSource: false, noBirthDate: false, privateOnly: false }, 0, 10);
    expect(rows.items.map((r) => r.id)).toContain(id);
  });

  it('maps_only_letters_like_the_rust_side', () => {
    expect(modernVariants('Іван Петрович')).toEqual([]);
    expect(modernVariants('Ѳеодоръ Петровъ')).toEqual(['феодор петров']);
    expect(modernVariants('Іоаннъ Василіевъ')).toEqual(['іоанн василіев', 'иоанн василиев']);
    expect(modernVariants('Вѣра')).toEqual(['віра', 'вера']);
    expect(modernVariants('объявленъ')).toEqual(['объявлен']);
  });

  it('lines_skip_the_preferred_name_and_take_the_year_from_the_citation_or_the_name', () => {
    const base = { kindText: null, private: false, display: 'X', prefix: null, given: 'X', call: null, surnamePrefix: null, surnames: [], patronymic: null, suffix: null, nickname: null, lang: null, citations: [] };
    const date = (display: string) => ({ raw: display, display, status: 'ok', sortMin: null, sortMax: null }) as NameDto['date'];
    const names = [
      { ...base, id: 'a', kind: 'as_recorded', preferred: true, date: null },
      { ...base, id: 'b', kind: 'as_recorded', preferred: false, date: date('ABT 1890') },
      { ...base, id: 'c', kind: 'aka', preferred: false, date: null },
    ] as NameDto[];
    const lines = asRecordedLines(names);
    expect(lines.map((l) => l.name.id)).toEqual(['b']);
    expect(lines[0].year).toBe('1890');
    expect(lines[0].source).toBeNull();
  });
});
