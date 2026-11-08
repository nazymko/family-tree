import { afterEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import FieldChanges from './FieldChanges.svelte';
import { setLanguage } from '../i18n';
import type { FieldChangeDto } from '../api/types';

afterEach(() => setLanguage('en'));

const words = (n: number, p: string) => Array.from({ length: n }, (_, k) => `${p}${k}`).join(' ');
const small: FieldChangeDto = { field: 'text', before: 'The red house stands here', after: 'The big house stands here' };

describe('FieldChanges text diff', () => {
  const note: FieldChangeDto = {
    field: 'text',
    before: `${words(60, 'alpha')} (a removed remark here) ${words(60, 'omega')}`,
    after: `${words(60, 'alpha')} ${words(60, 'omega')}`,
  };

  it('marks_removed_words_with_del_and_added_words_with_ins', () => {
    const { container } = render(FieldChanges, { fields: [small] });
    expect([...container.querySelectorAll('del')].map((e) => e.textContent)).toEqual(['red']);
    expect([...container.querySelectorAll('ins')].map((e) => e.textContent)).toEqual(['big']);
  });

  it('says_how_many_words_were_removed_and_added', () => {
    render(FieldChanges, { fields: [small] });
    expect(document.body.textContent).toContain('Removed 1 word, added 1 word');
  });

  it('uses_ukrainian_plural_forms_in_the_summary', () => {
    setLanguage('uk');
    const summaryFor = (n: number) => {
      const { unmount } = render(FieldChanges, { fields: [{ field: 'text', before: `Початок ${words(n, 'слово')} кінець`, after: 'Початок кінець' }] });
      const s = document.querySelector('.sum')?.textContent;
      unmount();
      return s;
    };
    expect(summaryFor(1)).toBe('Вилучено 1 слово');
    expect(summaryFor(2)).toBe('Вилучено 2 слова');
    expect(summaryFor(6)).toBe('Вилучено 6 слів');
    expect(summaryFor(21)).toBe('Вилучено 21 слово');
  });

  it('shows_only_the_changes_with_gaps_for_a_long_text_and_expands_on_request', async () => {
    const { container } = render(FieldChanges, { fields: [note] });
    const cell = () => container.querySelectorAll('td')[0].textContent ?? '';
    expect(cell()).toContain('…');
    expect(cell()).toContain('(a removed remark here)');
    expect(cell()).not.toContain('alpha5 ');
    const toggle = screen.getByRole('button', { name: 'Show full text' });
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    await userEvent.click(toggle);
    expect(cell()).not.toContain('…');
    expect(cell()).toContain('alpha5 ');
    await userEvent.click(screen.getByRole('button', { name: 'Show changes only' }));
    expect(cell()).toContain('…');
  });

  it('shows_a_short_text_in_full_without_a_toggle', () => {
    render(FieldChanges, { fields: [small] });
    expect(screen.queryByRole('button')).toBeNull();
    expect(document.body.textContent).not.toContain('…');
  });

  it('leaves_created_and_non_text_values_without_marks', () => {
    const { container } = render(FieldChanges, {
      fields: [
        { field: 'text', before: null, after: 'A new note about the house' },
        { field: 'sex', before: 'male', after: 'female' },
      ],
    });
    expect(container.querySelectorAll('del, ins')).toHaveLength(0);
  });
});

const surname: FieldChangeDto ={ field: 'surnames', before: 'Koval', after: 'Kovalchuk' };

/** Each body row as text with single spaces, headings included. */
const rows = () =>
  screen
    .getAllByRole('row')
    .slice(1)
    .map((r) => [...r.children].map((c) => c.textContent?.replace(/\s+/g, ' ').trim()));

describe('FieldChanges', () => {
  it('names_the_unnamed_siblings_entry_instead_of_unknown', () => {
    render(FieldChanges, { fields: [{ field: 'child', before: null, after: '\u0001siblings', of: '\u0001siblings' }] });
    expect(document.body.textContent).toContain('<Some more Sisters/Brothers>');
    expect(document.body.textContent).not.toContain('\u0001');
  });

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
      ['Sex', '♂ Male', '♀ Female'],
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

  it('keeps the rows of two names apart by name id even when number and kind read the same', () => {
    render(FieldChanges, {
      fields: [
        { field: 'position', nameNumber: 1, nameKind: 'birth', nameId: 'n2', before: '2', after: '1' },
        { field: 'given', nameNumber: 1, nameKind: 'birth', nameId: 'n1', before: 'Ivan', after: null },
        { field: 'surnames', nameNumber: 1, nameKind: 'birth', nameId: 'n1', before: 'Koval', after: null },
      ],
    });
    const r = rows();
    expect(r.map((x) => x[0])).toEqual([
      'Name 1 (Birth name)',
      'Order among names (1 is the main name)',
      'Name 1 (Birth name)',
      'Given name',
      'Surname',
    ]);
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
