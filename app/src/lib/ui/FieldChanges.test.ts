import { afterEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import FieldChanges from './FieldChanges.svelte';
import { setLanguage } from '../i18n';
import type { FieldChangeDto } from '../api/types';

afterEach(() => setLanguage('en'));

const surname: FieldChangeDto = { field: 'surnames', before: 'Koval', after: 'Kovalchuk' };

describe('FieldChanges', () => {
  it('shows one line per field with the label, the old and the new value', () => {
    render(FieldChanges, { fields: [surname, { field: 'given', before: 'Ivan', after: 'Olena' }] });
    const items = screen.getAllByRole('listitem');
    expect(items).toHaveLength(2);
    expect(items[0].textContent?.replace(/\s+/g, ' ').trim()).toBe('Surnames: Koval → Kovalchuk');
    expect(items[1].textContent?.replace(/\s+/g, ' ').trim()).toBe('Given name: Ivan → Olena');
  });

  it('wraps every value in bdi with automatic direction and keeps it as stored', () => {
    const { container } = render(FieldChanges, { fields: [{ field: 'nickname', before: ' Vanya ', after: 'Ваня' }] });
    const bdis = container.querySelectorAll('bdi');
    expect([...bdis].map((b) => b.getAttribute('dir'))).toEqual(['auto', 'auto']);
    expect([...bdis].map((b) => b.textContent)).toEqual([' Vanya ', 'Ваня']);
  });

  it('says empty for an empty side, in English', () => {
    render(FieldChanges, { fields: [{ field: 'patronymic', before: null, after: 'Petrovych' }, { field: 'suffix', before: 'Jr', after: null }] });
    const items = screen.getAllByRole('listitem');
    expect(items[0].textContent?.replace(/\s+/g, ' ').trim()).toBe('Father’s name (patronymic): empty → Petrovych');
    expect(items[1].textContent?.replace(/\s+/g, ' ').trim()).toBe('Suffix: Jr → empty');
  });

  it('says порожньо for an empty side, in Ukrainian', () => {
    setLanguage('uk');
    render(FieldChanges, { fields: [{ field: 'suffix', before: null, after: 'Jr' }] });
    expect(screen.getByRole('listitem').textContent?.replace(/\s+/g, ' ').trim()).toBe('Суфікс: порожньо → Jr');
  });

  it('words flags, kinds and sex, and names the name when there are several', () => {
    render(FieldChanges, {
      fields: [
        { field: 'preferred', nameNumber: 2, nameKind: 'married', before: 'no', after: 'yes' },
        { field: 'kind', nameNumber: 1, nameKind: 'birth', before: 'birth', after: 'other:Pen name' },
        { field: 'sex', before: 'male', after: 'female' },
        { field: 'position', nameNumber: 2, nameKind: 'married', before: '2', after: '1' },
      ],
    });
    const text = screen.getAllByRole('listitem').map((li) => li.textContent?.replace(/\s+/g, ' ').trim());
    expect(text[0]).toBe('Name 2 (Married name) Preferred name: no → yes');
    expect(text[1]).toBe('Name 1 (Birth name) Kind of name: Birth name → Pen name');
    expect(text[2]).toBe('Sex: Male → Female');
    expect(text[3]).toBe('Name 2 (Married name) Order among names (1 is the main name): 2 → 1');
  });

  it('words the same in Ukrainian without the word дерево', () => {
    setLanguage('uk');
    render(FieldChanges, { fields: [{ ...surname, nameNumber: 2, nameKind: 'married' }] });
    const text = screen.getByRole('listitem').textContent?.replace(/\s+/g, ' ').trim() ?? '';
    expect(text).toBe('Ім’я 2 (Ім’я в шлюбі) Прізвища: Koval → Kovalchuk');
    expect(text).not.toMatch(/дерев/i);
  });

  it('renders nothing for no fields', () => {
    const { container } = render(FieldChanges, { fields: [] });
    expect(container.querySelector('ul')).toBeNull();
  });
});
