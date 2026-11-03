import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import FieldHost from './testing/FieldHost.svelte';
import DialogHost from './testing/DialogHost.svelte';
import ComboHost from './testing/ComboHost.svelte';
import Button from './Button.svelte';
import { applyTheme } from '../state/theme';
import tokens from '../../styles/tokens.css?raw';

describe('Field', () => {
  it('field_links_label_hint_error', () => {
    render(FieldHost, { error: 'Enter a name' });
    const input = screen.getByRole('textbox', { name: 'Given name' });
    const hint = screen.getByText('As written on the record');
    const err = screen.getByText('Enter a name');
    const ids = input.getAttribute('aria-describedby')!.split(' ');
    expect(ids).toContain(hint.id);
    expect(ids).toContain(err.id);
    expect(input).toHaveAttribute('aria-invalid', 'true');
  });

  it('field_without_error_is_valid', () => {
    render(FieldHost, {});
    expect(screen.getByRole('textbox')).not.toHaveAttribute('aria-invalid', 'true');
  });
});

describe('Dialog', () => {
  it('dialog_traps_focus_and_restores', async () => {
    const user = userEvent.setup();
    const onclose = vi.fn();
    render(DialogHost, { onclose });
    const opener = screen.getByRole('button', { name: 'Open dialog' });
    await user.click(opener);
    const dlg = screen.getByRole('dialog', { name: 'Edit event' });
    expect(dlg).toHaveAttribute('aria-modal', 'true');
    for (let i = 0; i < 8; i++) {
      await user.tab();
      expect(dlg.contains(document.activeElement)).toBe(true);
    }
    await user.tab({ shift: true });
    expect(dlg.contains(document.activeElement)).toBe(true);
    await user.keyboard('{Escape}');
    expect(onclose).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(opener).toHaveFocus();
  });
});

describe('Combobox', () => {
  it('combobox_keyboard', async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    render(ComboHost, { onPick });
    const box = screen.getByRole('combobox', { name: 'Place' });
    expect(box).toHaveAttribute('aria-expanded', 'false');
    await user.click(box);
    await user.keyboard('{ArrowDown}');
    expect(box).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getAllByRole('option')).toHaveLength(3);
    const active = box.getAttribute('aria-activedescendant');
    expect(active).toBeTruthy();
    expect(document.getElementById(active!)).toHaveAttribute('aria-selected', 'true');
    await user.keyboard('{ArrowDown}{Enter}');
    expect(onPick).toHaveBeenCalledWith({ id: 2, name: 'Hannover' });
    expect(box).toHaveAttribute('aria-expanded', 'false');
    expect(box).toHaveValue('Hannover');
  });

  it('combobox_create_row_when_no_match', async () => {
    const user = userEvent.setup();
    const onCreate = vi.fn();
    render(ComboHost, { onCreate });
    const box = screen.getByRole('combobox', { name: 'Place' });
    await user.type(box, 'Zzz');
    const opts = screen.getAllByRole('option');
    expect(opts).toHaveLength(1);
    expect(opts[0]).toHaveTextContent('Add place "Zzz"');
    await user.keyboard('{Enter}');
    expect(onCreate).toHaveBeenCalledWith('Zzz');
  });

  it('combobox_escape_closes', async () => {
    const user = userEvent.setup();
    render(ComboHost, {});
    const box = screen.getByRole('combobox');
    await user.click(box);
    await user.keyboard('{ArrowDown}{Escape}');
    expect(box).toHaveAttribute('aria-expanded', 'false');
  });
});

describe('Button', () => {
  const children = createRawSnippet(() => ({ render: () => '<span>Save event</span>' }));

  it('button_disabled_not_focusable_action', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(Button, { disabled: true, onclick, children });
    const btn = screen.getByRole('button', { name: 'Save event' });
    expect(btn).toBeDisabled();
    await user.click(btn);
    await user.tab();
    expect(btn).not.toHaveFocus();
    expect(onclick).not.toHaveBeenCalled();
  });

  it('button_click_fires', async () => {
    const user = userEvent.setup();
    const onclick = vi.fn();
    render(Button, { variant: 'primary', onclick, children });
    await user.click(screen.getByRole('button', { name: 'Save event' }));
    expect(onclick).toHaveBeenCalledTimes(1);
  });
});

describe('theme', () => {
  it('theme_applies_data_attribute', () => {
    applyTheme('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    applyTheme('light');
    expect(document.documentElement.dataset.theme).toBe('light');
    applyTheme('system');
    expect(document.documentElement.hasAttribute('data-theme')).toBe(false);
  });

  it('reduced_motion_tokens', () => {
    const m = tokens.match(/@media \(prefers-reduced-motion: reduce\)\s*\{\s*:root\s*\{([^}]*)\}/);
    expect(m).not.toBeNull();
    for (const d of ['fast', 'base', 'slow']) expect(m![1]).toMatch(new RegExp(`--dur-${d}:\\s*0ms`));
  });
});
