import { beforeEach, describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import EventSummary from './EventSummary.svelte';
import type { EventDto } from '../api/generated/EventDto';
import { setLanguage } from '../i18n';

const event = (over: Partial<EventDto>) => ({ date: null, place: null, description: null, ...over }) as EventDto;
const date = { display: '1950' } as EventDto['date'];
const place = { full: 'Lviv' } as EventDto['place'];

beforeEach(() => setLanguage('en'));

describe('EventSummary', () => {
  it('shows_the_description_when_there_is_no_date_or_place', () => {
    render(EventSummary, { event: event({ description: 'Village doctor' }) });
    expect(screen.getByText('Village doctor')).toBeInTheDocument();
    expect(screen.queryByText('No date or place')).not.toBeInTheDocument();
  });

  it('shows_date_place_and_description_together', () => {
    render(EventSummary, { event: event({ date, place, description: 'Village doctor' }) });
    expect(screen.getByText('1950, Lviv')).toBeInTheDocument();
    expect(screen.getByText('Village doctor')).toBeInTheDocument();
  });

  it('says_so_only_when_there_is_nothing_at_all', () => {
    render(EventSummary, { event: event({ description: '   ' }) });
    expect(screen.getByText('No date or place')).toBeInTheDocument();
  });
});
