import { beforeEach, describe, expect, it } from 'vitest';
import { A3_BACKGROUND_KEY, A3_TITLE_KEY, a3Background, a3Title } from './a3Pref.svelte';

beforeEach(() => {
  localStorage.removeItem(A3_TITLE_KEY);
  localStorage.removeItem(A3_BACKGROUND_KEY);
  a3Title.reload();
  a3Background.reload();
});

describe('A3 print options', () => {
  it('start_with_the_title_block_on_and_the_paper_background', () => {
    expect(a3Title.value).toBe(true);
    expect(a3Background.value).toBe('paper');
  });

  it('are_remembered', () => {
    a3Title.set(false);
    a3Background.set('transparent');
    expect(localStorage.getItem(A3_TITLE_KEY)).toBe('0');
    expect(localStorage.getItem(A3_BACKGROUND_KEY)).toBe('transparent');
    a3Title.value = true;
    a3Background.value = 'paper';
    a3Title.reload();
    a3Background.reload();
    expect(a3Title.value).toBe(false);
    expect(a3Background.value).toBe('transparent');
    a3Background.set('white');
    a3Background.reload();
    expect(a3Background.value).toBe('white');
  });

  it('ignores_an_unknown_stored_background', () => {
    localStorage.setItem(A3_BACKGROUND_KEY, 'purple');
    a3Background.reload();
    expect(a3Background.value).toBe('paper');
  });
});
