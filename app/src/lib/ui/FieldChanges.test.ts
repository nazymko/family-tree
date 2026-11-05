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
});
