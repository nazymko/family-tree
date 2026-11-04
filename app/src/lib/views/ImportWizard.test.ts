import { describe, expect, it } from 'vitest';
import { fireEvent, screen, waitFor } from '@testing-library/svelte';
import ImportWizard from './ImportWizard.svelte';
import { renderWith } from '../testing/renderWith';
import { MockPlatform } from '../api/mock/MockPlatform';
import { MockApi } from '../api/mock/MockApi';
import { fixtureTree } from '../api/mock/fixtures';

function setup(mode: 'import' | 'create' = 'import') {
  const platform = new MockPlatform();
  renderWith(ImportWizard, { mode, oncancel: () => {} }, { api: new MockApi(fixtureTree(10)), platform });
  return platform;
}

const startButton = () => screen.getByRole('button', { name: 'Start import' });
const needed = () => document.querySelectorAll('[data-required][data-missing]');

describe('ImportWizard required fields', () => {
  it('marks_every_required_field_with_a_star', () => {
    setup();
    expect(screen.getByText('Fields marked * are required.')).toBeInTheDocument();
    expect(document.querySelectorAll('[data-required]').length).toBe(3);
    for (const el of document.querySelectorAll('[data-required]')) expect(el.querySelector('.req')?.textContent).toBe('*');
    expect(screen.getByRole('textbox', { name: /Tree name/ })).toHaveAttribute('aria-required', 'true');
  });

  it('lists_and_highlights_what_is_missing_while_start_is_disabled', () => {
    setup();
    expect(startButton()).toBeDisabled();
    expect(needed().length).toBe(3);
    const todo = screen.getByText('Still needed: a family tree file, a tree name, a folder to keep the tree in.');
    expect(startButton()).toHaveAttribute('aria-describedby', todo.id);
  });

  it('clears_each_highlight_as_the_field_is_filled', async () => {
    const platform = setup();
    platform.next.file = 'C:/Mock/Koval.ged';
    await fireEvent.click(screen.getByRole('button', { name: 'Choose file…' }));
    // Choosing the file also fills the tree name from it.
    await waitFor(() => expect(needed().length).toBe(1));
    expect(screen.getByText('Still needed: a folder to keep the tree in.')).toBeInTheDocument();
    await fireEvent.click(screen.getByRole('button', { name: 'Choose folder…' }));
    await waitFor(() => expect(needed().length).toBe(0));
    expect(screen.queryByText(/Still needed/)).not.toBeInTheDocument();
    expect(startButton()).toBeEnabled();
    expect(startButton()).not.toHaveAttribute('aria-describedby');
  });

  it('create_mode_needs_no_file', () => {
    setup('create');
    expect(document.querySelectorAll('[data-required]').length).toBe(2);
    expect(screen.getByText('Still needed: a tree name, a folder to keep the tree in.')).toBeInTheDocument();
  });
});
