import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import Harness from './testing/DateFieldHarness.svelte';
import { composeChoice, isoFromParts, parseChoice, partsFromIso } from './dateParts';
import { newApi, renderWithApi } from './testing/helpers';
import { previewDate } from '../api/mock/mockDate';
import { setLanguage } from '../i18n';

afterEach(() => setLanguage('en'));

function setup(value = '') {
  const onChange = vi.fn();
  const api = newApi(5);
  renderWithApi(api, Harness, { initial: value, spy: onChange });
  return { onChange, user: userEvent.setup(), api };
}
const open = (user: ReturnType<typeof userEvent.setup>, name = 'Pick a date or a range') => user.click(screen.getByRole('button', { name }));
const kind = () => screen.getByRole('combobox', { name: 'Kind of date' }) as HTMLSelectElement;
const years = () => screen.getAllByRole('textbox', { name: 'Year' }) as HTMLInputElement[];

describe('date choice text', () => {
  it('writes_text_the_parser_accepts', () => {
    const d = { year: '1850', month: '3', day: '7' };
    const e = { year: '1860', month: '', day: '' };
    const cases = [
      composeChoice({ kind: 'exact', a: d, b: e }),
      composeChoice({ kind: 'about', a: d, b: e }),
      composeChoice({ kind: 'before', a: e, b: e }),
      composeChoice({ kind: 'after', a: e, b: e }),
      composeChoice({ kind: 'between', a: d, b: e }),
    ];
    expect(cases).toEqual(['7 Mar 1850', 'about 7 Mar 1850', 'before 1860', 'after 1860', 'between 7 Mar 1850 and 1860']);
    for (const raw of cases) expect(previewDate(raw!, 'dmy').status, raw!).toBe('ok');
  });

  it('a_half_filled_range_writes_nothing', () => {
    const d = { year: '1850', month: '', day: '' };
    expect(composeChoice({ kind: 'between', a: d, b: { year: '', month: '', day: '' } })).toBeNull();
  });

  it('reads_back_what_it_writes_and_leaves_other_forms_alone', () => {
    for (const raw of ['', '1850', 'about 1850', 'before Mar 1850', 'after 7 Mar 1850', 'between 1850 and 1860']) {
      expect(composeChoice(parseChoice(raw)!), raw).toBe(raw);
    }
    expect(parseChoice('abt 1850')?.kind).toBe('about');
    for (const raw of ['from 1850 to 1860', '03.04.1901', '(the big flood)', 'about 1850/51']) expect(parseChoice(raw), raw).toBeNull();
  });

  it('native_calendar_values_round_trip_including_old_years', () => {
    expect(partsFromIso('0850-03-07')).toEqual({ year: '850', month: '3', day: '7' });
    expect(isoFromParts({ year: '850', month: '3', day: '7' })).toBe('0850-03-07');
    expect(isoFromParts({ year: '1850', month: '3', day: '' })).toBe('');
    expect(partsFromIso('')).toBeNull();
  });
});

describe('date picker in DateField', () => {
  it('opening_it_does_not_rewrite_the_value', async () => {
    const { onChange, user } = setup('abt 1850');
    await open(user);
    expect(onChange).not.toHaveBeenCalled();
    expect(kind().value).toBe('about');
    expect(years()[0].value).toBe('1850');
    expect(screen.getByRole('textbox', { name: 'Date of death' })).toHaveValue('abt 1850');
  });

  it('a_value_it_cannot_show_is_kept_until_changed', async () => {
    const { onChange, user } = setup('(the big flood)');
    await open(user);
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByText(/written another way/)).toBeInTheDocument();
  });

  it('picking_fills_the_text_and_the_preview', async () => {
    const { onChange, user } = setup();
    await open(user);
    await user.type(years()[0], '1704');
    expect(onChange).toHaveBeenLastCalledWith('1704');
    expect(screen.getByRole('textbox', { name: 'Date of death' })).toHaveValue('1704');
    expect(await screen.findByText(/Understood as: 1704/)).toBeInTheDocument();
    await user.selectOptions(kind(), 'about');
    expect(onChange).toHaveBeenLastCalledWith('about 1704');
    expect(await screen.findByText(/Understood as: about 1704/)).toBeInTheDocument();
  });

  it('between_needs_both_years_then_writes_the_range', async () => {
    const { onChange, user } = setup();
    await open(user);
    await user.selectOptions(kind(), 'between');
    await user.type(years()[0], '1850');
    expect(onChange).not.toHaveBeenCalledWith(expect.stringContaining('between'));
    expect(screen.getByText(/Give a year for each date/)).toBeInTheDocument();
    await user.type(years()[1], '1860');
    expect(onChange).toHaveBeenLastCalledWith('between 1850 and 1860');
    expect(await screen.findByText(/Understood as: between 1850 and 1860/)).toBeInTheDocument();
  });

  it('the_native_calendar_fills_a_full_date', async () => {
    const { onChange } = setup();
    await userEvent.click(screen.getByRole('button', { name: 'Pick a date or a range' }));
    const cal = screen.getByLabelText('Or pick from a calendar') as HTMLInputElement;
    expect(cal.min).toBe('0001-01-01');
    await fireEvent.change(cal, { target: { value: '1750-03-07' } });
    expect(onChange).toHaveBeenLastCalledWith('7 Mar 1750');
  });

  it('raw_typing_is_untouched_and_the_picker_follows_it', async () => {
    const { onChange, user } = setup();
    await open(user);
    const box = screen.getByRole('textbox', { name: 'Date of death' });
    await user.type(box, 'between 1801 and 1810');
    expect(onChange).toHaveBeenLastCalledWith('between 1801 and 1810');
    expect(box).toHaveValue('between 1801 and 1810');
    expect(kind().value).toBe('between');
    expect(years().map((y) => y.value)).toEqual(['1801', '1810']);
  });

  it('is_labelled_in_ukrainian', async () => {
    setLanguage('uk');
    const { user } = setup();
    await open(user, 'Вибрати дату або проміжок');
    expect(screen.getByRole('combobox', { name: 'Вид дати' })).toBeInTheDocument();
    const labels = Array.from((screen.getByRole('combobox', { name: 'Вид дати' }) as HTMLSelectElement).options).map((o) => o.text);
    expect(labels).toEqual(['Точна дата', 'Близько (приблизно)', 'До', 'Після', 'Між двома датами']);
  });
});
