import { afterEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import PeopleFilters from './PeopleFilters.svelte';
import { compact } from '../ui/compact.svelte';
import { t } from '../i18n';
import type { PersonQuery } from '../api/types';

const query = (): PersonQuery => ({ text: '', living: 'any', noSource: false, noBirthDate: false, privateOnly: false, sort: 'surname', desc: false }) as PersonQuery;

afterEach(() => {
  compact.value = false;
});

describe('people filters on a phone', () => {
  it('keeps the name field in view and folds the other filters and the sort into one disclosure', () => {
    compact.value = true;
    const { container } = render(PeopleFilters, { query: query() });
    expect(screen.getByLabelText(t('people.filterByName'))).toBeTruthy();
    const more = container.querySelector('details.more');
    expect(more).not.toBeNull();
    expect(more!.querySelector('summary')!.textContent).toBe(t('people.moreFilters'));
    expect(more!.hasAttribute('open')).toBe(false);
    // The status buttons, the checkboxes and the sort live inside the disclosure.
    expect(more!.querySelector('.seg')).not.toBeNull();
    expect(more!.querySelectorAll('input[type="checkbox"]').length).toBe(5);
  });

  it('shows everything at once on a desktop window', () => {
    compact.value = false;
    const { container } = render(PeopleFilters, { query: query() });
    expect(container.querySelector('details')).toBeNull();
    expect(container.querySelector('.seg')).not.toBeNull();
    expect(container.querySelectorAll('input[type="checkbox"]').length).toBe(5);
  });
});
