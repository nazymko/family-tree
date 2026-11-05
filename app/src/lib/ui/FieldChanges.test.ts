import { afterEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import FieldChanges from './FieldChanges.svelte';
import { setLanguage } from '../i18n';
import type { FieldChangeDto } from '../api/types';

afterEach(() => setLanguage('en'));

const surname: FieldChangeDto = { field: 'surnames', before: 'Koval', after: 'Kovalchuk' };

/** Each body row as text with single spaces, headings included. */
const rows = () =>
  screen
    .getAllByRole('row')
    .slice(1)
    .map((r) => [...r.children].map((c) => c.textContent?.replace(/\s+/g, ' ').trim()));

describe('FieldChanges', () => {
  it('is a table of the field, its value before and its value after', () => {
    render(FieldChanges, { fields: [surname, { field: 'given', before: 'Ivan', after: 'Olena' }] });
    const head = screen.getAllByRole('columnheader').map((h) => h.textContent);
    expect(head).toEqual(['Field', 'Before', 'After']);
    expect(rows()).toEqual([
      ['Surname', 'Koval', 'Kovalchuk'],
      ['Given name', 'Ivan', 'Olena'],
    ]);
  });

  it('wraps every value in bdi with automatic direction and keeps it as stored', () => {
    const { container } = render(FieldChanges, { fields: [{ field: 'nickname', before: ' Vanya ', after: 'Ваня' }] });
    const bdis = container.querySelectorAll('bdi');
    expect([...bdis].map((b) => b.getAttribute('dir'))).toEqual(['auto', 'auto']);
    expect([...bdis].map((b) => b.textContent)).toEqual([' Vanya ', 'Ваня']);
  });

  it('says empty for an empty side, in English', () => {
    render(FieldChanges, {
      fields: [
        { field: 'patronymic', before: null, after: 'Petrovych' },
        { field: 'suffix', before: 'Jr', after: null },
      ],
    });
    expect(rows()).toEqual([
      ['Father’s name (patronymic)', 'empty', 'Petrovych'],
      ['Suffix', 'Jr', 'empty'],
    ]);
  });

  it('heads the columns and says порожньо in Ukrainian, without the word дерево', () => {
    setLanguage('uk');
    const { container } = render(FieldChanges, { fields: [{ field: 'suffix', before: null, after: 'Jr' }] });
    expect(screen.getAllByRole('columnheader').map((h) => h.textContent)).toEqual(['Поле', 'Було', 'Стало']);
    expect(rows()).toEqual([['Суфікс', 'порожньо', 'Jr']]);
    expect(container.textContent).not.toMatch(/дерев/i);
  });

  it('words flags, kinds and sex', () => {
    render(FieldChanges, {
      fields: [
        { field: 'preferred', before: 'no', after: 'yes' },
        { field: 'kind', before: 'birth', after: 'other:Pen name' },
        { field: 'sex', before: 'male', after: 'female' },
      ],
    });
    expect(rows()).toEqual([
      ['Preferred name', 'no', 'yes'],
      ['Kind of name', 'Birth name', 'Pen name'],
      ['Sex', 'Male', 'Female'],
    ]);
  });

  it('names a name once above its rows when a person has several', () => {
    render(FieldChanges, {
      fields: [
        { field: 'call', nameNumber: 1, nameKind: 'married', before: null, after: 'Ivanko' },
        { field: 'raw', nameNumber: 1, nameKind: 'married', before: 'Ivan /Koval/', after: null },
        { field: 'position', nameNumber: 2, nameKind: 'birth', before: '2', after: '1' },
      ],
    });
    const r = rows();
    expect(r[0]).toEqual(['Name 1 (Married name)']);
    expect(r[1].slice(1)).toEqual(['empty', 'Ivanko']);
    expect(r[2].slice(1)).toEqual(['Ivan /Koval/', 'empty']);
    expect(r[3]).toEqual(['Name 2 (Birth name)']);
    expect(r[4]).toEqual(['Order among names (1 is the main name)', '2', '1']);
    expect(r).toHaveLength(5);
  });

  it('uses the plural label only when a side holds several surnames', () => {
    render(FieldChanges, { fields: [{ field: 'surnames', before: 'Koval', after: 'Koval, Petrenko' }] });
    expect(rows()).toEqual([['Surnames', 'Koval', 'Koval, Petrenko']]);
  });

  it('renders nothing for no fields', () => {
    const { container } = render(FieldChanges, { fields: [] });
    expect(container.querySelector('table')).toBeNull();
  });

  it('puts the language\'s own "and" between the partners of a family, whatever the names say', () => {
    const f: FieldChangeDto = { field: 'participant', of: 'Anna Koval\u001fUnknown', before: null, after: 'witness' };
    render(FieldChanges, { fields: [f] });
    expect(rows()[0][0]).toBe('Participant · Anna Koval and Unknown');
  });

  it('reads the partners in Ukrainian with та and a word for the unknown one', () => {
    setLanguage('uk');
    render(FieldChanges, { fields: [{ field: 'participant', of: 'Анна Коваль\u001fUnknown', before: null, after: 'witness' }] });
    expect(rows()[0][0]).toContain('Анна Коваль та ');
    expect(rows()[0][0]).not.toContain('Unknown');
    expect(rows()[0][0]).not.toContain(' and ');
  });

  it('words a record that was removed, not as empty', () => {
    render(FieldChanges, { fields: [{ field: 'place', before: '\u0001removed', after: null }, { field: 'citations', before: 'A; \u0001removed', after: null }] });
    expect(rows()).toEqual([
      ['Place', '(record removed)', 'empty'],
      ['Citations', 'A; (record removed)', 'empty'],
    ]);
  });

  it('counts the other data kept and the external ids, truthfully', () => {
    render(FieldChanges, {
      fields: [
        { field: 'unknown', before: '2', after: '0' },
        { field: 'extIds', before: null, after: 'agent:file: 42' },
        { field: 'mediaPrimary', of: 'photo.jpg', before: null, after: 'yes' },
      ],
    });
    expect(rows()).toEqual([
      ['Other data', '2 items', '0 items'],
      ['External ids', 'empty', 'agent:file: 42'],
      ['Main picture · photo.jpg', 'empty', 'yes'],
    ]);
  });
});
