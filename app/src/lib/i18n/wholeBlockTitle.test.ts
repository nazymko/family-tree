import { afterEach, describe, expect, it } from 'vitest';
import { setLanguage } from '.';
import { wholeBlockTitle } from './backend';

afterEach(() => setLanguage('en'));

describe('the heading of a group on the whole-tree chart', () => {
  it('is worded in English', () => {
    expect(wholeBlockTitle({ surname: 'Koval', people: 8, unconnected: false })).toBe('Koval family (8 people)');
    expect(wholeBlockTitle({ surname: 'Koval', people: 1, unconnected: false })).toBe('Koval family (1 person)');
    expect(wholeBlockTitle({ surname: '', people: 3, unconnected: false })).toBe('Family (3 people)');
    expect(wholeBlockTitle({ surname: '', people: 12, unconnected: true })).toBe('Unconnected people (12)');
  });

  it('is worded in Ukrainian, with the right plural', () => {
    setLanguage('uk');
    expect(wholeBlockTitle({ surname: 'Коваль', people: 8, unconnected: false })).toBe('Родина Коваль (8 осіб)');
    expect(wholeBlockTitle({ surname: 'Коваль', people: 2, unconnected: false })).toBe('Родина Коваль (2 особи)');
    expect(wholeBlockTitle({ surname: 'Коваль', people: 1, unconnected: false })).toBe('Родина Коваль (1 особа)');
    expect(wholeBlockTitle({ surname: '', people: 5, unconnected: false })).toBe('Родина (5 осіб)');
    expect(wholeBlockTitle({ surname: '', people: 12, unconnected: true })).toBe('Особи без родинних зв’язків (12)');
  });

  it('shows_the_name_the_user_gave_the_tree_instead_of_the_surname', () => {
    expect(wholeBlockTitle({ surname: 'Koval', customName: 'Hill House', people: 8, unconnected: false })).toBe('Hill House (8 people)');
    setLanguage('uk');
    expect(wholeBlockTitle({ surname: 'Коваль', customName: 'Родовід Горбів', people: 2, unconnected: false })).toBe('Родовід Горбів (2 особи)');
  });
});
