import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import PersonCard from './PersonCard.svelte';
import { birthSurnameText } from './birthSurname';
import { setLanguage } from '../i18n';

beforeEach(() => setLanguage('uk'));
afterEach(() => setLanguage('en'));

describe('birth surname line', () => {
  it('words_it_by_sex_in_both_languages', () => {
    expect(birthSurnameText({ sex: 'female', birthSurname: 'Bondar' })).toBe('дів. Bondar');
    expect(birthSurnameText({ sex: 'unknown', birthSurname: 'Bondar' })).toBe('дів. Bondar');
    expect(birthSurnameText({ sex: 'male', birthSurname: 'Bondar' })).toBe('при нар. Bondar');
    setLanguage('en');
    expect(birthSurnameText({ sex: 'female', birthSurname: 'Bondar' })).toBe('née Bondar');
    expect(birthSurnameText({ sex: 'male', birthSurname: 'Bondar' })).toBe('né Bondar');
  });

  it('is_empty_when_there_is_nothing_to_show', () => {
    expect(birthSurnameText({ sex: 'female' })).toBe('');
    expect(birthSurnameText({ sex: 'female', birthSurname: '  ' })).toBe('');
    expect(birthSurnameText(null)).toBe('');
  });

  it('person_card_shows_it_under_the_name_in_ukrainian', () => {
    render(PersonCard, { name: 'Olena Koval', lifespan: 'b. 1991', sex: 'female', birthSurname: 'Bondar', onclick: () => {} });
    expect(screen.getByText('дів. Bondar')).toBeInTheDocument();
    expect(screen.getByRole('button').textContent).toMatch(/Olena Koval\s*дів\. Bondar/);
  });

  it('person_card_without_it_has_no_extra_line', () => {
    const { container } = render(PersonCard, { name: 'Olena Koval', lifespan: 'b. 1991', sex: 'female', onclick: () => {} });
    expect(screen.queryByText(/дів\./)).toBeNull();
    expect(container.querySelector('.maiden')).toBeNull();
  });

  it('person_card_label_includes_it_when_the_relationship_is_spoken', () => {
    render(PersonCard, { name: 'Olena Koval', lifespan: '1991', sex: 'female', birthSurname: 'Bondar', relationship: 'Мати', onclick: () => {} });
    expect(screen.getByRole('button', { name: 'Olena Koval, дів. Bondar, 1991, Мати' })).toBeInTheDocument();
  });
});
