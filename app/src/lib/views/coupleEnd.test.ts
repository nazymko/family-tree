import { describe, expect, it } from 'vitest';
import type { EventDto, PersonRef } from '../api/types';
import { coupleEnd, ROLE_ALONE, ROLE_LEFT } from './coupleEnd';

const p = (id: string, name: string): PersonRef => ({
  id,
  name,
  lifespan: '',
  sex: 'male',
  living: false,
  private: false,
  siblingMarker: false,
});
const dad = p('a', 'Petro Koval');
const mum = p('b', 'Maria Koval');

function ev(kind: string, display: string | null, roles: [PersonRef, string][] = [], negative = false): EventDto {
  return {
    id: `e-${kind}`,
    kind,
    label: kind,
    date: display === null ? null : ({ raw: display, display } as never),
    description: null,
    cause: null,
    negative,
    private: false,
    preferred: true,
    participants: roles.map(([person, role]) => ({ person, role })),
  } as unknown as EventDto;
}

describe('coupleEnd', () => {
  it('no_end_event_means_no_end', () => {
    expect(coupleEnd([ev('marriage', '1930')])).toBeNull();
    expect(coupleEnd(undefined)).toBeNull();
  });

  it('a_divorce_with_who_left_and_who_raised_the_children', () => {
    const end = coupleEnd([
      ev('marriage', '1930'),
      ev('divorce', '12 May 1935', [
        [dad, ROLE_LEFT],
        [mum, ROLE_ALONE],
      ]),
    ]);
    expect(end).toMatchObject({
      reason: 'divorce',
      left: dad,
      alone: mum,
      when: '1935',
    });
  });

  it('a_widowing_is_a_death_and_a_date_without_a_year_is_kept_as_written', () => {
    expect(coupleEnd([ev('custom:widowed', 'after the war', [[dad, ROLE_LEFT]])])).toMatchObject({
      reason: 'death',
      when: 'after the war',
    });
  });

  it('an_annulment_counts_as_a_divorce_and_a_negative_event_is_ignored', () => {
    expect(coupleEnd([ev('annulment', null)])).toMatchObject({
      reason: 'divorce',
      when: '',
    });
    expect(coupleEnd([ev('divorce', null, [], true)])).toBeNull();
  });

  it('a_final_divorce_wins_over_its_filing', () => {
    expect(coupleEnd([ev('divorce_filed', '1934'), ev('divorce', '1935')])?.event.kind).toBe('divorce');
  });
});
