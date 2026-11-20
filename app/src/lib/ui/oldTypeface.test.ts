import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { makeFakeContext } from '../../test-setup';
import type { ChartNode, PersonRow as PersonRowDto } from '../api/types';
import { draw, type Theme } from '../chart/renderer';
import PersonRow from './PersonRow.svelte';
import NameEditor from '../editors/NameEditor.svelte';
import { insertAtCaret } from '../editors/oldLetters';
import { newApi, renderWithApi } from '../editors/testing/helpers';
import { loadOldTypeface } from './fonts';
import { NAMES_AS_RECORDED_KEY, OLD_TYPEFACE_KEY, namesAsRecorded, oldClass, oldTypeface, usesOldTypeface } from './namesPref.svelte';

// Invented people only.
const theme: Theme = { paper: '#fff', rule: '#ccc', ink: '#000', ink2: '#666', ribbon: '#f00', wash: '#fee', font: 'sans-serif', serif: 'serif' };
const view = { w: 800, h: 600, dpr: 2 };
const fedir = { name: 'Fedir Petrovych Vyhadanenko', asRecorded: 'Ѳеодоръ Петровъ Вигаданенко' };

beforeEach(() => {
  localStorage.removeItem(NAMES_AS_RECORDED_KEY);
  localStorage.removeItem(OLD_TYPEFACE_KEY);
  namesAsRecorded.reload();
  oldTypeface.reload();
});
afterEach(() => {
  localStorage.removeItem(NAMES_AS_RECORDED_KEY);
  localStorage.removeItem(OLD_TYPEFACE_KEY);
  namesAsRecorded.reload();
  oldTypeface.reload();
});

describe('old typeface setting', () => {
  it('is_on_by_default_and_remembered', () => {
    expect(oldTypeface.value).toBe(true);
    oldTypeface.set(false);
    expect(localStorage.getItem(OLD_TYPEFACE_KEY)).toBe('0');
    oldTypeface.value = true;
    oldTypeface.reload();
    expect(oldTypeface.value).toBe(false);
    oldTypeface.set(true);
    oldTypeface.reload();
    expect(oldTypeface.value).toBe(true);
  });

  it('applies_only_to_a_name_shown_as_recorded', () => {
    expect(usesOldTypeface(fedir)).toBe(false); // names as recorded is off
    namesAsRecorded.set(true);
    expect(usesOldTypeface(fedir)).toBe(true);
    expect(oldClass(fedir)).toBe('old-typeface');
    expect(oldClass({ name: 'Olena Vyhadanenko' })).toBeUndefined();
    oldTypeface.set(false);
    expect(oldClass(fedir)).toBeUndefined();
  });

  it('a_people_row_gets_the_class_for_a_recorded_name_only', () => {
    namesAsRecorded.set(true);
    const row = { id: 'p1', ...fedir, lifespan: '', sex: 'male', living: false, private: false, siblingMarker: false } as PersonRowDto;
    const { container, unmount } = render(PersonRow, { row });
    expect(container.querySelector('.name')).toHaveClass('old-typeface');
    unmount();
    const modern = render(PersonRow, { row: { ...row, asRecorded: undefined } });
    expect(modern.container.querySelector('.name')).not.toHaveClass('old-typeface');
  });

  it('a_chart_card_draws_the_recorded_name_in_old_standard_tt_and_a_modern_one_in_the_serif', () => {
    const mk = (person: ChartNode['person']): ChartNode => ({
      id: 0, person, x: 0, y: 0, w: 300, h: 66, gen: 0, ahnen: 1, dupOf: null, moreUp: false, moreDown: false,
      nav: { parent: null, child: null, prev: null, next: null }, label: '',
    });
    const person = { id: 'p0', ...fedir, lifespan: '', sex: 'male', living: false, private: false, siblingMarker: false } as ChartNode['person'];
    const nameFont = (n: ChartNode): string => {
      const ctx = makeFakeContext();
      draw(ctx, { nodes: [n], edges: [] }, { x: 0, y: 0, zoom: 1 }, view, { selected: null, focus: null, theme });
      const i = ctx.calls.findIndex((c) => c.op === 'fillText' && (String(c.args[0]).includes('Вигаданенко') || String(c.args[0]).includes('Vyhadanenko')));
      const fonts = ctx.calls.slice(0, i).filter((c) => c.op === 'set:font');
      return String(fonts[fonts.length - 1].args[0]);
    };
    expect(nameFont(mk(person))).not.toContain('Old Standard TT'); // modern name shown
    namesAsRecorded.set(true);
    expect(nameFont(mk(person))).toContain("'Old Standard TT'");
    oldTypeface.set(false);
    expect(nameFont(mk(person))).not.toContain('Old Standard TT');
  });

  it('loads_the_font_before_drawing', async () => {
    const load = vi.fn().mockResolvedValue([]);
    await loadOldTypeface({ fonts: { load } } as unknown as Document);
    expect(load).toHaveBeenCalled();
    expect(String(load.mock.calls[0][0])).toContain('Old Standard TT');
    expect(String(load.mock.calls[0][1])).toContain('ѣ');
  });
});

describe('insertAtCaret', () => {
  it('inserts_at_the_caret_and_over_a_selection', () => {
    expect(insertAtCaret('Федор', 3, 3, 'ѳ')).toEqual({ value: 'Федѳор', caret: 4 });
    expect(insertAtCaret('Федор', 0, 1, 'Ѳ')).toEqual({ value: 'Ѳедор', caret: 1 });
    expect(insertAtCaret('', 0, 0, 'ъ')).toEqual({ value: 'ъ', caret: 1 });
    expect(insertAtCaret('аб', 9, 9, 'ѣ')).toEqual({ value: 'абѣ', caret: 3 });
  });
});

describe('old-letter palette in the name editor', () => {
  it('inserts_at_the_caret_of_the_focused_field_and_keeps_focus', async () => {
    const user = userEvent.setup();
    const api = newApi(10);
    const id = (await api.apply({ type: 'createPerson', draft: { given: 'Fedir', surnames: ['Vyhadanenko'], sex: 'male' } })).focus!;
    renderWithApi(api, NameEditor, { person: id, onSaved: vi.fn(), onCancel: vi.fn() });
    const given = screen.getByRole('textbox', { name: 'Given name' }) as HTMLInputElement;
    await user.type(given, 'Feodor');
    given.setSelectionRange(1, 1);
    await user.click(screen.getByRole('button', { name: /Insert ѣ: yat/ }));
    await waitFor(() => expect(given).toHaveValue('Fѣeodor'));
    expect(given).toHaveFocus();
    expect(given.selectionStart).toBe(2);
    // The next letter lands after the first one, in the surname when that field was last focused.
    const surname = screen.getByRole('textbox', { name: 'Surnames' }) as HTMLInputElement;
    await user.click(surname);
    await user.keyboard('Petrov');
    await user.click(screen.getByRole('button', { name: /Insert ъ: hard sign/ }));
    await waitFor(() => expect(surname).toHaveValue('Petrovъ'));
    expect(surname).toHaveFocus();
  });

  it('every_button_is_labelled_and_reachable_by_keyboard', async () => {
    const user = userEvent.setup();
    const api = newApi(10);
    const id = (await api.apply({ type: 'createPerson', draft: { given: 'Fedir', surnames: [], sex: 'male' } })).focus!;
    renderWithApi(api, NameEditor, { person: id, onSaved: vi.fn(), onCancel: vi.fn() });
    const group = screen.getByRole('group', { name: 'Old letters' });
    const buttons = Array.from(group.querySelectorAll('button'));
    expect(buttons.map((b) => b.textContent)).toEqual(['Ѣ', 'ѣ', 'І', 'і', 'Ѳ', 'ѳ', 'Ѵ', 'ѵ', 'Ъ', 'ъ']);
    for (const b of buttons) {
      expect(b.getAttribute('aria-label')).toMatch(/^Insert .: /);
      expect(b.getAttribute('title')).toBe(b.getAttribute('aria-label'));
    }
    const given = screen.getByRole('textbox', { name: 'Given name' }) as HTMLInputElement;
    await user.click(given);
    buttons[4].focus();
    await user.keyboard('{Enter}');
    await waitFor(() => expect(given.value).toContain('Ѳ'));
    expect(given).toHaveFocus();
  });
});
