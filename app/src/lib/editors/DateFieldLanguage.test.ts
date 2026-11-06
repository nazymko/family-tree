import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import DateField from './DateField.svelte';
import { newApi, renderWithApi } from './testing/helpers';
import { setLanguage } from '../i18n';

let api: ReturnType<typeof newApi>;
beforeEach(() => {
  api = newApi(20);
  setLanguage('uk');
});
afterEach(() => setLanguage('en'));

function box() {
  renderWithApi(api, DateField, { label: 'Дата', value: '', onChange: vi.fn(), order: 'dmy' });
  return screen.getByRole('textbox', { name: 'Дата' });
}

describe('the feedback under a date field follows the language', () => {
  it('understood_date_and_range', async () => {
    await userEvent.setup().type(box(), 'between 1704 and 1712');
    const status = await screen.findByText(/Розпізнано як/);
    expect(status.textContent).toContain('між 1704 і 1712');
    expect(status.textContent).toContain('(1704 – 1712)');
    expect(status.textContent).not.toMatch(/between| to /);
  });

  it('about_a_date_with_a_month', async () => {
    await userEvent.setup().type(box(), 'abt 3 Apr 1901');
    const status = await screen.findByText(/Розпізнано як/);
    expect(status.textContent).toMatch(/близько 3 квіт\S* 1901/);
  });

  it('phrase_keeps_the_words_as_typed', async () => {
    await userEvent.setup().type(box(), '(the winter of the big flood)');
    expect(await screen.findByText(/Збережено як фразу: «\(the winter of the big flood\)»/)).toBeInTheDocument();
  });

  it('warning_about_day_and_month', async () => {
    // The wording of the real parser (gt-session lookup.rs); the mock words its warning differently.
    vi.spyOn(api, 'parseDate').mockResolvedValue({
      raw: '03/04/1901',
      understoodAs: '3 Apr 1901',
      range: null,
      status: 'ok',
      warnings: ['Day and month are ambiguous; read as DMY', 'Double year: the first year is Old Style, the second New Style.'],
    });
    await userEvent.setup().type(box(), '03/04/1901');
    expect(await screen.findByText(/прочитано як день, місяць, рік/)).toBeInTheDocument();
    expect(screen.getByText(/Подвійний рік/)).toBeInTheDocument();
  });
});
