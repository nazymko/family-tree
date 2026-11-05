import { describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import DatePartsField from './DatePartsField.svelte';
import { composeDate, daysInMonth, parseDateParts } from './dateParts';
import { newApi, renderWithApi } from './testing/helpers';

function setup(value = '') {
  const onChange = vi.fn();
  renderWithApi(newApi(5), DatePartsField, { label: 'Birth date', value, onChange, order: 'dmy' });
  return { onChange, user: userEvent.setup() };
}
const year = () => screen.getByRole('textbox', { name: 'Year' }) as HTMLInputElement;
const month = () => screen.getByRole('combobox', { name: 'Month' }) as HTMLSelectElement;
const day = () => screen.getByRole('combobox', { name: 'Day' }) as HTMLSelectElement;

describe('dateParts', () => {
  it('composes_only_what_was_given', () => {
    expect(composeDate({ year: '', month: '', day: '' })).toBe('');
    expect(composeDate({ year: '1850', month: '', day: '' })).toBe('1850');
    expect(composeDate({ year: '1850', month: '3', day: '' })).toBe('Mar 1850');
    expect(composeDate({ year: '1850', month: '3', day: '7' })).toBe('7 Mar 1850');
    expect(composeDate({ year: '', month: '3', day: '7' })).toBe('');
    expect(composeDate({ year: '0', month: '', day: '' })).toBe('');
  });

  it('reads_back_what_it_composes', () => {
    for (const raw of ['', '1850', 'Mar 1850', '7 Mar 1850', '29 Feb 1904', '31 Dec 850']) {
      expect(composeDate(parseDateParts(raw)!)).toBe(raw);
    }
    expect(parseDateParts('07 march 1850')).toEqual({ year: '1850', month: '3', day: '7' });
  });

  it('leaves_to_the_text_field_what_the_parts_cannot_show', () => {
    for (const raw of ['abt 1850', 'about Mar 1850', '03.04.1901', '3/4/1901', '1850-03-04', '11 Feb 1750/51', 'unknown', '31 Apr 1850', '29 Feb 1900', '0']) {
      expect(parseDateParts(raw), raw).toBeNull();
    }
  });

  it('knows_leap_years', () => {
    expect(daysInMonth(1904, 2)).toBe(29);
    expect(daysInMonth(1900, 2)).toBe(28);
    expect(daysInMonth(2000, 2)).toBe(29);
    expect(daysInMonth(1850, 4)).toBe(30);
    expect(daysInMonth(1850, 12)).toBe(31);
  });
});

describe('DatePartsField', () => {
  it('starts_empty_and_sends_nothing_until_asked', () => {
    const { onChange } = setup();
    expect(year().value).toBe('');
    expect(month().disabled).toBe(true);
    expect(day().disabled).toBe(true);
    expect(onChange).not.toHaveBeenCalled();
  });

  it('takes_a_year_alone_then_a_month_then_a_day', async () => {
    const { onChange, user } = setup();
    await user.type(year(), '1850');
    expect(onChange).toHaveBeenLastCalledWith('1850');
    await user.selectOptions(month(), '3');
    expect(onChange).toHaveBeenLastCalledWith('Mar 1850');
    await user.selectOptions(day(), '7');
    expect(onChange).toHaveBeenLastCalledWith('7 Mar 1850');
  });

  it('can_be_emptied_again_and_nothing_stays_out_of_sight', async () => {
    const { onChange, user } = setup('7 Mar 1850');
    await user.clear(year());
    expect(onChange).toHaveBeenLastCalledWith('');
    expect(month().value).toBe('');
    expect(day().value).toBe('');
    expect(month().disabled).toBe(true);
  });

  it('removing_the_month_removes_the_day', async () => {
    const { onChange, user } = setup('7 Mar 1850');
    await user.selectOptions(month(), '');
    expect(onChange).toHaveBeenLastCalledWith('1850');
    expect(day().disabled).toBe(true);
  });

  it('offers_only_the_days_of_the_month_and_drops_one_that_no_longer_exists', async () => {
    const { onChange, user } = setup('31 Mar 1850');
    await user.selectOptions(month(), '4');
    expect(onChange).toHaveBeenLastCalledWith('Apr 1850');
    expect(day().options.length).toBe(31); // 30 days and "not set"
  });

  it('a_leap_day_goes_when_the_year_is_not_a_leap_year', async () => {
    const { onChange, user } = setup('29 Feb 1904');
    await user.clear(year());
    await user.type(year(), '1903');
    expect(onChange).toHaveBeenLastCalledWith('1903');
  });

  it('ignores_letters_in_the_year', async () => {
    const { onChange, user } = setup();
    await user.type(year(), '18a5');
    expect(year().value).toBe('185');
    expect(onChange).toHaveBeenLastCalledWith('185');
  });

  it('shows_an_existing_date_in_its_parts', () => {
    setup('7 Mar 1850');
    expect(year().value).toBe('1850');
    expect(month().value).toBe('3');
    expect(day().value).toBe('7');
  });

  it('opens_a_date_it_cannot_show_as_text_without_changing_it', () => {
    const { onChange } = setup('abt 1850');
    expect((screen.getByRole('textbox', { name: 'Birth date' }) as HTMLInputElement).value).toBe('abt 1850');
    expect(screen.queryByRole('combobox', { name: 'Month' })).toBeNull();
    // It cannot go back to the lists without losing "about".
    expect(screen.queryByRole('button', { name: 'Choose the year, month and day' })).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });

  it('can_type_a_date_the_lists_cannot_give', async () => {
    const { onChange, user } = setup();
    await user.click(screen.getByRole('button', { name: /Type it another way/ }));
    await user.type(screen.getByRole('textbox', { name: 'Birth date' }), 'abt 1850');
    expect(onChange).toHaveBeenLastCalledWith('abt 1850');
    expect(await screen.findByText(/Understood as: about 1850/)).toBeTruthy();
  });

  it('carries_the_date_to_the_text_field', async () => {
    const { user } = setup('7 Mar 1850');
    await user.click(screen.getByRole('button', { name: /Type it another way/ }));
    expect((screen.getByRole('textbox', { name: 'Birth date' }) as HTMLInputElement).value).toBe('7 Mar 1850');
  });
});
