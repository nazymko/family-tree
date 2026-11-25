import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import type { CitationDto } from '../api/api';
import NoteCitations from './NoteCitations.svelte';
import { citationsReferredTo, shifrsIn } from './noteCitations';

// Invented sources only.
const cit = (id: string, title: string, callNumber: string | null, page: string | null) =>
  ({ id, page, source: { id: 's' + id, title, callNumber, abbreviation: null } }) as unknown as CitationDto;
const a = cit('c1', 'Архів 1, метрична книга', '1-2-3', 'PDF с. 10, ч. 5');
const b = cit('c2', 'Інший реєстр', '9-9-9', 'арк. 4');

describe('citationsReferredTo', () => {
  it('finds_shifrs_and_matches_by_call_number_or_title', () => {
    expect(shifrsIn('Архів 1, 1-2-3, дата 12-05-1890')).toEqual(['1-2-3', '12-05-1890']);
    expect(citationsReferredTo('Див. 1-2-3, PDF с. 10', [a, b])).toEqual([a]);
    expect(citationsReferredTo('У книзі «Інший реєстр»', [a, b])).toEqual([b]);
    expect(citationsReferredTo('Без посилань', [a, b])).toEqual([]);
  });
});

describe('NoteCitations', () => {
  it('lists_the_referred_citations_and_opens_one_on_click', async () => {
    const onOpen = vi.fn();
    render(NoteCitations, { text: 'Архів 1, ф. 1, 1-2-3', citations: [a, b], onOpen });
    expect(screen.getByText('Sources named in this note')).toBeInTheDocument();
    expect(screen.queryByText(/Інший реєстр/)).toBeNull();
    await userEvent.setup().click(screen.getByRole('button', { name: 'Архів 1, метрична книга, PDF с. 10, ч. 5' }));
    expect(onOpen).toHaveBeenCalledWith(a);
  });

  it('falls_back_to_a_compact_list_of_all_sources', () => {
    render(NoteCitations, { text: 'No references here', citations: [a, b], onOpen: () => {} });
    expect(screen.getByText('Sources (2)')).toBeInTheDocument();
    expect(screen.getAllByRole('button')).toHaveLength(2);
  });

  it('renders_nothing_without_citations', () => {
    render(NoteCitations, { text: 'x', citations: [], onOpen: () => {} });
    expect(screen.queryByTestId('note-citations')).toBeNull();
  });
});
